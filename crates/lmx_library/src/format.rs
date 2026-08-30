//! Tag-length-value binary encoding with forward/backward compatibility.
//!
//! Every value is `tag: u16, len: u32, bytes`. Readers skip tags they don't
//! know; writers may add tags freely. Nested records are values whose bytes
//! are another TLV sequence. Files carry a magic, a version and a CRC32.

use std::io::{self, Read, Write};

pub struct Writer {
    buf: Vec<u8>,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
    fn entry(&mut self, tag: u16, bytes: &[u8]) {
        self.buf.extend_from_slice(&tag.to_le_bytes());
        self.buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(bytes);
    }
    pub fn u8(&mut self, tag: u16, v: u8) {
        self.entry(tag, &[v]);
    }
    pub fn bool(&mut self, tag: u16, v: bool) {
        self.entry(tag, &[v as u8]);
    }
    pub fn u32(&mut self, tag: u16, v: u32) {
        self.entry(tag, &v.to_le_bytes());
    }
    pub fn u64(&mut self, tag: u16, v: u64) {
        self.entry(tag, &v.to_le_bytes());
    }
    pub fn f32(&mut self, tag: u16, v: f32) {
        self.entry(tag, &v.to_le_bytes());
    }
    pub fn f64(&mut self, tag: u16, v: f64) {
        self.entry(tag, &v.to_le_bytes());
    }
    pub fn str(&mut self, tag: u16, v: &str) {
        self.entry(tag, v.as_bytes());
    }
    pub fn blob(&mut self, tag: u16, inner: Writer) {
        self.entry(tag, &inner.buf);
    }
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// One decoded entry.
#[derive(Clone, Copy)]
pub struct Entry<'a> {
    pub tag: u16,
    pub bytes: &'a [u8],
}

impl<'a> Entry<'a> {
    pub fn u8(&self) -> Option<u8> {
        self.bytes.first().copied()
    }
    pub fn bool(&self) -> Option<bool> {
        self.u8().map(|v| v != 0)
    }
    pub fn u32(&self) -> Option<u32> {
        self.bytes.try_into().ok().map(u32::from_le_bytes)
    }
    pub fn u64(&self) -> Option<u64> {
        self.bytes.try_into().ok().map(u64::from_le_bytes)
    }
    pub fn f32(&self) -> Option<f32> {
        self.bytes.try_into().ok().map(f32::from_le_bytes)
    }
    pub fn f64(&self) -> Option<f64> {
        self.bytes.try_into().ok().map(f64::from_le_bytes)
    }
    pub fn str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes).ok()
    }
    pub fn entries(&self) -> Entries<'a> {
        Entries { data: self.bytes, pos: 0 }
    }
}

/// Iterator over a TLV sequence. Stops (without error) at a truncated entry.
pub struct Entries<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Entries<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Entry<'a>;
    fn next(&mut self) -> Option<Entry<'a>> {
        let d = self.data;
        if self.pos + 6 > d.len() {
            return None;
        }
        let tag = u16::from_le_bytes([d[self.pos], d[self.pos + 1]]);
        let len = u32::from_le_bytes([d[self.pos + 2], d[self.pos + 3], d[self.pos + 4], d[self.pos + 5]]) as usize;
        let start = self.pos + 6;
        if start + len > d.len() {
            return None;
        }
        self.pos = start + len;
        Some(Entry { tag, bytes: &d[start..start + len] })
    }
}

// ── CRC32 (IEEE) ─────────────────────────────────────────────────────────

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, e) in t.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    t
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for b in bytes {
        c = t[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// ── file container ───────────────────────────────────────────────────────

pub const MAGIC: &[u8; 4] = b"LMX1";

/// `magic · version u32 · body · crc32(body)`.
pub fn write_container(w: &mut impl Write, version: u32, body: &[u8]) -> io::Result<()> {
    w.write_all(MAGIC)?;
    w.write_all(&version.to_le_bytes())?;
    w.write_all(body)?;
    w.write_all(&crc32(body).to_le_bytes())?;
    Ok(())
}

/// Returns (version, body) or an error if magic/crc don't check out.
pub fn read_container(r: &mut impl Read) -> io::Result<(u32, Vec<u8>)> {
    let mut all = Vec::new();
    r.read_to_end(&mut all)?;
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
    if all.len() < 12 || &all[..4] != MAGIC {
        return Err(bad("not a Lantern Mix file"));
    }
    let version = u32::from_le_bytes([all[4], all[5], all[6], all[7]]);
    let body = &all[8..all.len() - 4];
    let stored = u32::from_le_bytes([all[all.len() - 4], all[all.len() - 3], all[all.len() - 2], all[all.len() - 1]]);
    if crc32(body) != stored {
        return Err(bad("checksum mismatch"));
    }
    Ok((version, body.to_vec()))
}

/// Journal record: `len u32 · body · crc32(body)`.
pub fn write_record(w: &mut impl Write, body: &[u8]) -> io::Result<()> {
    w.write_all(&(body.len() as u32).to_le_bytes())?;
    w.write_all(body)?;
    w.write_all(&crc32(body).to_le_bytes())?;
    Ok(())
}

/// All intact records, stopping at the first torn/corrupt one.
pub fn read_records(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= data.len() {
        let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let start = pos + 4;
        let end = start + len;
        if end + 4 > data.len() {
            break;
        }
        let body = &data[start..end];
        let stored = u32::from_le_bytes([data[end], data[end + 1], data[end + 2], data[end + 3]]);
        if crc32(body) != stored {
            break;
        }
        out.push(body);
        pos = end + 4;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlv_roundtrip_skips_unknown() {
        let mut w = Writer::new();
        w.u64(1, 42);
        w.str(2, "héllo");
        w.f64(99, 1.5); // unknown to the reader below
        let mut inner = Writer::new();
        inner.bool(1, true);
        w.blob(3, inner);
        let bytes = w.finish();
        let mut got = (0u64, String::new(), false);
        for e in Entries::new(&bytes) {
            match e.tag {
                1 => got.0 = e.u64().unwrap(),
                2 => got.1 = e.str().unwrap().to_string(),
                3 => {
                    for i in e.entries() {
                        if i.tag == 1 {
                            got.2 = i.bool().unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
        assert_eq!(got, (42, "héllo".to_string(), true));
        // truncated sequence yields what's intact, no panic
        assert_eq!(Entries::new(&bytes[..bytes.len() - 3]).count(), 3);
    }

    #[test]
    fn crc_known_value_and_containers() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let mut file = Vec::new();
        write_container(&mut file, 7, b"body").unwrap();
        let (v, body) = read_container(&mut &file[..]).unwrap();
        assert_eq!((v, body.as_slice()), (7, &b"body"[..]));
        file[10] ^= 1;
        assert!(read_container(&mut &file[..]).is_err());

        let mut j = Vec::new();
        write_record(&mut j, b"one").unwrap();
        write_record(&mut j, b"two").unwrap();
        let cut = j.len() - 2; // torn last record
        assert_eq!(read_records(&j[..cut]), vec![&b"one"[..]]);
        assert_eq!(read_records(&j).len(), 2);
    }
}
