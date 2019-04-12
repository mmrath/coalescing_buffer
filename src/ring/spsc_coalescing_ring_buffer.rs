use crossbeam_utils::atomic::AtomicCell;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{cmp, mem};

struct CoalescingRingBuffer<K, V>
where
    V: Send + Clone,
{
    next_write: AtomicUsize,
    last_cleaned: AtomicUsize,
    rejection_count: AtomicUsize,
    keys: Vec<KeyCell<KeyHolder<K>>>,
    values: Vec<AtomicCell<Option<V>>>,
    mask: usize,
    capacity: usize,
    first_write: AtomicUsize,
    last_read: AtomicUsize,
}

#[derive(Debug)]
struct KeyCell<T> {
    value: UnsafeCell<T>,
}

impl<T> KeyCell<T> {
    pub fn new(value: T) -> KeyCell<T> {
        KeyCell {
            value: UnsafeCell::new(value),
        }
    }
    pub fn set(&self, val: T) {
        let old = mem::replace(unsafe { &mut *self.value.get() }, val);
        drop(old);
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.value.get() }
    }
}

#[derive(PartialEq, Debug)]
enum KeyHolder<T> {
    Empty,
    NonEmpty(T),
    NonCollapsible,
}

fn next_power_of_two(capacity: usize) -> usize {
    let mut v = capacity;
    v = v - 1;
    v = v | (v >> 1);
    v = v | (v >> 2);
    v = v | (v >> 4);
    v = v | (v >> 8);
    v = v | (v >> 16);
    v = v + 1;
    return v;
}

