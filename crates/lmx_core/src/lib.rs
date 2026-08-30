//! Shared vocabulary for Lantern Mix: ids, musical/time types, hashing, errors, paths. No logic beyond conversions.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod ids;
pub mod time;
pub mod music;
pub mod hash;
pub mod paths;
pub mod audio;
pub mod error;

pub use audio::TrackAudio;
