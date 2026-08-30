//! Audio decoders (WAV, AIFF, FLAC; MP3 later) and tag readers. Never panics on
//! malformed input, never writes files.
//!
//! Output is always interleaved stereo f32 in −1..1: mono is duplicated, extra
//! channels are dropped (L/R only). Design: `docs/07-DECODERS.md`.
#![forbid(unsafe_code)]

pub mod aiff;
pub mod bits;
pub mod flac;
pub mod mp3;
pub mod probe;
pub mod tags;
pub mod wav;

use std::fmt;
use std::path::Path;

pub use probe::{probe, Format, Probe};
pub use tags::Metadata;

pub use lmx_core::TrackAudio;

#[derive(Debug)]
pub enum CodecError {
    Io(std::io::Error),
    /// Well-formed but something we don't decode (with the offending detail).
    Unsupported(String),
    /// Malformed file.
    Invalid(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Io(e) => write!(f, "i/o: {e}"),
            CodecError::Unsupported(s) => write!(f, "unsupported: {s}"),
            CodecError::Invalid(s) => write!(f, "invalid file: {s}"),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<std::io::Error> for CodecError {
    fn from(e: std::io::Error) -> Self {
        CodecError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, CodecError>;

pub(crate) fn invalid<T>(what: impl Into<String>) -> Result<T> {
    Err(CodecError::Invalid(what.into()))
}

/// Streaming decoder. `read` appends interleaved stereo frames to `out`.
pub trait Decoder: Send {
    fn info(&self) -> &Probe;
    /// Next frame `read` will produce.
    fn position(&self) -> u64;
    fn seek(&mut self, frame: u64) -> Result<()>;
    /// Decode up to `max_frames` frames, appending `2 * n` samples to `out`.
    /// Returns `n`; 0 means end of stream.
    fn read(&mut self, out: &mut Vec<f32>, max_frames: usize) -> Result<usize>;
}

/// Open a streaming decoder for any supported format.
pub fn open(path: &Path) -> Result<Box<dyn Decoder>> {
    match probe::sniff_path(path)? {
        Format::Wav => Ok(Box::new(wav::WavDecoder::open(path)?)),
        other => Err(CodecError::Unsupported(format!("{other:?} decoding not implemented yet"))),
    }
}

/// Decode a whole file. `progress` is called with 0..1 as frames arrive.
pub fn decode_all(path: &Path, mut progress: impl FnMut(f32)) -> Result<(TrackAudio, Metadata)> {
    let mut dec = open(path)?;
    let info = dec.info().clone();
    let total = info.duration_frames as usize;
    let mut frames: Vec<f32> = Vec::with_capacity(total.saturating_mul(2).min(1 << 31));
    let chunk = 65_536;
    loop {
        let n = dec.read(&mut frames, chunk)?;
        if n == 0 {
            break;
        }
        if total > 0 {
            progress((frames.len() / 2) as f32 / total as f32);
        }
    }
    progress(1.0);
    Ok((TrackAudio { sample_rate: info.sample_rate, channels: 2, frames }, info.metadata))
}