#[allow(unused)]
impl<K, V> CoalescingRingBuffer<K, V>
where
    K: Eq + Send,
    V: Send + Clone,
{
    pub fn new(capacity: usize) -> CoalescingRingBuffer<K, V> {
        let size = next_power_of_two(capacity);

        let mut keys: Vec<KeyCell<KeyHolder<K>>> = Vec::with_capacity(size);
        let mut values: Vec<AtomicCell<Option<V>>> = Vec::with_capacity(size);

        for _ in 0..size {
            keys.push(KeyCell::new(KeyHolder::Empty));
            values.push(AtomicCell::new(None));
        }

        CoalescingRingBuffer {
            next_write: AtomicUsize::new(1),
            last_cleaned: AtomicUsize::new(0),
            rejection_count: AtomicUsize::new(0),
            first_write: AtomicUsize::new(1),
            last_read: AtomicUsize::new(0),
            capacity: size,
            mask: size - 1,
            keys,
            values,
        }
    }

    pub fn size(&self) -> usize {
        // loop until you get a consistent read of both volatile indices
        loop {
            let last_read_before = self.last_read.load(Ordering::SeqCst);
            let current_next_write = self.next_write.load(Ordering::SeqCst);
            let last_read_after = self.last_read.load(Ordering::SeqCst);

            if last_read_before == last_read_after {
                return (current_next_write - last_read_before) - 1;
            }
        }
    }

    pub fn capacity(&self) -> usize {
        return self.capacity;
    }

    pub fn rejection_count(&self) -> usize {
        return self.rejection_count.load(Ordering::SeqCst);
    }

    pub fn next_write(&self) -> usize {
        return self.next_write.load(Ordering::SeqCst);
    }

    pub fn first_write(&self) -> usize {
        return self.first_write.load(Ordering::SeqCst);
    }

    pub fn is_empty(&self) -> bool {
        return self.first_write.load(Ordering::SeqCst) == self.next_write.load(Ordering::SeqCst);
    }

    pub fn is_full(&self) -> bool {
        return self.size() == self.capacity;
    }

    pub fn offer(&self, key: K, value: V) -> bool {
        use std::borrow::Cow;

        let next_write = self.next_write.load(Ordering::SeqCst);
        let key_type = KeyHolder::NonEmpty(key);
        for update_pos in self.first_write.load(Ordering::SeqCst)..next_write {
            let index = self.mask(update_pos);
            if &key_type == self.keys[index].get() {
                let old_ptr = self.values[index].swap(Some(value.clone()));
                if update_pos >= self.first_write.load(Ordering::SeqCst) {
                    return true;
                } else {
                    //self.values[index].compare_and_swap(old_ptr, val_ptr);
                    break;
                }
            }
        }
        return self.add(key_type, value);
    }

    pub fn offer_value_only(&self, value: V) -> bool {
        return self.add(KeyHolder::NonCollapsible, value);
    }

    fn add(&self, key: KeyHolder<K>, value: V) -> bool {
        if self.is_full() {
            self.rejection_count.fetch_add(1, Ordering::SeqCst);
            return false;
        }
        self.clean_up();
        self.store(key, value);
        return true;
    }

    pub fn clean_up(&self) {
        let last_read = self.last_read.load(Ordering::SeqCst);

        let last_cln = self.last_cleaned.load(Ordering::Relaxed);
        if last_read == last_cln {
            return;
        }

        for x in last_cln..last_read {
            let index = self.mask(x + 1);
            self.keys[index].set(KeyHolder::Empty);
            let old_val = self.values[index].swap(None);
        }
        self.last_cleaned.store(last_read, Ordering::SeqCst);
    }

    fn store(&self, key: KeyHolder<K>, value: V) {
        let next_write = self.next_write.load(Ordering::SeqCst);
        let index = self.mask(next_write);
        self.keys[index].set(key);
        let old_ptr = self.values[index].swap(Some(value));
        self.next_write.store(next_write + 1, Ordering::SeqCst);
    }

    pub fn poll_all(&self) -> Vec<V> {
        let total_to_poll = self.next_write.load(Ordering::SeqCst);
        return self.fill(total_to_poll);
    }

    pub fn poll(&self, max_items: usize) -> Vec<V> {
        let claim_up_to = cmp::min(
            self.first_write.load(Ordering::SeqCst) + max_items,
            self.next_write.load(Ordering::SeqCst),
        );
        return self.fill(claim_up_to);
    }

    fn fill(&self, claim_up_to: usize) -> Vec<V> {
        self.first_write.store(claim_up_to, Ordering::SeqCst);
        let last_read = self.last_read.load(Ordering::SeqCst);

        let mut bucket: Vec<V> = Vec::new();
        for read_index in last_read + 1..claim_up_to {
            let index = self.mask(read_index);
            let val = self.values[index].swap(None);
            if val.is_none() {
                //println!("{:?}", self);
                //println!("claim_up_to:{:?}", claim_up_to);
                panic!("Null pointer is not expected here!")
            } else {
                bucket.push(val.unwrap());
            }
        }
        self.last_read.store(claim_up_to - 1, Ordering::SeqCst);
        return bucket;
    }

    fn mask(&self, value: usize) -> usize {
        return value & self.mask;
    }
}

unsafe impl<K, V> Send for CoalescingRingBuffer<K, V> where V: Send + Clone {}
unsafe impl<K, V> Sync for CoalescingRingBuffer<K, V> where V: Send + Clone {}

pub struct Receiver<K, V>
where
    V: Send + Clone,
{
    buffer: Arc<CoalescingRingBuffer<K, V>>,
    _phantom_data: PhantomData<*mut ()>, //This to make sure we have only one thread access this
}

unsafe impl<K: Send, V: Send + Clone> Send for Receiver<K, V> where V: Send {}

impl<K: Send + Eq, V: Send + Clone> Receiver<K, V> {
    fn new(buf: Arc<CoalescingRingBuffer<K, V>>) -> Self {
        Receiver {
            buffer: buf,
            _phantom_data: PhantomData,
        }
    }

    pub fn poll_all(&self) -> Vec<V> {
        return self.buffer.poll_all();
    }

    pub fn poll(&self, max_items: usize) -> Vec<V> {
        return self.buffer.poll(max_items);
    }

    pub fn size(&self) -> usize {
        self.buffer.size()
    }
}

pub struct Sender<K, V>
where
    V: Send + Clone,
{
    buffer: Arc<CoalescingRingBuffer<K, V>>,
    _phantom_data: PhantomData<*mut ()>, //This to make sure we have only one thread access this
}

unsafe impl<K: Send, V: Send + Clone> Send for Sender<K, V> where V: Send {}

