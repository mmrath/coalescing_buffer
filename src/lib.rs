#![feature(optin_builtin_traits)]

mod coalescing_ring_buffer;
mod spsc_cr_buffer;
pub use coalescing_ring_buffer::CoalescingRingBuffer;
pub use spsc_cr_buffer::{Receiver,Sender,create_buf};
