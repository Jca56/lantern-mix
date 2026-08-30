//! Fast header-only probe: format, rate, channels, duration, metadata.

use crate::tags::Metadata;
use crate::{wav, CodecError, Result};
use std::io::Read;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Wav,
    Aiff,
    Flac,
    Mp3,
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub format: Format,
    pub sample_rate: u32,
    /// Channels in the *file* (output is always stereo).
    pub channels: u16,
    /// Bits per sample in the file (32 for float32, 64 for float64).
    pub bits: u16,
    pub duration_frames: u64,
    pub metadata: Metadata,
}

impl Probe {
    pub fn duration_secs(&self) -> f64 {
        self.duration_frames as f64 / self.sample_rate.max(1) as f64
    }
}

/// Identify a format from the first bytes of a file.
pub fn sniff(head: &[u8]) -> Option<Format> {
    if head.len() < 12 {
        return None;
    }
    match &head[0..4] {
        b"RIFF" | b"RF64" | b"BW64" if &head[8..12] == b"WAVE" => Some(Format::Wav),
        b"FORM" if &head[8..12] == b"AIFF" || &head[8..12] == b"AIFC" => Some(Format::Aiff),
        b"fLaC" => Some(Format::Flac),
        _ if &head[0..3] == b"ID3" => Some(Format::Mp3),
        _ if head[0] == 0xFF && (head[1] & 0xE0) == 0xE0 => Some(Format::Mp3),
        _ => None,
    }
}

pub fn sniff_path(path: &Path) -> Result<Format> {
    let mut head = [0u8; 12];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    sniff(&head[..n]).ok_or_else(|| CodecError::Unsupported(format!("unknown format: {}", path.display())))
}

/// Read headers and tags only.
pub fn probe(path: &Path) -> Result<Probe> {
    match sniff_path(path)? {
        Format::Wav => wav::probe(path),
        other => Err(CodecError::Unsupported(format!("{other:?} probing not implemented yet"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_magic() {
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), Some(Format::Wav));
        assert_eq!(sniff(b"RF64\0\0\0\0WAVEds64"), Some(Format::Wav));
        assert_eq!(sniff(b"FORM\0\0\0\0AIFFCOMM"), Some(Format::Aiff));
        assert_eq!(sniff(b"fLaC\0\0\0\"\0\0\0\0"), Some(Format::Flac));
        assert_eq!(sniff(b"ID3\x04\0\0\0\0\0\0\0\0"), Some(Format::Mp3));
        assert_eq!(sniff(b"\xFF\xFB\x90\0\0\0\0\0\0\0\0\0"), Some(Format::Mp3));
        assert_eq!(sniff(b"hello world!"), None);
        assert_eq!(sniff(b"RIFF"), None);
    }
}
