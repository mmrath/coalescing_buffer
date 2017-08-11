use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::{cmp, ptr};
use std::fmt::Debug;

pub struct CoalescingRingBuffer<K, V> {
    next_write: AtomicUsize,
    last_cleaned: usize,
    rejection_count: AtomicUsize,
    keys: Vec<Option<K>>,
    values: Vec<AtomicPtr<V>>,
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
        let mut values: Vec<AtomicPtr<V>> = Vec::with_capacity(size);

        for _ in 0..size {
            keys.push(None);
            values.push(AtomicPtr::new(ptr::null_mut()));
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
        let next_write = self.next_write.load(Ordering::SeqCst);
        let val_ptr = Box::into_raw(Box::new(value));
        for update_pos in self.first_write.load(Ordering::SeqCst)..next_write {
            let index = self.mask(update_pos);
            if Some(key) == self.keys[index] {
                let old_ptr = self.values[index].swap(val_ptr, Ordering::SeqCst);
                drop_value(old_ptr);
                if update_pos >= self.first_write.load(Ordering::SeqCst) {
                    // check that the reader has not read beyond our update point yet
                    return true;
                } else {
                    break;
                }
            }
        }
        return self.add(key, val_ptr);
    }

    fn add(&mut self, key: K, value: *mut V) -> bool {
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
            let old_val = self.values[index].swap(ptr::null_mut(), Ordering::Relaxed);
            drop_value(old_val);
        }
    }

    fn store(&mut self, key: K, value: *mut V) {
        let next_write = self.next_write.load(Ordering::Relaxed);
        let index = self.mask(next_write);
        self.keys[index] = Some(key);
        let old_ptr = self.values[index].swap(value, Ordering::SeqCst);
        drop_value(old_ptr);
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
            let val = self.values[index].swap(ptr::null_mut(), Ordering::SeqCst);
            if val.is_null() {
                panic!("Null pointer is not expected here!")
            } else {
                bucket.push(unsafe { *(Box::from_raw(val)) });
            }
        }

        self.last_read.store(claim_up_to - 1, Ordering::SeqCst);
        return bucket;
    }

    fn mask(&mut self, value: usize) -> usize {
        return value & self.mask;
    }
}

fn drop_value<V>(val_ptr: *mut V) {
    if !val_ptr.is_null() {
        let _ = unsafe { Box::from_raw(val_ptr) };
    }
}
