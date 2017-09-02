extern crate coalescing_buffer;

use coalescing_buffer::simple::{create_buf};
use std::thread;
use std::sync::Arc;


fn main() {
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

