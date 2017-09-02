use std::sync::atomic::{AtomicPtr, Ordering};
use std::ptr;
use std::sync::Arc;
use std::marker::PhantomData;


struct Buffer<T> {
    value: AtomicPtr<T>,
}

impl<T: Send> Buffer<T> {
    pub fn new() -> Self {
        Buffer {
            value: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn send(&self, val: T) {
        let val_ptr = Box::into_raw(Box::new(val));
        let old_ptr = self.value.swap(val_ptr, Ordering::SeqCst);
        drop_if_not_null(old_ptr);
    }

    pub fn poll(&self) -> Option<T> {
        let val = self.value.swap(ptr::null_mut(), Ordering::SeqCst);
        if val.is_null() {
            None
        } else {
            Some(unsafe { *(Box::from_raw(val)) })
        }
    }
}

pub struct Receiver<T> {
    buffer: Arc<Buffer<T>>,
    _phantom_data: PhantomData<*mut ()>,
}

unsafe impl<T: Send> Send for Receiver<T> {}

impl<T: Send> Receiver<T> {
    fn new(buf: Arc<Buffer<T>>) -> Self {
        Receiver {
            buffer: buf,
            _phantom_data: PhantomData,
        }
    }

    pub fn poll(&self) -> Option<T> {
        self.buffer.poll()
    }
}

pub struct Sender<T> {
    buffer: Arc<Buffer<T>>,
}

unsafe impl<T: Send> Send for Sender<T> {}

impl<T: Send> Sender<T> {
    fn new(buf: Arc<Buffer<T>>) -> Self {
        Sender { buffer: buf }
    }

    pub fn offer(&self, val: T) {
        self.buffer.send(val);
    }
}


/// Creates a ring buffer and provides a sender(producer) and a receiver(consumer) to send/receive
/// data to/from the buffer. Sender and Receiver can only be access by one thread
///
/// `let (sender, receiver) = new_simple_buffer(25);`
///
pub fn new_simple_buffer<T: Send>() -> (Sender<T>, Receiver<T>) {
    let buf = Arc::new(Buffer::new());
    let buf_clone = buf.clone();
    (Sender::new(buf), Receiver::new(buf_clone))
}

fn drop_if_not_null<V>(val_ptr: *mut V) {
    if !val_ptr.is_null() {
        drop(unsafe { Box::from_raw(val_ptr) });
    }
}
