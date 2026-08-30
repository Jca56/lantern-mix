//! sRGB colors for the painter and text. Stored as 0..1 floats in sRGB space (what
//! designers think in); `to_linear` is what the GPU gets.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    pub const fn rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }
    pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
    }
    /// `0xRRGGBB`.
    pub const fn hex(v: u32) -> Self {
        Self::rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
    pub fn mul_alpha(self, f: f32) -> Self {
        Self { a: self.a * f, ..self }
    }
    /// Move toward white by `t` (0 = unchanged, 1 = white).
    pub fn lighten(self, t: f32) -> Self {
        Self::rgba(self.r + (1.0 - self.r) * t, self.g + (1.0 - self.g) * t, self.b + (1.0 - self.b) * t, self.a)
    }
    /// Move toward black by `t`.
    pub fn darken(self, t: f32) -> Self {
        Self::rgba(self.r * (1.0 - t), self.g * (1.0 - t), self.b * (1.0 - t), self.a)
    }
    pub fn lerp(self, o: Color, t: f32) -> Self {
        Self::rgba(
            self.r + (o.r - self.r) * t,
            self.g + (o.g - self.g) * t,
            self.b + (o.b - self.b) * t,
            self.a + (o.a - self.a) * t,
        )
    }
    /// sRGB → linear, straight (non-premultiplied) alpha.
    pub fn to_linear(self) -> [f32; 4] {
        [srgb_to_linear(self.r), srgb_to_linear(self.g), srgb_to_linear(self.b), self.a]
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = Color::hex(0x2F8BFF);
        assert!((c.r - 47.0 / 255.0).abs() < 1e-6);
        assert!((c.g - 139.0 / 255.0).abs() < 1e-6);
        assert!((c.b - 1.0).abs() < 1e-6);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn linear_endpoints() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        assert!((srgb_to_linear(0.5) - 0.2140).abs() < 1e-3);
    }
}
