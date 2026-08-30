//! The real-time audio graph: decks, channel strips, crossfader, FX, cue bus,
//! master. process() never allocates, locks, blocks, or panics.
//!
//! Phase 1 slice: four decks with Hermite playback, commands in over an SPSC
//! ring, a snapshot out through a triple buffer, retired tracks handed back
//! through a garbage ring, and the test tone folded into the master.
//! Design: `docs/02-AUDIO-ENGINE.md`.
#![forbid(unsafe_code)]

pub mod command;
pub mod deck;
pub mod engine;
pub mod fx;
pub mod jog;
pub mod mixer;
pub mod strip;
pub mod sync;

pub use command::{DeckSnap, EngineCommand, Snapshot};
pub use engine::{Engine, EngineHandles, ToneParams, DECKS};
