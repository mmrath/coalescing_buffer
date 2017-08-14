extern crate rbuf;

use rbuf::CoalescingRingBuffer;
use std::thread;
use std::sync::Arc;
use std::time::Duration;

const POISON_PILL: i32 = -1;

fn main() {
    should_be_able_to_reuse_capacity();
}



fn should_be_able_to_reuse_capacity() {
    let mut buffer: CoalescingRingBuffer<i32, i32> = CoalescingRingBuffer::new(32);
    let shared_buf = Arc::new(buffer);
    let mut buf_clone1 = shared_buf.clone();
    let mut buf_clone2 = shared_buf.clone();
    let producer = thread::spawn(move || producer_task(buf_clone1));
    let consumer = thread::spawn(move || consumer_task(buf_clone2));

    let producer_overflow = producer.join().unwrap();
    let _ = consumer.join();
    assert!(!producer_overflow, "ring buffer has overflowed");
    println!("Completed successfully");
}

fn producer_task(buffer: Arc<CoalescingRingBuffer<i32, i32>>) -> bool {
    for run in 1..1000000 {
        println!("P: {:}", run);
        //println!("P: {:?}", buffer);
        //thread::sleep(Duration::from_millis(100));

        for message in 1..10 {

            let success = buffer.offer(message, message);
            if !success {
                buffer.offer_value_only(POISON_PILL);
                return true;
            }
        }
    }
    buffer.offer_value_only(POISON_PILL);
    return false;
}

fn consumer_task(buffer: Arc<CoalescingRingBuffer<i32, i32>>) {
    loop {
        let values = buffer.poll(100);
        if values.contains(&POISON_PILL) {
            return;
        }
    }
}
