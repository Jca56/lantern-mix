//! The real-time audio graph: decks, channel strips, crossfader, FX, cue bus, master. process() never allocates, locks, blocks, or panics.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod engine;
pub mod deck;
pub mod jog;
pub mod strip;
pub mod mixer;
pub mod fx;
pub mod sync;
pub mod command;
