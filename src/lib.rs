/// Coalescing ring buffer is a circular buffer of key and value pair(like a map). A update with
/// same key will replace the value if the value is not yet read
///
/// ```
/// extern crate coalescing_buffer;
///
/// use coalescing_buffer::ring::*;
/// use std::thread;
///
/// const POISON_PILL: i32 = -1;
///
///
/// fn main() {
///     let (sender, receiver) = new_ring_buffer(25); // This will be changed to 32 nearest 2^x
///     let producer = thread::spawn(move || producer_task(sender));
///     let consumer = thread::spawn(move || consumer_task(receiver));
///
///     let producer_overflow = producer.join().unwrap();
///     consumer.join();
///     assert!(!producer_overflow, "ring simple has overflowed");
/// }
///
/// fn producer_task(sender: Sender<i32, i32>) -> bool {
///     for run in 0..100000 {
///         for message in 0..10 {
///             let success = sender.offer(message, run * 10 + message);
///             if !success {
///                 sender.offer_value_only(POISON_PILL);
///                 return true;
///             }
///         }
///     }
///     sender.offer_value_only(POISON_PILL);
///     return false;
/// }
///
/// fn consumer_task(receiver: Receiver<i32, i32>) {
///     loop {
///         let values = receiver.poll(100);
///         if values.contains(&POISON_PILL) {
///             return;
///         }
///     }
/// }
/// ```
///
pub mod ring;
