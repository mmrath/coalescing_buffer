extern crate rbuf;


#[cfg(test)]
mod tests {
    use rbuf::ring_buffer::*;
    use std::thread;

    const POISON_PILL: i32 = -1;


    #[test]
    fn should_be_able_to_reuse_capacity() {
        let (sender, receiver) = new_ring_buffer(32);
        let producer = thread::spawn(move || producer_task(sender));
        let _ = thread::spawn(move || consumer_task(receiver));

        let producer_overflow = producer.join().unwrap();
        assert!(!producer_overflow, "ring buffer has overflowed");
    }

    fn producer_task( sender: Sender<i32, i32>) -> bool {
        for run in 0..1000000 {
            for message in 0..10 {
                let success = sender.offer(message, run * 10 + message);
                if !success {
                    sender.offer_value_only(POISON_PILL);
                    return true;
                }
            }
        }
        sender.offer_value_only(POISON_PILL);
        return false;
    }

    fn consumer_task(receiver: Receiver<i32, i32>) {
        loop {
            let values = receiver.poll(100);
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
