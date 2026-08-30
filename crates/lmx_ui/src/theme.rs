//! Sizes (minimums per docs/05-UI.md), scale factor, palette.
//!
//! Sizes are logical px, already *big*, and always multiples of 5 (Alva's rule —
//! 45 not 44, 55 not 56). Nothing goes below 20 px. No glows anywhere.
//! Palette direction from Alva: neutral grey/black base, neutral accent, decks are
//! plain Red / Green / Blue / Purple. This must NOT look like the Lantern plugins.

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
    pub scrollbar: Color,
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
            bg: Color::hex(0x0B0B0B),
            panel: Color::hex(0x151515),
            well: Color::hex(0x2E2E2E),
            well_deep: Color::hex(0x070707),
            border: Color::hex(0x525252),
            border_hot: Color::hex(0x9A9A9A),
            fg: Color::hex(0xF6F6F6),
            fg_dim: Color::hex(0xB8B8B8),
            accent: Color::hex(0xD8D8D8),
            accent_hot: Color::hex(0xFFFFFF),
            warn: Color::hex(0xFF4646),
            ok: Color::hex(0x7CE04A),
            track: Color::hex(0x484848),
            grip: Color::hex(0xEDEDED),
            meter_ok: Color::hex(0x64D23C),
            meter_hot: Color::hex(0xFF3B30),
            scrollbar: Color::hex(0xE6B422),
            deck: [Color::hex(0xFF2E2E), Color::hex(0x2EDD4A), Color::hex(0x2E7BFF), Color::hex(0xB04DFF)],
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
