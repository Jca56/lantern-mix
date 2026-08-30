//! Triple buffer: writer publishes whole snapshots, reader always sees the latest
//! complete one. Neither side ever waits. Used for engine → UI state.
//!
//! Three slots; one atomic holds the index of the "middle" slot plus a dirty bit.
//! Writer fills its private slot and swaps it into the middle; reader swaps the
//! middle out when dirty. Classic, wait-free, no allocation after construction.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const DIRTY: u8 = 0b100;
const IDX: u8 = 0b011;

struct Shared<T> {
    slots: [UnsafeCell<T>; 3],
    state: AtomicU8,
}

// SAFETY: each slot is only ever touched by one side at a time — ownership of a
// slot index moves between writer and reader through the atomic swap.
unsafe impl<T: Send> Sync for Shared<T> {}
unsafe impl<T: Send> Send for Shared<T> {}

pub struct TripleWriter<T> {
    shared: Arc<Shared<T>>,
    write_idx: u8,
}

pub struct TripleReader<T> {
    shared: Arc<Shared<T>>,
    read_idx: u8,
}

/// Create a triple buffer seeded with `init` in every slot.
pub fn triple<T: Copy>(init: T) -> (TripleWriter<T>, TripleReader<T>) {
    let shared = Arc::new(Shared {
        slots: [UnsafeCell::new(init), UnsafeCell::new(init), UnsafeCell::new(init)],
        state: AtomicU8::new(1), // middle = slot 1, clean
    });
    (TripleWriter { shared: shared.clone(), write_idx: 0 }, TripleReader { shared, read_idx: 2 })
}

impl<T: Copy> TripleWriter<T> {
    /// Publish a new snapshot. Wait-free.
    #[inline]
    pub fn write(&mut self, v: T) {
        // SAFETY: write_idx is owned by the writer until the swap below.
        unsafe { *self.shared.slots[self.write_idx as usize].get() = v };
        let old = self.shared.state.swap(self.write_idx | DIRTY, Ordering::AcqRel);
        self.write_idx = old & IDX;
    }
}

impl<T: Copy> TripleReader<T> {
    /// Latest published snapshot (or the previous one if nothing new). Wait-free.
    #[inline]
    pub fn read(&mut self) -> T {
        if self.shared.state.load(Ordering::Acquire) & DIRTY != 0 {
            let old = self.shared.state.swap(self.read_idx, Ordering::AcqRel);
            self.read_idx = old & IDX;
        }
        // SAFETY: read_idx is owned by the reader until the next swap.
        unsafe { *self.shared.slots[self.read_idx as usize].get() }
    }

    /// True if a snapshot newer than the last `read` is available.
    pub fn is_dirty(&self) -> bool {
        self.shared.state.load(Ordering::Acquire) & DIRTY != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_sees_latest_not_torn() {
        let (mut w, mut r) = triple([0u64; 4]);
        assert_eq!(r.read(), [0; 4]);
        w.write([1; 4]);
        w.write([2; 4]);
        assert!(r.is_dirty());
        assert_eq!(r.read(), [2; 4]);
        assert!(!r.is_dirty());
        assert_eq!(r.read(), [2; 4]);
    }

    #[test]
    fn threaded_snapshots_are_consistent() {
        let (mut w, mut r) = triple([0u32; 8]);
        let writer = std::thread::spawn(move || {
            for i in 1..200_000u32 {
                w.write([i; 8]);
            }
        });
        let mut last = 0;
        for _ in 0..500_000 {
            let v = r.read();
            assert!(v.iter().all(|x| *x == v[0]), "torn read");
            assert!(v[0] >= last, "went backwards");
            last = v[0];
        }
        writer.join().unwrap();
        assert_eq!(r.read(), [199_999; 8]);
    }
}
