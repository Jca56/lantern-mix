//! Track analysis: beatgrid/BPM, key, waveform summaries, loudness. Job-shaped, cancellable, runs on worker threads.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod stft;
pub mod onset;
pub mod tempo;
pub mod key;
pub mod waveform;
pub mod loudness;
pub mod job;
