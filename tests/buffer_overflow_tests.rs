extern crate rbuf;


#[cfg(test)]
mod tests {
    use rbuf::*;
    use std::thread;
    use std::sync::Arc;
    use std::cell::Cell;

    const POISON_PILL: i32 = -1;


    #[test]
    fn should_be_able_to_reuse_capacity() {
        let mut buffer: CoalescingRingBuffer<i32, i32> = CoalescingRingBuffer::new(32);
        let shared_buf = Arc::new(buffer);
        let mut buf_clone1 = shared_buf.clone();
        let mut buf_clone2 = shared_buf.clone();
        let producer = thread::spawn(move || producer_task(buf_clone1));
        let consumer = thread::spawn(move || consumer_task(buf_clone2));

        let producer_overflow = producer.join().unwrap();
        assert!(!producer_overflow, "ring buffer has overflowed");
    }

    fn producer_task(mut shared_buf: Arc<CoalescingRingBuffer<i32, i32>>) -> bool {
        let mut buffer = shared_buf;
        for run in 0..1000000 {
            for message in 0..10 {
                let success = buffer.offer(message, run * 10 + message);
                if !success {
                    buffer.offer_value_only(POISON_PILL);
                    return true;
                }
            }
        }
        buffer.offer_value_only(POISON_PILL);
        return false;
    }

    fn consumer_task(mut buffer: Arc<CoalescingRingBuffer<i32, i32>>) {
        loop {
            let values = buffer.poll(100);
            if values.contains(&POISON_PILL) {
                return;
            }
        }
    }


    /*
    struct MyThreadSafeStruct {
        inner: UnsafeCell<i32>
    }

    unsafe impl Send for MyThreadSafeStruct {}

    unsafe impl Sync for MyThreadSafeStruct {}

    impl MyThreadSafeStruct {
        fn produce(& self) {}
        fn consume(& self) {}
    }

    #[test]
    fn test_this(){
        let obj = MyThreadSafeStruct{inner: UnsafeCell::};
        let shared_obj = Arc::new(obj);
        let clone1 = shared_obj.clone();
        let clone2 = shared_obj.clone();
        let producer = thread::spawn(move || { clone1.produce() });
        let consumer = thread::spawn(move || { clone2.consume() });

        producer.join().unwrap();
        consumer.join().unwrap();
    }
    */

}
