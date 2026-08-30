//! Shared Metadata + readers: id3v2, vorbis_comment, riff_info, aiff_text.
//!
//! Readers only fill fields they find; `Metadata::merge` layers sources so a
//! WAV with both an INFO list and an `id3 ` chunk gets the union (ID3 wins).

use crate::bits::{latin1, Bytes};
use crate::Result;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub comment: Option<String>,
    pub year: Option<u16>,
    /// BPM written by another program (a hint, never trusted over analysis).
    pub bpm_tag: Option<f32>,
    /// Key as written by another program ("Am", "8A", …), unparsed.
    pub key_tag: Option<String>,
    /// Cover art bytes (PNG/JPEG) if embedded.
    pub artwork: Option<Vec<u8>>,
}

impl Metadata {
    /// Take every field `other` has that `self` lacks.
    pub fn merge(&mut self, other: Metadata) {
        fn fill<T>(a: &mut Option<T>, b: Option<T>) {
            if a.is_none() {
                *a = b;
            }
        }
        fill(&mut self.title, other.title);
        fill(&mut self.artist, other.artist);
        fill(&mut self.album, other.album);
        fill(&mut self.genre, other.genre);
        fill(&mut self.label, other.label);
        fill(&mut self.comment, other.comment);
        fill(&mut self.year, other.year);
        fill(&mut self.bpm_tag, other.bpm_tag);
        fill(&mut self.key_tag, other.key_tag);
        fill(&mut self.artwork, other.artwork);
    }

    /// Prefer `other`'s fields where present (used to let ID3 win over INFO).
    pub fn overlay(&mut self, other: Metadata) {
        let mut o = other;
        o.merge(std::mem::take(self));
        *self = o;
    }
}

fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

fn parse_year(s: &str) -> Option<u16> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u16>().ok().filter(|y| *y > 1000)
}

// ── RIFF LIST/INFO ───────────────────────────────────────────────────────────

/// Parse the body of a `LIST` chunk of type `INFO` (after the "INFO" fourcc).
pub fn riff_info(body: &[u8]) -> Metadata {
    let mut m = Metadata::default();
    let mut b = Bytes::new(body);
    while b.remaining() >= 8 {
        let Ok(id) = b.fourcc() else { break };
        let Ok(size) = b.u32le() else { break };
        let size = size as usize;
        let Ok(data) = b.take(size.min(b.remaining())) else { break };
        let text = non_empty(latin1_or_utf8(data));
        match &id {
            b"INAM" => m.title = text,
            b"IART" => m.artist = text,
            b"IPRD" => m.album = text,
            b"IGNR" => m.genre = text,
            b"ICMT" => m.comment = text,
            b"ICRD" => m.year = text.as_deref().and_then(parse_year),
            _ => {}
        }
        if size % 2 == 1 {
            let _ = b.skip(1);
        }
    }
    m
}

/// INFO text is nominally Latin-1 but modern writers emit UTF-8; prefer UTF-8
/// when it validates.
fn latin1_or_utf8(data: &[u8]) -> String {
    let end = data.iter().position(|c| *c == 0).unwrap_or(data.len());
    match std::str::from_utf8(&data[..end]) {
        Ok(s) => s.to_string(),
        Err(_) => latin1(data),
    }
}

// ── ID3v2 ────────────────────────────────────────────────────────────────────

/// Parse an ID3v2.2/2.3/2.4 tag starting at the "ID3" header. Returns what it
/// could read; a truncated tag yields partial metadata, not an error.
pub fn id3v2(data: &[u8]) -> Result<Metadata> {
    let mut m = Metadata::default();
    let mut b = Bytes::new(data);
    if b.take(3)? != b"ID3" {
        return crate::invalid("not an ID3v2 tag");
    }
    let major = b.u8()?;
    let _rev = b.u8()?;
    let flags = b.u8()?;
    let size = b.syncsafe()? as usize;
    let body_raw = b.take(size.min(b.remaining()))?;
    let unsync_all = flags & 0x80 != 0 && major < 4;
    let owned;
    let mut body: &[u8] = if unsync_all {
        owned = de_unsync(body_raw);
        &owned
    } else {
        body_raw
    };
    if flags & 0x40 != 0 {
        // extended header: skip it
        let mut e = Bytes::new(body);
        let ext = if major >= 4 { e.syncsafe()? as usize } else { e.u32be()? as usize + 4 };
        body = &body[ext.min(body.len())..];
    }
    let mut f = Bytes::new(body);
    loop {
        let (id, fsize, fflags) = if major == 2 {
            if f.remaining() < 6 {
                break;
            }
            let id = f.take(3)?;
            let s = f.take(3)?;
            let size = ((s[0] as usize) << 16) | ((s[1] as usize) << 8) | s[2] as usize;
            (map_v22(id), size, 0u16)
        } else {
            if f.remaining() < 10 {
                break;
            }
            let id = f.fourcc()?;
            let size = if major >= 4 { f.syncsafe()? as usize } else { f.u32be()? as usize };
            let fl = f.u16be()?;
            (id, size, fl)
        };
        if id == [0; 4] || id[0] == 0 {
            break; // padding
        }
        let Ok(raw) = f.take(fsize.min(f.remaining())) else { break };
        let frame_unsync = major >= 4 && fflags & 0x0002 != 0;
        let has_dli = major >= 4 && fflags & 0x0001 != 0;
        let owned_frame;
        let mut payload: &[u8] = raw;
        if has_dli {
            payload = &payload[4.min(payload.len())..];
        }
        if frame_unsync {
            owned_frame = de_unsync(payload);
            payload = &owned_frame;
        }
        apply_frame(&mut m, &id, payload);
        if f.remaining() == 0 {
            break;
        }
    }
    Ok(m)
}

