//! RIFF/WAVE: PCM, float, EXTENSIBLE; LIST/INFO and embedded id3 chunks.
//!
//! Handles RF64/BW64 (`ds64`), odd-chunk padding, `data` sizes that lie (0 or
//! 0xFFFFFFFF from streaming writers) by clamping to the file, and an `acid`
//! tempo hint. Output is stereo f32 regardless of the file's channel count.

use crate::bits::Bytes;
use crate::probe::{Format, Probe};
use crate::tags::{self, Metadata};
use crate::{invalid, CodecError, Decoder, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sample {
    U8,
    I16,
    I24,
    I32,
    F32,
    F64,
}

#[derive(Clone, Debug)]
struct Fmt {
    sample: Sample,
    channels: u16,
    rate: u32,
    bits: u16,
    block_align: usize,
}

struct Header {
    fmt: Fmt,
    data_start: u64,
    data_len: u64,
    metadata: Metadata,
}

fn parse_fmt(body: &[u8]) -> Result<Fmt> {
    let mut b = Bytes::new(body);
    let mut tag = b.u16le()?;
    let channels = b.u16le()?;
    let rate = b.u32le()?;
    let _byte_rate = b.u32le()?;
    let block_align = b.u16le()? as usize;
    let bits = b.u16le()?;
    if tag == 0xFFFE {
        // WAVE_FORMAT_EXTENSIBLE: cbSize, valid bits, channel mask, subformat GUID
        let _cb = b.u16le()?;
        let _valid = b.u16le()?;
        let _mask = b.u32le()?;
        tag = b.u16le()?; // first two bytes of the GUID carry the real tag
    }
    if channels == 0 || rate == 0 {
        return invalid("fmt: zero channels or rate");
    }
    let sample = match (tag, bits) {
        (1, 8) => Sample::U8,
        (1, 16) => Sample::I16,
        (1, 24) => Sample::I24,
        (1, 32) => Sample::I32,
        (3, 32) => Sample::F32,
        (3, 64) => Sample::F64,
        (t, b) => return Err(CodecError::Unsupported(format!("WAV format tag {t} with {b} bits"))),
    };
    let block_align = if block_align == 0 { channels as usize * (bits as usize / 8) } else { block_align };
    if block_align < channels as usize * (bits as usize / 8) {
        return invalid("fmt: block align smaller than a frame");
    }
    Ok(Fmt { sample, channels, rate, bits, block_align })
}

/// Walk the chunk list. Stops cleanly at EOF or on a chunk that runs past it.
fn parse_header<R: Read + Seek>(r: &mut R) -> Result<Header> {
    let file_len = r.seek(SeekFrom::End(0))?;
    r.seek(SeekFrom::Start(0))?;
    let mut head = [0u8; 12];
    r.read_exact(&mut head)?;
    let is_rf64 = &head[0..4] == b"RF64" || &head[0..4] == b"BW64";
    if !(is_rf64 || &head[0..4] == b"RIFF") || &head[8..12] != b"WAVE" {
        return invalid("not a RIFF/WAVE file");
    }
    let mut fmt = None;
    let mut data: Option<(u64, u64)> = None;
    let mut ds64_data_len: Option<u64> = None;
    let mut metadata = Metadata::default();
    let mut pos = 12u64;
    let mut small = vec![0u8; 0];
    while pos + 8 <= file_len {
        r.seek(SeekFrom::Start(pos))?;
        let mut ch = [0u8; 8];
        r.read_exact(&mut ch)?;
        let id = [ch[0], ch[1], ch[2], ch[3]];
        let mut size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let body_start = pos + 8;
        let avail = file_len.saturating_sub(body_start);
        match &id {
            b"data" => {
                if is_rf64 && size == 0xFFFF_FFFF {
                    size = ds64_data_len.unwrap_or(avail);
                }
                if size == 0 || size == 0xFFFF_FFFF || size > avail {
                    size = avail; // streaming writers lie; trust the file
                }
                if data.is_none() {
                    data = Some((body_start, size));
                }
            }
            _ => {
                let take = size.min(avail).min(64 << 20) as usize; // never slurp >64 MB of metadata
                small.resize(take, 0);
                r.read_exact(&mut small)?;
                match &id {
                    b"fmt " => fmt = Some(parse_fmt(&small)?),
                    b"ds64" => {
                        let mut b = Bytes::new(&small);
                        let _riff = b.u64le()?;
                        ds64_data_len = Some(b.u64le()?);
                    }
                    b"LIST" if small.starts_with(b"INFO") => metadata.merge(tags::riff_info(&small[4..])),
                    b"id3 " | b"ID3 " => {
                        if let Ok(m) = tags::id3v2(&small) {
                            metadata.overlay(m);
                        }
                    }
                    b"acid" => {
                        let mut b = Bytes::new(&small);
                        if b.skip(20).is_ok() {
                            if let Ok(t) = b.f32le() {
                                if t > 0.0 && t < 1000.0 && metadata.bpm_tag.is_none() {
                                    metadata.bpm_tag = Some(t);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let padded = size + (size & 1);
        pos = body_start.saturating_add(padded);
    }
    let fmt = fmt.ok_or_else(|| CodecError::Invalid("no fmt chunk".into()))?;
    let (data_start, data_len) = data.ok_or_else(|| CodecError::Invalid("no data chunk".into()))?;
    Ok(Header { fmt, data_start, data_len, metadata })
}

fn header_probe(h: &Header) -> Probe {
    Probe {
        format: Format::Wav,
        sample_rate: h.fmt.rate,
        channels: h.fmt.channels,
        bits: h.fmt.bits,
        duration_frames: h.data_len / h.fmt.block_align as u64,
        metadata: h.metadata.clone(),
    }
}

/// Headers and tags only.
pub fn probe(path: &Path) -> Result<Probe> {
    let mut r = BufReader::new(File::open(path)?);
    Ok(header_probe(&parse_header(&mut r)?))
}

pub struct WavDecoder {
    r: BufReader<File>,
    info: Probe,
    fmt: Fmt,
    data_start: u64,
    frames: u64,
    pos: u64,
    block: Vec<u8>,
}

impl WavDecoder {
    pub fn open(path: &Path) -> Result<Self> {
        let mut r = BufReader::new(File::open(path)?);
        let h = parse_header(&mut r)?;
        let info = header_probe(&h);
        let frames = info.duration_frames;
        r.seek(SeekFrom::Start(h.data_start))?;
        Ok(Self { r, info, fmt: h.fmt, data_start: h.data_start, frames, pos: 0, block: Vec::new() })
    }
}

impl Decoder for WavDecoder {
    fn info(&self) -> &Probe {
        &self.info
    }

    fn position(&self) -> u64 {
        self.pos
    }

    fn seek(&mut self, frame: u64) -> Result<()> {
        let frame = frame.min(self.frames);
        self.r.seek(SeekFrom::Start(self.data_start + frame * self.fmt.block_align as u64))?;
        self.pos = frame;
        Ok(())
    }

    fn read(&mut self, out: &mut Vec<f32>, max_frames: usize) -> Result<usize> {
        let left = (self.frames - self.pos) as usize;
        let n = max_frames.min(left);
        if n == 0 {
            return Ok(0);
        }
        let ba = self.fmt.block_align;
        self.block.resize(n * ba, 0);
        // A short read at the very end (file shorter than the header claims)
        // just ends the stream.
        let mut got = 0;
        while got < n * ba {
            let k = self.r.read(&mut self.block[got..])?;
            if k == 0 {
                break;
            }
            got += k;
        }
        let frames = got / ba;
        if frames == 0 {
            self.pos = self.frames;
            return Ok(0);
        }
        convert(&self.fmt, &self.block[..frames * ba], out);
        self.pos += frames as u64;
        Ok(frames)
    }
}

/// Convert raw frames to interleaved stereo f32, appending to `out`.
fn convert(fmt: &Fmt, raw: &[u8], out: &mut Vec<f32>) {
    let ba = fmt.block_align;
    let bps = (fmt.bits / 8) as usize;
    let ch = fmt.channels as usize;
    out.reserve(raw.len() / ba * 2);
    for frame in raw.chunks_exact(ba) {
        let s = |c: usize| -> f32 {
            let o = c * bps;
            let b = &frame[o..o + bps];
            match fmt.sample {
                Sample::U8 => (b[0] as f32 - 128.0) / 128.0,
                Sample::I16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
                Sample::I24 => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
                Sample::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
                Sample::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                Sample::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32,
            }
        };
        let l = s(0);
        let r = if ch >= 2 { s(1) } else { l };
        out.push(l);
        out.push(r);
    }
}

#[cfg(test)]
mod tests {
    use crate::decode_all;

    /// Minimal WAV writer for fixtures. `bits`: 8/16/24/32 PCM, 32f/64f float.
    pub(crate) fn make_wav(channels: u16, rate: u32, kind: &str, samples: &[f32], extra: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let (tag, bits): (u16, u16) = match kind {
            "8" => (1, 8),
            "16" => (1, 16),
            "24" => (1, 24),
            "32" => (1, 32),
            "32f" => (3, 32),
            "64f" => (3, 64),
            "ext24" => (0xFFFE, 24),
            _ => panic!("kind"),
        };
        let bps = bits as usize / 8;
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&tag.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * channels as u32 * bps as u32).to_le_bytes());
        fmt.extend_from_slice(&((channels as usize * bps) as u16).to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if tag == 0xFFFE {
            fmt.extend_from_slice(&22u16.to_le_bytes());
            fmt.extend_from_slice(&bits.to_le_bytes());
            fmt.extend_from_slice(&0u32.to_le_bytes());
            fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM sub-format
            fmt.extend_from_slice(&[0u8; 14]);
        }
        let mut data = Vec::new();
        for s in samples {
            match kind {
                "8" => data.push(((s * 127.0).round() + 128.0) as u8),
                "16" => data.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes()),
                "24" | "ext24" => {
                    let v = (s * 8_388_607.0).round() as i32;
                    data.extend_from_slice(&v.to_le_bytes()[..3]);
                }
                "32" => data.extend_from_slice(&((*s as f64 * 2_147_483_647.0).round() as i32).to_le_bytes()),
                "32f" => data.extend_from_slice(&s.to_le_bytes()),
                "64f" => data.extend_from_slice(&(*s as f64).to_le_bytes()),
                _ => unreachable!(),
            }
        }
        let mut chunks: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"fmt ", fmt)];
        chunks.extend(extra.iter().map(|(i, v)| (*i, v.clone())));
        chunks.push((b"data", data));
        let mut body = b"WAVE".to_vec();
        for (id, v) in &chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(v.len() as u32).to_le_bytes());
            body.extend_from_slice(v);
            if v.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut f = b"RIFF".to_vec();
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend_from_slice(&body);
        f
    }

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("lmx_codec_{}_{}", std::process::id(), name));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn ramp(n: usize) -> Vec<f32> {
        (0..n).map(|i| ((i as f32 / n as f32) * 2.0 - 1.0) * 0.9).collect()
    }

    #[test]
    fn roundtrip_every_sample_format() {
        let src = ramp(200); // 100 stereo frames
        for (kind, tol) in [("8", 1.0 / 100.0), ("16", 1.0 / 30000.0), ("24", 1e-6), ("ext24", 1e-6), ("32", 1e-7), ("32f", 0.0), ("64f", 1e-7)] {
            let p = tmp(kind, &make_wav(2, 44_100, kind, &src, &[]));
            let (audio, _) = decode_all(&p, |_| {}).unwrap();
            assert_eq!(audio.sample_rate, 44_100);
            assert_eq!(audio.frame_count(), 100, "{kind}");
            for (a, b) in audio.frames.iter().zip(&src) {
                assert!((a - b).abs() <= tol, "{kind}: {a} vs {b}");
            }
            std::fs::remove_file(p).ok();
        }
    }

    #[test]
    fn mono_duplicates_and_six_channels_keep_lr() {
        let mono = ramp(50);
        let p = tmp("mono", &make_wav(1, 48_000, "16", &mono, &[]));
        let (a, _) = decode_all(&p, |_| {}).unwrap();
        assert_eq!(a.frame_count(), 50);
        assert_eq!(a.frames[0], a.frames[1]);
        std::fs::remove_file(p).ok();

        let six: Vec<f32> = (0..60).map(|i| (i % 6) as f32 * 0.1).collect();
        let p = tmp("six", &make_wav(6, 48_000, "32f", &six, &[]));
        let (a, _) = decode_all(&p, |_| {}).unwrap();
        assert_eq!(a.frame_count(), 10);
        assert_eq!(&a.frames[..4], &[0.0, 0.1, 0.0, 0.1]);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn metadata_from_info_and_id3_with_id3_winning() {
        let mut info = b"INFO".to_vec();
        info.extend_from_slice(b"INAM");
        info.extend_from_slice(&9u32.to_le_bytes());
        info.extend_from_slice(b"InfoTitle\0"); // odd → padded
        info.extend_from_slice(b"IART");
        info.extend_from_slice(&4u32.to_le_bytes());
        info.extend_from_slice(b"Alva");
        let mut id3 = b"ID3\x03\x00\x00".to_vec();
        let frame = {
            let mut v = b"TIT2".to_vec();
            v.extend_from_slice(&9u32.to_be_bytes());
            v.extend_from_slice(&[0, 0]);
            v.extend_from_slice(b"\x03Id3Title");
            v
        };
        let n = frame.len() as u32;
        id3.extend_from_slice(&[(n >> 21) as u8, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
        id3.extend_from_slice(&frame);
        let mut acid = vec![0u8; 20];
        acid.extend_from_slice(&140.0f32.to_le_bytes());
        let p = tmp("meta", &make_wav(2, 44_100, "16", &ramp(20), &[(b"LIST", info), (b"id3 ", id3), (b"acid", acid)]));
        let pr = crate::probe(&p).unwrap();
        assert_eq!(pr.metadata.title.as_deref(), Some("Id3Title"));
        assert_eq!(pr.metadata.artist.as_deref(), Some("Alva"));
        assert_eq!(pr.metadata.bpm_tag, Some(140.0));
        assert_eq!(pr.duration_frames, 10);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn lying_data_size_and_truncated_file_clamp() {
        let mut bytes = make_wav(2, 44_100, "16", &ramp(40), &[]);
        let dpos = bytes.windows(4).position(|w| w == b"data").unwrap();
        bytes[dpos + 4..dpos + 8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let p = tmp("lie", &bytes);
        let (a, _) = decode_all(&p, |_| {}).unwrap();
        assert_eq!(a.frame_count(), 20);
        std::fs::remove_file(p).ok();

        let full = make_wav(2, 44_100, "16", &ramp(40), &[]);
        let p = tmp("trunc", &full[..full.len() - 10]);
        let (a, _) = decode_all(&p, |_| {}).unwrap();
        assert_eq!(a.frame_count(), 17); // 20 frames − 10 bytes = 2.5 frames lost
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn seek_and_streaming_reads() {
        let p = tmp("seek", &make_wav(2, 44_100, "32f", &ramp(2000), &[]));
        let mut d = crate::open(&p).unwrap();
        let mut out = Vec::new();
        assert_eq!(d.read(&mut out, 300).unwrap(), 300);
        d.seek(900).unwrap();
        out.clear();
        assert_eq!(d.read(&mut out, 1000).unwrap(), 100);
        assert_eq!(d.position(), 1000);
        assert_eq!(d.read(&mut out, 10).unwrap(), 0);
        let expect = ramp(2000)[1800];
        assert_eq!(out[0], expect);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn fuzzed_inputs_never_panic() {
        let base = make_wav(2, 44_100, "24", &ramp(64), &[(b"LIST", b"INFOINAM\x04\0\0\0abcd".to_vec())]);
        let mut seed = 0x9E37_79B9u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for i in 0..400 {
            let mut b = base.clone();
            for _ in 0..(1 + i % 8) {
                let at = next() as usize % b.len();
                b[at] = next() as u8;
            }
            if i % 5 == 0 {
                let cut = next() as usize % b.len();
                b.truncate(cut);
            }
            let p = tmp(&format!("fuzz{i}"), &b);
            let _ = decode_all(&p, |_| {});
            std::fs::remove_file(p).ok();
        }
    }
}