impl<K: Send + Eq, V: Send + Clone> Sender<K, V>
where
    V: Send + Clone,
{
    fn new(buf: Arc<CoalescingRingBuffer<K, V>>) -> Self {
        Sender {
            buffer: buf,
            _phantom_data: PhantomData,
        }
    }

    pub fn offer(&self, key: K, value: V) -> bool {
        return self.buffer.offer(key, value);
    }

    pub fn offer_value_only(&self, value: V) -> bool {
        return self.buffer.offer_value_only(value);
    }

    pub fn size(&self) -> usize {
        self.buffer.size()
    }

    pub fn rejection_count(&self) -> usize {
        self.buffer.rejection_count()
    }
}

/// Creates a ring buffer and provides a sender(to produce) and a receiver(consumer) to send/receive
/// data of the buffer. Sender and Receiver can only be access by one thread
///
/// `let (sender, receiver) = new_ring_buffer(25);`
///
pub fn new_ring_buffer<K: Send + Eq, V: Send + Clone>(
    capacity: usize,
) -> (Sender<K, V>, Receiver<K, V>) {
    let buf = Arc::new(CoalescingRingBuffer::new(capacity));
    let buf_clone = buf.clone();
    (Sender::new(buf), Receiver::new(buf_clone))
}

#[cfg(test)]
mod tests {
    use super::*;

    static VOD_SNAPSHOT_1: MarketSnapshot = MarketSnapshot {
        instrument_id: 1,
        bid: 3,
        ask: 4,
    };
    static VOD_SNAPSHOT_2: MarketSnapshot = MarketSnapshot {
        instrument_id: 1,
        bid: 5,
        ask: 6,
    };
    static BP_SNAPSHOT: MarketSnapshot = MarketSnapshot {
        instrument_id: 2,
        bid: 7,
        ask: 8,
    };

    #[derive(Debug, Clone, PartialEq, Eq, Copy)]
    struct MarketSnapshot {
        pub instrument_id: usize,
        pub bid: isize,
        pub ask: isize,
    }

    impl MarketSnapshot {
        pub fn new(instrument_id: usize, best_bid: isize, best_ask: isize) -> Self {
            MarketSnapshot {
                instrument_id,
                bid: best_bid,
                ask: best_ask,
            }
        }
    }

    fn create_buffer(capacity: usize) -> CoalescingRingBuffer<usize, MarketSnapshot> {
        CoalescingRingBuffer::new(capacity)
    }

    #[test]
    fn should_correctly_increase_the_capacity_to_the_next_higher_power_of_two() {
        check_capacity(1024, &create_buffer(1023));
        check_capacity(1024, &create_buffer(1024));
        check_capacity(2048, &create_buffer(1025));
    }

    fn check_capacity(capacity: usize, buffer: &CoalescingRingBuffer<usize, MarketSnapshot>) {
        assert_eq!(capacity, buffer.capacity());
        for i in 0..capacity {
            let success = buffer.offer(0, MarketSnapshot::new(i, i as isize, i as isize));
            assert!(success);
        }
    }

