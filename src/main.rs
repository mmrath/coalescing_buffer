extern crate coalescing_buffer;

use coalescing_buffer::ring::CoalescingRingBuffer;
use coalescing_buffer::simple::{create_buf, Receiver, Sender};
use std::thread;
use std::sync::Arc;
use std::time::Duration;

const POISON_PILL: i32 = -1;

fn main() {
    //should_be_able_to_reuse_capacity();
    mpsc_buffer_test();
}


fn mpsc_buffer_test() {
    let (sx, rx) = create_buf::<i32>();

    let sx = Arc::new(sx);
    let sx1 = sx.clone();

    let producer0 = thread::spawn(move || {
        for i in 0..10000000 {
            sx.offer(i);
        }
        sx.offer(-1);
    });

    let producer1 = thread::spawn(move || {
        for i in 0..10000000 {
            sx1.offer(i);
        }
        sx1.offer(-1);
    });

    let consumer = thread::spawn(move || {
        loop {
            if let Some(ref value) = rx.poll() {
                if *value == -1 {
                    break;
                }
            }
        }
    });



    let _ = producer0.join();
    let _ = producer1.join();
    let _ = consumer.join();
}


fn should_be_able_to_reuse_capacity() {
    let  buffer: CoalescingRingBuffer<i32, i32> = CoalescingRingBuffer::new(32);
    let shared_buf = Arc::new(buffer);
    let  buf_clone1 = shared_buf.clone();
    let  buf_clone2 = shared_buf.clone();
    let producer = thread::spawn(move || producer_task(buf_clone1));
    let consumer = thread::spawn(move || consumer_task(buf_clone2));

    let producer_overflow = producer.join().unwrap();
    let _ = consumer.join();
    assert!(!producer_overflow, "ring simple has overflowed");
    println!("Completed successfully");
}

fn producer_task(buffer: Arc<CoalescingRingBuffer<i32, i32>>) -> bool {
    for run in 1..1000000 {
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
