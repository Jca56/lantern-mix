//! Track analysis: beatgrid/BPM, key, waveform summaries, loudness. Job-shaped,
//! cancellable, runs on worker threads.
//!
//! Design: `docs/03-ANALYSIS.md`.
#![forbid(unsafe_code)]

pub mod job;
pub mod key;
pub mod loudness;
pub mod onset;
pub mod stft;
pub mod tempo;
pub mod waveform;

pub use waveform::{WaveformSummary, FINE_FRAMES, OVERVIEW_FRAMES};
