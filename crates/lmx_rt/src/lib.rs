//! Real-time-safe primitives: SPSC ring, triple buffer, atomic cells, deferred-drop boxes. The only pure crate allowed to use unsafe.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.

pub mod spsc;
pub mod triple;
pub mod atomic;
pub mod rtbox;
pub mod rtvec;
