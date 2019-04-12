
use coalescing_buffer::ring::{Sender, Receiver, new_ring_buffer};
use std::time::{Instant, Duration};
use chrono::Local;
use std::thread;


const TIME_UPDATE: i32 = 1;
const SIZE_UPDATE: i32 = 2;

pub fn main() {
    let (sender, receiver) = new_ring_buffer(8);
    let producer = thread::spawn(move || producer_task(sender));
    let consumer = thread::spawn(move || consumer_task(receiver));

    consumer.join().unwrap();
}

fn producer_task(sender: Sender<i32, String>) -> bool {
    let mut messages_sent: i64 = 0;
    let mut last_count_time = Instant::now();

    loop {
        let date = Local::now();


        put(TIME_UPDATE, format!("{}", date.format("%Y-%m-%d][%H:%M:%S")), &sender);
        put(SIZE_UPDATE, format!("buffer size = {}", sender.size()), &sender);
        messages_sent += 2;

        let elapsed = last_count_time.elapsed();
        if elapsed.as_secs() > 10 {
            last_count_time = Instant::now();
            messages_sent += 1;
            put_val_only(   format!("sent {} messages", messages_sent), &sender);
        }
    }
}


fn put(key: i32, value: String, sender: &Sender<i32, String>) {
    let success = sender.offer(key, value);
    if !success {
        panic!(format!("offer of {} = {} failed", key, ""));
    }
}

fn put_val_only(value: String, sender: &Sender<i32, String>) {
    let success = sender.offer_value_only(value);
    if !success {
        panic!(format!("offer of value {} failed", ""));
    }
}

fn consumer_task(receiver: Receiver<i32, String>) {
    let messages:Vec<String>  = Vec::new();

    loop {
        let  messages = receiver.poll(10);
        for message in messages {
            println!("{}", message);
        }
        println!("-----------------");
        std::thread::sleep(Duration::from_millis(1000))
    }
}
