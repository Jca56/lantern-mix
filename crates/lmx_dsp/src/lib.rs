//! DSP toolbox: FFT, biquads, SVF, YIN, smoothing, resampling, time-stretch, metering. Pure functions and small state structs, allocation-free after construction.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod fft;
pub mod biquad;
pub mod svf;
pub mod yin;
pub mod smooth;
pub mod resample;
pub mod stretch;
pub mod window;
pub mod meter;