fn map_v22(id: &[u8]) -> [u8; 4] {
    match id {
        b"TT2" => *b"TIT2",
        b"TP1" => *b"TPE1",
        b"TAL" => *b"TALB",
        b"TCO" => *b"TCON",
        b"TYE" => *b"TYER",
        b"TBP" => *b"TBPM",
        b"TKE" => *b"TKEY",
        b"TPB" => *b"TPUB",
        b"COM" => *b"COMM",
        b"PIC" => *b"APIC",
        _ => [id[0], id[1], id[2], b' '],
    }
}

fn apply_frame(m: &mut Metadata, id: &[u8; 4], p: &[u8]) {
    match id {
        b"TIT2" => m.title = text_frame(p),
        b"TPE1" => m.artist = text_frame(p),
        b"TALB" => m.album = text_frame(p),
        b"TCON" => m.genre = text_frame(p).map(clean_genre),
        b"TPUB" => m.label = text_frame(p),
        b"TYER" | b"TDRC" | b"TDRL" => {
            if m.year.is_none() {
                m.year = text_frame(p).as_deref().and_then(parse_year);
            }
        }
        b"TBPM" => m.bpm_tag = text_frame(p).and_then(|s| s.trim().parse::<f32>().ok()).filter(|b| *b > 0.0 && *b < 1000.0),
        b"TKEY" => m.key_tag = text_frame(p),
        b"COMM" => m.comment = comm_frame(p),
        b"APIC" => m.artwork = apic_frame(p),
        _ => {}
    }
}

/// Text frame: encoding byte + string(s); returns the first string.
fn text_frame(p: &[u8]) -> Option<String> {
    let (enc, rest) = p.split_first()?;
    let (s, _) = decode_string(*enc, rest);
    non_empty(s)
}

/// COMM: encoding, language(3), short description (terminated), text.
fn comm_frame(p: &[u8]) -> Option<String> {
    let (enc, rest) = p.split_first()?;
    let rest = rest.get(3..)?;
    let (_desc, after) = decode_string(*enc, rest);
    let (text, _) = decode_string(*enc, after);
    non_empty(text)
}

/// APIC: encoding, mime (nul-terminated latin1), picture type, description
/// (terminated in `encoding`), picture data.
fn apic_frame(p: &[u8]) -> Option<Vec<u8>> {
    let (enc, rest) = p.split_first()?;
    let mime_end = rest.iter().position(|c| *c == 0)?;
    let rest = rest.get(mime_end + 1..)?;
    let rest = rest.get(1..)?; // picture type
    let (_desc, data) = decode_string(*enc, rest);
    if data.is_empty() { None } else { Some(data.to_vec()) }
}

/// Decode one string in the given ID3 encoding; returns (string, bytes after
/// its terminator). Encodings: 0 Latin-1, 1 UTF-16 with BOM, 2 UTF-16BE, 3 UTF-8.
fn decode_string(enc: u8, d: &[u8]) -> (String, &[u8]) {
    match enc {
        1 | 2 => {
            let (be, body) = if enc == 1 {
                match d {
                    [0xFF, 0xFE, rest @ ..] => (false, rest),
                    [0xFE, 0xFF, rest @ ..] => (true, rest),
                    _ => (true, d),
                }
            } else {
                (true, d)
            };
            let mut units = Vec::with_capacity(body.len() / 2);
            let mut i = 0;
            while i + 1 < body.len() {
                let u = if be { u16::from_be_bytes([body[i], body[i + 1]]) } else { u16::from_le_bytes([body[i], body[i + 1]]) };
                i += 2;
                if u == 0 {
                    break;
                }
                units.push(u);
            }
            (String::from_utf16_lossy(&units), &body[i.min(body.len())..])
        }
        3 => {
            let end = d.iter().position(|c| *c == 0).unwrap_or(d.len());
            (String::from_utf8_lossy(&d[..end]).into_owned(), &d[(end + 1).min(d.len())..])
        }
        _ => {
            let end = d.iter().position(|c| *c == 0).unwrap_or(d.len());
            (latin1(&d[..end]), &d[(end + 1).min(d.len())..])
        }
    }
}

