use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::cmp;
use std::fmt::Debug;

pub struct CoalescingRingBuffer<K, V> {
    next_write: AtomicUsize,
    last_cleaned: usize,
    rejection_count: AtomicUsize,
    keys: Vec<Option<K>>,
    values: Vec<AtomicPtr<Option<V>>>,
    mask: usize,
    capacity: usize,
    first_write: AtomicUsize,
    last_read: AtomicUsize,
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
    K: Eq + Copy + Debug,
    V: Debug,
{
    pub fn new(capacity: usize) -> CoalescingRingBuffer<K, V> {
        let size = next_power_of_two(capacity);

        let mut keys: Vec<Option<K>> = Vec::with_capacity(size);
        let mut values: Vec<AtomicPtr<Option<V>>> = Vec::with_capacity(size);

        for i in 0..size {
            keys.push(None);
            values.push(AtomicPtr::new(&mut None));
        }

        CoalescingRingBuffer {
            next_write: AtomicUsize::new(1),
            last_cleaned: 0,
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

    pub fn rejection_count(&mut self) -> usize {
        return self.rejection_count.load(Ordering::SeqCst);
    }

    pub fn next_write(&mut self) -> usize {
        return self.next_write.load(Ordering::SeqCst);
    }

    pub fn first_write(&mut self) -> usize {
        return self.first_write.load(Ordering::SeqCst);
    }

    pub fn is_empty(&mut self) -> bool {
        return self.first_write.load(Ordering::SeqCst) == self.next_write.load(Ordering::SeqCst);
    }

    pub fn is_full(&mut self) -> bool {
        return self.size() == self.capacity;
    }

    pub fn offer(&mut self, key: K, value: V) -> bool {
        println!("{:?}", key);

        let next_write = self.next_write.load(Ordering::SeqCst);
        let mut found_index: Option<usize> = None;
        let mut read_reached: bool = false;

        for update_pos in self.first_write.load(Ordering::SeqCst)..next_write {
            let index = self.mask(update_pos);
            if Some(key) == self.keys[index] {
                found_index = Some(index);
                if update_pos >= self.first_write.load(Ordering::SeqCst) {
                    // check that the reader has not read beyond our update point yet
                    read_reached = true;
                    break;
                } else {
                    break;
                }
            }
        }

        println!("Read reached {:?},{:?}", read_reached, found_index);

        if read_reached {
            match found_index {
                Some(x) => {
                    self.values[x].store(&mut Some(value), Ordering::SeqCst);
                    return true;
                }
                None => {
                    panic!("Invalid index.");
                }
            }
        } else {
            return self.add(key, value);
        }
    }

    pub fn add(&mut self, key: K, value: V) -> bool {
        if self.is_full() {
            self.rejection_count.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        self.clean_up();
        self.store(key, value);
        return true;
    }

    pub fn clean_up(&mut self) {
        let last_read = self.last_read.load(Ordering::Relaxed);

        if last_read == self.last_cleaned {
            return;
        }

        while self.last_cleaned < last_read {
            self.last_cleaned = last_read + 1;
            let x = self.last_cleaned;
            let index = self.mask(x);
            self.keys[index] = None;
            self.values[index].store(&mut None, Ordering::Relaxed);
        }
    }

    pub fn store(&mut self, key: K, value: V) {
        let next_write = self.next_write.load(Ordering::Relaxed);
        let index = self.mask(next_write);
        self.keys[index] = Some(key);
        self.values[index].store(&mut Some(value), Ordering::SeqCst);
        self.next_write.store(next_write + 1, Ordering::Relaxed);
    }

    pub fn poll_all(&mut self) -> Vec<V> {
        let total_to_poll = self.next_write.load(Ordering::SeqCst);
        return self.fill(total_to_poll);
    }

    pub fn poll(&mut self, max_items: usize) -> Vec<V> {
        let claim_up_to = cmp::min(
            self.first_write.load(Ordering::SeqCst) + max_items,
            self.next_write.load(Ordering::Relaxed),
        );
        return self.fill(claim_up_to);
    }

    fn fill(&mut self, claim_up_to: usize) -> Vec<V> {
        self.first_write.store(claim_up_to, Ordering::SeqCst);
        let last_read = self.last_read.load(Ordering::SeqCst);

        let mut bucket: Vec<V> = Vec::new();
        for read_index in last_read + 1..claim_up_to {
            let index = self.mask(read_index);
            unsafe {
                let val = self.values[index].swap(&mut None, Ordering::SeqCst);
                bucket.push((*val).take().unwrap());
            }
        }

        self.last_read.fetch_sub(1, Ordering::SeqCst);
        return bucket;
    }

    fn mask(&mut self, value: usize) -> usize {
        return value & self.mask;
    }
}
