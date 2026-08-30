//! DSP toolbox: FFT, biquads, SVF, YIN, smoothing, resampling, time-stretch,
//! metering. Pure functions and small state structs, allocation-free after
//! construction.
//!
//! Everything here is safe to call from the audio thread once constructed.
#![forbid(unsafe_code)]

pub mod biquad;
pub mod fft;
pub mod meter;
pub mod resample;
pub mod smooth;
pub mod stretch;
pub mod svf;
pub mod window;
pub mod yin;

pub use biquad::{Biquad, Coeffs, Mode};
pub use resample::hermite;
