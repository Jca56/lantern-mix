//! Our 64-bit content hash used for TrackId.
//!
//! Not cryptographic: it identifies files, it doesn't protect them. 8 bytes at
//! a time with a multiply-xor mix and a splitmix finalizer; ~1 GB/s.

pub struct Hasher(u64);

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher {
    pub fn new() -> Self {
        Hasher(0x9E37_79B9_7F4A_7C15)
    }

    #[inline]
    fn mix(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(29) ^ v).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.mix(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.mix(u64::from_le_bytes(b) ^ ((rest.len() as u64) << 56));
        }
    }

    pub fn write_u64(&mut self, v: u64) {
        self.mix(v ^ 0xA5A5_A5A5_A5A5_A5A5);
    }

    pub fn finish(&self) -> u64 {
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

pub fn hash64(bytes: &[u8]) -> u64 {
    let mut h = Hasher::new();
    h.write(bytes);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_sensitive() {
        assert_eq!(hash64(b"lantern"), hash64(b"lantern"));
        assert_ne!(hash64(b"lantern"), hash64(b"lanterm"));
        assert_ne!(hash64(b""), hash64(b"\0"));
        assert_ne!(hash64(&[0u8; 8]), hash64(&[0u8; 9]));
    }
}