/// Undo ID3 unsynchronisation: every `FF 00` becomes `FF`.
fn de_unsync(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len());
    let mut i = 0;
    while i < d.len() {
        out.push(d[i]);
        if d[i] == 0xFF && i + 1 < d.len() && d[i + 1] == 0x00 {
            i += 1;
        }
        i += 1;
    }
    out
}

/// "(17)Rock" / "(17)" → "Rock" / "17".
fn clean_genre(s: String) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix('(') {
        if let Some(close) = rest.find(')') {
            let after = rest[close + 1..].trim();
            if !after.is_empty() {
                return after.to_string();
            }
            return rest[..close].to_string();
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(payload);
        v
    }

    fn tag(major: u8, frames: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = frames.concat();
        let n = body.len() as u32;
        let mut v = b"ID3".to_vec();
        v.extend_from_slice(&[major, 0, 0]);
        v.extend_from_slice(&[(n >> 21) as u8 & 0x7f, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn v23_text_frames_in_three_encodings() {
        let mut t_utf16 = vec![1u8, 0xFF, 0xFE];
        for u in "Bangarang".encode_utf16() {
            t_utf16.extend_from_slice(&u.to_le_bytes());
        }
        let frames = [
            frame(b"TIT2", &t_utf16),
            frame(b"TPE1", b"\x03Skrillex"),
            frame(b"TALB", b"\x00Album\xE9"),
            frame(b"TBPM", b"\x00140"),
            frame(b"TCON", b"\x00(52)Electronic"),
            frame(b"TYER", b"\x002011"),
            frame(b"COMM", b"\x00engdesc\x00the comment"),
        ];
        let m = id3v2(&tag(3, &frames)).unwrap();
        assert_eq!(m.title.as_deref(), Some("Bangarang"));
        assert_eq!(m.artist.as_deref(), Some("Skrillex"));
        assert_eq!(m.album.as_deref(), Some("Albumé"));
        assert_eq!(m.bpm_tag, Some(140.0));
        assert_eq!(m.genre.as_deref(), Some("Electronic"));
        assert_eq!(m.year, Some(2011));
        assert_eq!(m.comment.as_deref(), Some("the comment"));
    }

    #[test]
    fn v24_syncsafe_sizes_and_apic() {
        let mut pic = b"\x00image/png\x00\x03\x00".to_vec();
        pic.extend_from_slice(&[0x89, b'P', b'N', b'G']);
        let f_title = {
            let mut v = b"TIT2".to_vec();
            let n = 6u32;
            v.extend_from_slice(&[(n >> 21) as u8, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
            v.extend_from_slice(&[0, 0]);
            v.extend_from_slice(b"\x03Hello");
            v
        };
        let f_pic = {
            let mut v = b"APIC".to_vec();
            let n = pic.len() as u32;
            v.extend_from_slice(&[(n >> 21) as u8, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
            v.extend_from_slice(&[0, 0]);
            v.extend_from_slice(&pic);
            v
        };
        let m = id3v2(&tag(4, &[f_title, f_pic])).unwrap();
        assert_eq!(m.title.as_deref(), Some("Hello"));
        assert_eq!(m.artwork.as_deref(), Some(&[0x89, b'P', b'N', b'G'][..]));
    }

    #[test]
    fn truncated_tag_is_partial_not_panic() {
        let full = tag(3, &[frame(b"TIT2", b"\x03Title"), frame(b"TPE1", b"\x03Artist")]);
        for cut in 0..full.len() {
            let _ = id3v2(&full[..cut]);
        }
        let m = id3v2(&full[..full.len() - 3]).unwrap();
        assert_eq!(m.title.as_deref(), Some("Title"));
    }

    #[test]
    fn riff_info_list() {
        let mut body = b"INAM".to_vec();
        body.extend_from_slice(&5u32.to_le_bytes());
        body.extend_from_slice(b"Wub\0\0\0"); // 5 bytes + pad
        body.extend_from_slice(b"IART");
        body.extend_from_slice(&4u32.to_le_bytes());
        body.extend_from_slice(b"Alva");
        body.extend_from_slice(b"ICRD");
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend_from_slice(b"2026-08-29");
        let m = riff_info(&body);
        assert_eq!(m.title.as_deref(), Some("Wub"));
        assert_eq!(m.artist.as_deref(), Some("Alva"));
        assert_eq!(m.year, Some(2026));
    }
}
