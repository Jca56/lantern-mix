//! Scrolling waveform + overview widgets with markers.

use crate::{Rect, UiFrame, Vec2};
use lmx_gpu::{WaveId, WaveLevel};

/// What a deck strip needs to draw itself.
#[derive(Clone, Copy, Debug)]
pub struct StripView {
    pub wave: Option<WaveId>,
    /// Fine columns in the summary.
    pub columns: u32,
    /// Fine-column position of the play head.
    pub playhead_col: f32,
    /// Fine columns per logical pixel (zoom).
    pub cols_per_px: f32,
    pub deck: usize,
    pub playing: bool,
}

impl UiFrame<'_> {
    /// Deck strip: dark well, waveform scrolling under a fixed center play head,
    /// deck tag top-left.
    pub fn waveform_strip(&mut self, rect: Rect, v: &StripView, label: &str) {
        let th = self.ui.theme.clone();
        let color = th.deck[v.deck.min(3)];
        self.p.set_layer(0);
        self.p.fill_rrect(rect, 5.0, th.well_deep);
        if let Some(id) = v.wave {
            let inner = Rect::new(rect.x + 10.0, rect.y, rect.w - 10.0, rect.h);
            let first = v.playhead_col - inner.w * 0.5 * v.cols_per_px;
            self.p.push_clip(inner);
            self.p.waveform(id, WaveLevel::Fine, inner, first, v.cols_per_px, 1.0);
            self.p.pop_clip();
        }
        self.p.set_layer(1);
        self.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        let cx = rect.x + 10.0 + (rect.w - 10.0) * 0.5;
        self.p.line(Vec2::new(cx, rect.y), Vec2::new(cx, rect.bottom()), th.line, th.fg);
        let tag = Rect::new(rect.x + 20.0, rect.y, 400.0, rect.h.min(35.0));
        self.text_left(tag, label, th.text_small, color);
        self.p.set_layer(0);
    }
}
