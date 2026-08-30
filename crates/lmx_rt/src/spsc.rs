//! Bounded single-producer / single-consumer lock-free ring.
//!
//! `T` may own heap data (a command carrying a boxed track): values are moved in
//! and out, never cloned. Capacity is rounded up to a power of two.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Shared<T> {
    buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: usize,
    head: AtomicUsize, // next slot to pop (consumer-owned)
    tail: AtomicUsize, // next slot to push (producer-owned)
}

// SAFETY: slots between head and tail are owned by the consumer, the rest by the
// producer; the indices are the only shared state and are atomic.
unsafe impl<T: Send> Sync for Shared<T> {}
unsafe impl<T: Send> Send for Shared<T> {}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        let head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        for i in head..tail {
            // SAFETY: slots in [head, tail) hold initialized values nobody popped.
            unsafe { (*self.buf[i & self.mask].get()).assume_init_drop() };
        }
    }
}

pub struct Producer<T> {
    shared: Arc<Shared<T>>,
}
pub struct Consumer<T> {
    shared: Arc<Shared<T>>,
}

/// Create a ring holding at least `capacity` items.
pub fn spsc<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let cap = capacity.max(2).next_power_of_two();
    let buf: Vec<UnsafeCell<MaybeUninit<T>>> = (0..cap).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
    let shared = Arc::new(Shared {
        buf: buf.into_boxed_slice(),
        mask: cap - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (Producer { shared: shared.clone() }, Consumer { shared })
}

impl<T> Producer<T> {
    /// Push, or hand the value back if the ring is full. Never blocks.
    #[inline]
    pub fn push(&mut self, v: T) -> Result<(), T> {
        let s = &*self.shared;
        let tail = s.tail.load(Ordering::Relaxed);
        let head = s.head.load(Ordering::Acquire);
        if tail - head > s.mask {
            return Err(v);
        }
        // SAFETY: slot `tail` is free (not in [head, tail)) and producer-owned.
        unsafe { (*s.buf[tail & s.mask].get()).write(v) };
        s.tail.store(tail + 1, Ordering::Release);
        Ok(())
    }

    pub fn is_full(&self) -> bool {
        let s = &*self.shared;
        s.tail.load(Ordering::Relaxed) - s.head.load(Ordering::Acquire) > s.mask
    }

    pub fn capacity(&self) -> usize {
        self.shared.mask + 1
    }
}

impl<T> Consumer<T> {
    /// Pop the oldest value, if any. Never blocks.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let s = &*self.shared;
        let head = s.head.load(Ordering::Relaxed);
        let tail = s.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // SAFETY: slot `head` is initialized (in [head, tail)) and consumer-owned.
        let v = unsafe { (*s.buf[head & s.mask].get()).assume_init_read() };
        s.head.store(head + 1, Ordering::Release);
        Some(v)
    }

    pub fn len(&self) -> usize {
        let s = &*self.shared;
        s.tail.load(Ordering::Acquire) - s.head.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_full() {
        let (mut p, mut c) = spsc::<u32>(4);
        assert_eq!(p.capacity(), 4);
        for i in 0..4 {
            p.push(i).unwrap();
        }
        assert_eq!(p.push(99), Err(99));
        assert_eq!(c.pop(), Some(0));
        p.push(4).unwrap();
        assert_eq!((1..5).map(|_| c.pop().unwrap()).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn drops_owned_values_left_inside() {
        use std::sync::atomic::AtomicUsize;
        static DROPS: AtomicUsize = AtomicUsize::new(0);
        struct D;
        impl Drop for D {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (mut p, mut c) = spsc::<D>(8);
        for _ in 0..5 {
            assert!(p.push(D).is_ok());
        }
        drop(c.pop());
        drop((p, c));
        assert_eq!(DROPS.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn threaded_ordering() {
        let (mut p, mut c) = spsc::<u64>(64);
        let n = 500_000u64;
        let t = std::thread::spawn(move || {
            let mut i = 0;
            while i < n {
                if p.push(i).is_ok() {
                    i += 1;
                }
            }
        });
        let mut expect = 0;
        while expect < n {
            if let Some(v) = c.pop() {
                assert_eq!(v, expect);
                expect += 1;
            }
        }
        t.join().unwrap();
    }
}
