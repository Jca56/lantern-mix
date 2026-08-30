//! Audio decoders (WAV, AIFF, FLAC; MP3 later) and tag readers. Never panics on malformed input, never writes files.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod wav;
pub mod aiff;
pub mod flac;
pub mod mp3;
pub mod tags;
pub mod bits;
pub mod probe;
