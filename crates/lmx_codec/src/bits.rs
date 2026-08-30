//! Bit readers (MSB-first for FLAC/MP3) and endian helpers.
//!
//! `Bytes` is a bounds-checked cursor over a byte slice: every read returns
//! `Err(Invalid)` past the end instead of panicking, which is the whole reason
//! decoders never index slices directly.

use crate::{invalid, Result};

#[derive(Clone, Copy)]
pub struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return invalid("unexpected end of data");
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.data[self.pos.min(self.data.len())..]
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16le(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u16be(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn u32le(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u32be(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64le(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    pub fn f32le(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32le()?))
    }
    pub fn fourcc(&mut self) -> Result<[u8; 4]> {
        let b = self.take(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }
    /// ID3 "syncsafe" 28-bit integer (7 bits per byte, big-endian).
    pub fn syncsafe(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(((b[0] as u32 & 0x7f) << 21) | ((b[1] as u32 & 0x7f) << 14) | ((b[2] as u32 & 0x7f) << 7) | (b[3] as u32 & 0x7f))
    }
}

/// Decode `n` bytes of Latin-1 to a String, trimming NULs and whitespace.
pub fn latin1(b: &[u8]) -> String {
    b.iter().take_while(|c| **c != 0).map(|c| *c as char).collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_refuses_overrun() {
        let d = [1u8, 0, 2, 0, 0, 0, 0x7f, 0x7f, 0x7f, 0x7f];
        let mut b = Bytes::new(&d);
        assert_eq!(b.u16le().unwrap(), 1);
        assert_eq!(b.u32le().unwrap(), 2);
        assert_eq!(b.syncsafe().unwrap(), 0x0FFF_FFFF);
        assert!(b.u8().is_err());
        assert!(b.is_empty());
    }
}
