use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::{cmp, mem, ptr};
use std::fmt::Debug;
use std::cell::UnsafeCell;

#[derive(Debug)]
pub struct CoalescingRingBuffer<K, V> {
    next_write: AtomicUsize,
    last_cleaned: AtomicUsize,
    rejection_count: AtomicUsize,
    keys: Vec<KeyCell<KeyHolder<K>>>,
    values: Vec<AtomicPtr<V>>,
    mask: usize,
    capacity: usize,
    first_write: AtomicUsize,
    last_read: AtomicUsize,
}

#[derive(Debug)]
pub struct KeyCell<T> {
    value: UnsafeCell<T>,
}

impl<T> KeyCell<T> {
    pub fn new(value: T) -> KeyCell<T> {
        KeyCell {
            value: UnsafeCell::new(value),
        }
    }
    pub fn set(&self, val: T) {

        //let val_ptr = Box::into_raw(Box::new(val));
        //let old_ptr = self.value.swap(val_ptr, Ordering::SeqCst);
        //drop_value(old_ptr);

        let old = self.replace(val);
        drop(old);
    }

    fn replace(&self, val: T) -> T {
        mem::replace(unsafe { &mut *self.value.get() }, val)
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


impl<K, V> CoalescingRingBuffer<K, V>
where
    K: Eq + Debug + Send,
    V: Debug,
{
    pub fn new(capacity: usize) -> CoalescingRingBuffer<K, V> {
        let size = next_power_of_two(capacity);

        let mut keys: Vec<KeyCell<KeyHolder<K>>> = Vec::with_capacity(size);
        let mut values: Vec<AtomicPtr<V>> = Vec::with_capacity(size);

        for _ in 0..size {
            keys.push(KeyCell::new(KeyHolder::Empty));
            values.push(AtomicPtr::new(ptr::null_mut()));
        }

        CoalescingRingBuffer {
            next_write: AtomicUsize::new(1),
            last_cleaned: AtomicUsize::new(0),
            rejection_count: AtomicUsize::new(0),
            first_write: AtomicUsize::new(1),
            last_read: AtomicUsize::new(0),
            capacity: size,
            mask: size - 1,
            keys: keys,
            values: values,
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
        let next_write = self.next_write.load(Ordering::SeqCst);
        let val_ptr = Box::into_raw(Box::new(value));
        let key_type = KeyHolder::NonEmpty(key);
        for update_pos in self.first_write.load(Ordering::SeqCst)..next_write {
            let index = self.mask(update_pos);
            if &key_type == self.keys[index].get() {
                let old_ptr = self.values[index].swap(val_ptr, Ordering::SeqCst);
                if update_pos >= self.first_write.load(Ordering::SeqCst) {
                    // check that the reader has not read beyond our update point yet
                    drop_value(old_ptr);
                    return true;
                } else {
                    self.values[index].swap(old_ptr, Ordering::SeqCst);
                    //::std::thread::sleep(::std::time::Duration::from_millis(2000));
                    break;
                }
            }
        }
        return self.add(key_type, val_ptr);
    }

    pub fn offer_value_only(&self, value: V) -> bool {
        return self.add(KeyHolder::NonCollapsible, Box::into_raw(Box::new(value)));
    }

    fn add(&self, key: KeyHolder<K>, value: *mut V) -> bool {
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
            self.keys[index].replace(KeyHolder::Empty);
            let old_val = self.values[index].swap(ptr::null_mut(), Ordering::SeqCst);
            drop_value(old_val);
        }
        self.last_cleaned.store(last_read, Ordering::SeqCst);
    }

    fn store(&self, key: KeyHolder<K>, value: *mut V) {
        let next_write = self.next_write.load(Ordering::SeqCst);
        let index = self.mask(next_write);
        self.keys[index].replace(key);
        let old_ptr = self.values[index].swap(value, Ordering::SeqCst);
        drop_value(old_ptr);
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
            let val = self.values[index].swap(ptr::null_mut(), Ordering::SeqCst);
            if val.is_null() {
                println!("{:?}", self);
                println!("claim_up_to:{:?}", claim_up_to);
                panic!("Null pointer is not expected here!")
            } else {
                bucket.push(unsafe { *(Box::from_raw(val)) });
            }
        }
        self.last_read.store(claim_up_to - 1, Ordering::SeqCst);
        return bucket;
    }

    fn mask(&self, value: usize) -> usize {
        return value & self.mask;
    }
}


unsafe impl<K, V> Send for CoalescingRingBuffer<K, V> {}

unsafe impl<K, V> Sync for CoalescingRingBuffer<K, V> {}

fn drop_value<V>(val_ptr: *mut V)
where
    V: Debug,
{
    if !val_ptr.is_null() {
        let val = unsafe { Box::from_raw(val_ptr) };
    }
}