    #[test]
    fn should_correctly_report_size() {
        let buffer = create_buffer(2);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer(VOD_SNAPSHOT_1.instrument_id, VOD_SNAPSHOT_1.clone());
        assert_eq!(2, buffer.size());
        assert!(!buffer.is_empty());
        assert!(buffer.is_full());

        let _ = buffer.poll(1);
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        let _ = buffer.poll(1);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    fn should_reject_new_keys_when_full() {
        let buffer = create_buffer(2);
        buffer.offer(1, BP_SNAPSHOT.clone());
        buffer.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(!buffer.offer(4, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, buffer.size());
    }

    #[test]
    fn should_accept_existing_keys_when_full() {
        let buffer = create_buffer(2);
        buffer.offer(1, BP_SNAPSHOT.clone());
        buffer.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(buffer.offer(2, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, buffer.size());
    }

    #[test]
    fn should_return_single_value() {
        let buffer = create_buffer(2);

        add_key_value(&buffer, BP_SNAPSHOT.clone());
        assert_contains(&buffer, vec![BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn should_return_two_values_with_different_keys() {
        let buffer = create_buffer(2);
        add_key_value(&buffer, BP_SNAPSHOT.clone());
        add_key_value(&buffer, VOD_SNAPSHOT_1.clone());

        assert_contains(&buffer, vec![BP_SNAPSHOT.clone(), VOD_SNAPSHOT_1.clone()]);
    }

    #[test]
    fn should_update_values_with_equal_keys() {
        let buffer = create_buffer(2);
        add_key_value(&buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(&buffer, vec![VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn should_not_update_values_without_keys() {
        let buffer = create_buffer(2);
        add_value(&buffer, VOD_SNAPSHOT_1.clone());
        add_value(&buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(
            &buffer,
            vec![VOD_SNAPSHOT_1.clone(), VOD_SNAPSHOT_2.clone()],
        );
    }

    #[test]
    fn should_update_values_with_equal_keys_and_preserve_ordering() {
        let buffer = create_buffer(2);
        add_key_value(&buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&buffer, BP_SNAPSHOT.clone());
        add_key_value(&buffer, VOD_SNAPSHOT_2.clone());

        assert_contains(&buffer, vec![VOD_SNAPSHOT_2.clone(), BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn should_not_update_values_if_read_occurs_between_values() {
        let buffer = create_buffer(2);

        add_key_value(&buffer, VOD_SNAPSHOT_1.clone());
        assert_contains(&buffer, vec![VOD_SNAPSHOT_1.clone()]);

        add_key_value(&buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(&buffer, vec![VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn should_return_only_the_maximum_number_of_requested_items() {
        let buffer = create_buffer(10);
        add_value(&buffer, BP_SNAPSHOT.clone());
        add_value(&buffer, VOD_SNAPSHOT_1.clone());
        add_value(&buffer, VOD_SNAPSHOT_2.clone());

        let snapshots = buffer.poll(2);
        assert_eq!(2, snapshots.len());
        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_1, snapshots.get(1).unwrap());

        let snapshots = buffer.poll(1);
        assert_eq!(1, snapshots.len());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(0).unwrap());

        assert_is_empty(&buffer);
    }

    #[test]
    fn should_return_all_items_without_request_limit() {
        let buffer = create_buffer(10);
        add_value(&buffer, BP_SNAPSHOT.clone());
        add_key_value(&buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&buffer, VOD_SNAPSHOT_2.clone());

        let snapshots = buffer.poll_all();
        assert_eq!(2, snapshots.len());

        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(1).unwrap());

        assert_is_empty(&buffer);
    }

    #[test]
    fn should_count_rejections() {
        let buffer = create_buffer(2);
        assert_eq!(0, buffer.rejection_count());

        buffer.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer(1, VOD_SNAPSHOT_1.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer(1, VOD_SNAPSHOT_2.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(1, buffer.rejection_count());

        buffer.offer(2, BP_SNAPSHOT.clone());
        assert_eq!(2, buffer.rejection_count());
    }

    #[test]
    fn should_use_object_equality_to_compare_keys() {
        let buffer: CoalescingRingBuffer<String, MarketSnapshot> = CoalescingRingBuffer::new(2);

        buffer.offer(String::from("boo"), BP_SNAPSHOT.clone());
        buffer.offer(String::from("boo"), BP_SNAPSHOT.clone());

        assert_eq!(1, buffer.size());
    }

    fn assert_is_empty(buffer: &CoalescingRingBuffer<usize, MarketSnapshot>) {
        assert_contains(buffer, vec![]);
    }

    fn add_key_value(
        buffer: &CoalescingRingBuffer<usize, MarketSnapshot>,
        snapshot: MarketSnapshot,
    ) {
        assert!(buffer.offer(snapshot.instrument_id, snapshot));
    }

    fn assert_contains(
        buffer: &CoalescingRingBuffer<usize, MarketSnapshot>,
        expected: Vec<MarketSnapshot>,
    ) -> bool {
        let actual = buffer.poll_all();
        let ret = (actual.len() == expected.len()) && // zip stops at the shortest
            actual.iter().zip(expected).all(|(a, b)| a == &b);
        return ret;
    }

    fn add_value(buffer: &CoalescingRingBuffer<usize, MarketSnapshot>, snapshot: MarketSnapshot) {
        assert!(buffer.offer_value_only(snapshot));
    }
}
