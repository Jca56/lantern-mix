//! Sizes (minimums per docs/05-UI.md), scale factor, palette.
//!
//! Sizes are logical px, already *big*, and always multiples of 5 (Alva's rule —
//! 45 not 44, 55 not 56). Nothing goes below 20 px. No glows anywhere.
//! Palette direction from Alva: warm, not blue, high contrast.

use crate::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    /// Extra user scale on top of the window's DPI scale.
    pub scale: f32,

    // ── type ──
    pub text: f32,
    pub text_small: f32,
    pub readout: f32,
    pub title: f32,

    // ── geometry ──
    pub button_h: f32,
    pub hit: f32,
    pub knob_r: f32,
    pub fader_grip: f32,
    pub fader_track: f32,
    pub meter_w: f32,
    pub radius: f32,
    pub stroke: f32,
    pub line: f32,
    pub gap: f32,
    pub pad: f32,

    // ── palette ──
    pub bg: Color,
    pub panel: Color,
    pub well: Color,
    pub well_deep: Color,
    pub border: Color,
    pub border_hot: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub accent: Color,
    pub accent_hot: Color,
    pub warn: Color,
    pub ok: Color,
    pub track: Color,
    pub grip: Color,
    pub meter_ok: Color,
    pub meter_hot: Color,
    pub deck: [Color; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            scale: 1.0,
            text: 25.0,
            text_small: 20.0,
            readout: 45.0,
            title: 60.0,
            button_h: 55.0,
            hit: 50.0,
            knob_r: 45.0,
            fader_grip: 30.0,
            fader_track: 10.0,
            meter_w: 25.0,
            radius: 10.0,
            stroke: 5.0,
            line: 5.0,
            gap: 10.0,
            pad: 15.0,
            bg: Color::hex(0x0D0B09),
            panel: Color::hex(0x1C1814),
            well: Color::hex(0x2A2420),
            well_deep: Color::hex(0x110E0B),
            border: Color::hex(0x5A4E42),
            border_hot: Color::hex(0x9C8A76),
            fg: Color::hex(0xFFF8EE),
            fg_dim: Color::hex(0xC4B7A6),
            accent: Color::hex(0xFFB238),
            accent_hot: Color::hex(0xFFD07A),
            warn: Color::hex(0xFF4A3A),
            ok: Color::hex(0x8FE04A),
            track: Color::hex(0x3A322B),
            grip: Color::hex(0xF4ECE0),
            meter_ok: Color::hex(0x7AD64A),
            meter_hot: Color::hex(0xFF3B2F),
            deck: [Color::hex(0xFF6A3D), Color::hex(0xFF4F6E), Color::hex(0xBFE84A), Color::hex(0xE86BFF)],
        }
    }
}

impl Theme {
    /// Where lntrn-text wants `y` (top of its 1.2×size line box) to visually
    /// center `size` px text in a box starting at `box_y` of height `box_h`.
    pub fn text_y(&self, box_y: f32, box_h: f32, size: f32) -> f32 {
        box_y + (box_h - size * 1.2) * 0.5
    }
}
