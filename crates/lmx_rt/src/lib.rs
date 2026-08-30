//! Real-time-safe primitives: SPSC ring, triple buffer, atomic cells, deferred-drop
//! boxes. The only pure crate allowed to use unsafe.
//!
//! Contract for everything here: no allocation after construction, no locks, no
//! syscalls, no blocking on either side. Design: `docs/01-ARCHITECTURE.md` (Messaging).

pub mod atomic;
pub mod rtbox;
pub mod rtvec;
pub mod spsc;
pub mod triple;
pub mod wake;

pub use atomic::{AtomicF32, AtomicF64};
pub use spsc::{spsc, Consumer, Producer};
pub use triple::{triple, TripleReader, TripleWriter};
pub use wake::{wake_pair, WakeNotifier, WakeWaiter};
