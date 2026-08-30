//! Sizes (minimums per docs/05-UI.md), scale factor, palette.
//!
//! Sizes are logical px and already *big*: Alva has poor eyesight, nothing goes
//! below 20 px. The palette is a placeholder until there is a screen to look at —
//! every color lives here so swapping it is one edit.

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
            text: 22.0,
            text_small: 20.0,
            readout: 44.0,
            title: 64.0,
            button_h: 56.0,
            hit: 48.0,
            knob_r: 44.0,
            fader_grip: 28.0,
            fader_track: 10.0,
            meter_w: 26.0,
            radius: 10.0,
            stroke: 3.0,
            gap: 12.0,
            pad: 16.0,
            bg: Color::hex(0x0B0C10),
            panel: Color::hex(0x15171E),
            well: Color::hex(0x1D2029),
            well_deep: Color::hex(0x0F1015),
            border: Color::hex(0x3A3F52),
            border_hot: Color::hex(0x6B7190),
            fg: Color::hex(0xF2F2F6),
            fg_dim: Color::hex(0xA9ACBD),
            accent: Color::hex(0x2F8BFF),
            accent_hot: Color::hex(0x7FB6FF),
            warn: Color::hex(0xFF5A3C),
            ok: Color::hex(0x3DD68C),
            track: Color::hex(0x2A2D3A),
            grip: Color::hex(0xE8E9F0),
            meter_ok: Color::hex(0x22B45A),
            meter_hot: Color::hex(0xF02A2A),
            deck: [Color::hex(0x2F8BFF), Color::hex(0xFF8A3D), Color::hex(0x3DD68C), Color::hex(0xC46BFF)],
        }
    }
}

impl Theme {
    /// Vertical offset from a line box's top to where lntrn-text wants `y` for
    /// visually centering `size` px text in a box of height `h`.
    pub fn text_y(&self, box_y: f32, box_h: f32, size: f32) -> f32 {
        box_y + (box_h - size * 1.2) * 0.5
    }
}
