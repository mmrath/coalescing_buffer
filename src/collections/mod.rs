mod collections{

    use std::sync::atomic::{AtomicUsize, AtomicPtr, Ordering};
    use std::cmp;

    struct CoalescingRingBuffer<K, V> {
        nextWrite: AtomicUsize,
        lastCleaned: usize,
        rejectionCount: AtomicUsize,
        keys: Vec<Option<K>>,
        values: Vec<AtomicPtr<Option<&V>>>,
        mask: usize,
        capacity: usize,
        firstWrite: AtomicUsize,
        lastRead: AtomicUsize,
    }

    fn nextPowerOfTwo(capacity: usize) -> usize {
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


    impl<K,V> CoalescingRingBuffer<K,V> where K:Eq+Copy {
        pub fn new(capacity: usize) -> CoalescingRingBuffer<K, V> {
            let size = nextPowerOfTwo(capacity);
            CoalescingRingBuffer {
                nextWrite: AtomicUsize::new(1),
                lastCleaned: 0,
                rejectionCount: AtomicUsize::new(0),
                firstWrite: AtomicUsize::new(1),
                lastRead: AtomicUsize::new(0),
                capacity: size,
                mask: size - 1,
                keys: Vec::with_capacity(size),
                values: Vec::with_capacity(size),
            }
        }


        pub fn size(&mut self) -> usize {
            // loop until you get a consistent read of both volatile indices
            while true {
                let lastReadBefore = self.lastRead.load(Ordering::SeqCst);
                let currentNextWrite = self.nextWrite.load(Ordering::SeqCst);
                let lastReadAfter = self.lastRead.load(Ordering::SeqCst);

                if lastReadBefore == lastReadAfter {
                    return (currentNextWrite - lastReadBefore) - 1;
                }
            }
            return 0;
        }


        pub fn capacity(&mut self) -> usize {
            return self.capacity;
        }

        pub fn rejection_count(&mut self) -> usize {
            return self.rejectionCount.load(Ordering::SeqCst);
        }

        pub fn next_write(&mut self) -> usize {
            return self.nextWrite.load(Ordering::SeqCst);
        }

        pub fn first_write(&mut self) -> usize {
            return self.firstWrite.load(Ordering::SeqCst);
        }

        pub fn is_empty(&mut self) -> bool {
            return self.firstWrite.load(Ordering::SeqCst) == self.nextWrite.load(Ordering::SeqCst);
        }

        pub fn is_full(&mut self) -> bool {
            return self.size() == self.capacity;
        }

        pub fn offer(&mut self, key: K, value: V) -> bool {
            let nextWrite = self.nextWrite.load(Ordering::SeqCst);

            for updatePosition in self.firstWrite.load(Ordering::SeqCst)..nextWrite {
                let index = self.mask(updatePosition);
                if Some(key) == self.keys[index] {
                    // Use equals here
                    self.values[index].store(&mut Some(value), Ordering::SeqCst);
                    if updatePosition >= self.firstWrite.load(Ordering::SeqCst) {
                        // check that the reader has not read beyond our update point yet
                        return true;
                    } else {
                        break;
                    }
                }
            }
            return self.add(key, &value);
        }

        pub fn add(&mut self, key: K, value: &V) -> bool {
            if self.is_full() {
                self.rejectionCount.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            self.clean_up();
            self.store(key, value);
            return true;
        }

        pub fn clean_up(&mut self) {
            let lastRead = self.lastRead.load(Ordering::Relaxed);

            if lastRead == self.lastCleaned {
                return;
            }

            while self.lastCleaned < lastRead {
                self.lastCleaned = lastRead + 1;
                let x = self.lastCleaned;
                let index = self.mask(x);
                self.keys[index] = None;
                self.values[index].store(&mut None, Ordering::Relaxed);
            }
        }

        pub fn store(&mut self, key: K, value: &V) {
            let nextWrite = self.nextWrite.load(Ordering::Relaxed);
            let index = self.mask(nextWrite);
            self.keys[index] = Some(key);
            self.values[index].store(&mut Some(value), Ordering::SeqCst);
            self.nextWrite.store(nextWrite + 1, Ordering::Relaxed);
        }

        pub fn poll_all(&mut self, bucket: &mut Vec<Option<V>>) -> usize {
            return self.fill(bucket, self.nextWrite.load(Ordering::SeqCst));
        }

        pub fn poll(&mut self, bucket: &mut Vec<Option<V>>, maxItems: usize) -> usize {
            let claimUpTo = cmp::min(self.firstWrite.load(Ordering::SeqCst) + maxItems,
                                     self.nextWrite.load(Ordering::Relaxed));
            return self.fill(bucket, claimUpTo);
        }

        fn fill(&mut self, bucket: &mut Vec<Option<V>>, claimUpTo: usize) -> usize {
            self.firstWrite.store(claimUpTo, Ordering::SeqCst);
            let lastRead = self.lastRead.load(Ordering::SeqCst);

            for readIndex in lastRead + 1..claimUpTo {
                let index = self.mask(readIndex);
                unsafe {
                    let val = self.values[index].load(Ordering::SeqCst).as_ref();
                    match val {
                        Some(&x) => bucket.push(x),
                        None => bucket.push(None),
                    }
                }
            }

            self.lastRead.fetch_sub(1, Ordering::SeqCst);
            return claimUpTo - lastRead - 1;
        }

        fn mask(&mut self, value: usize) -> usize {
            return value & self.mask;
        }
    }
}
