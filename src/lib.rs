/// # Simple Buffer Usage Example
/// ```
/// use coalescing_buffer::simple::create_buf;
/// use std::thread;
/// use std::sync::Arc;
///
/// fn main(){
///     let (sx, rx) = create_buf::<i32>();
///
///     let sx = Arc::new(sx);
///     let sx1 = sx.clone();
///
///     let producer0 = thread::spawn(move || {
///         for i in 0..10000000 {
///             sx.offer(i);
///         }
///         sx.offer(-1);
///     });
///
///     let producer1 = thread::spawn(move || {
///         for i in 0..10000000 {
///             sx1.offer(i);
///         }
///         sx1.offer(-1);
///     });
///
///     let consumer = thread::spawn(move || loop {
///         if let Some(ref value) = rx.poll() {
///             if *value == -1 {
///                 break;
///             }
///         }
///     });
///
///     let _ = producer0.join();
///     let _ = producer1.join();
///     let _ = consumer.join();
///
/// }
/// ```
///
/// # Ring buffer usage
///
/// ```
/// use coalescing_buffer::ring::*;
/// use std::thread;
///
/// const POISON_PILL: i32 = -1;
///
///
/// fn main() {
///     let (sender, receiver) = new_ring_buffer(32);
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


pub mod simple;
pub mod ring;
