//! Scrolling waveform + overview widgets with markers.

use crate::{Rect, UiFrame, Vec2};
use lmx_gpu::{WaveId, WaveLevel};

/// Beat grid to overlay, in fine columns.
#[derive(Clone, Copy, Debug)]
pub struct GridView {
    /// Columns per beat (sample_rate · 60 / bpm / FINE_FRAMES).
    pub beat_cols: f32,
    /// Column of the anchor beat (bar 1, beat 1).
    pub anchor_col: f32,
}

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
    pub grid: Option<GridView>,
}

/// What the pointer did to a strip this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct StripInteraction {
    /// Horizontal drag this frame, in fine columns (positive = dragged right).
    pub drag_cols: f32,
    pub held: bool,
    pub released: bool,
    pub shift: bool,
}

/// Cap on grid lines per strip so a zoomed-out view can't drown the painter.
const MAX_GRID_LINES: usize = 600;

impl UiFrame<'_> {
    /// Deck strip: dark well, waveform scrolling under a fixed center play head,
    /// bar grid, deck tag top-left. Dragging reports columns moved.
    #[track_caller]
    pub fn waveform_strip(&mut self, rect: Rect, v: &StripView, label: &str) -> StripInteraction {
        let th = self.ui.theme.clone();
        let color = th.deck[v.deck.min(3)];
        let inner = Rect::new(rect.x + 10.0, rect.y, rect.w - 10.0, rect.h);
        let id = self.id();
        let it = self.interact(id, inner);
        let out = StripInteraction {
            drag_cols: if it.held { it.drag.x * v.cols_per_px } else { 0.0 },
            held: it.held,
            released: it.released,
            shift: self.input.shift,
        };

        self.p.set_layer(0);
        self.p.fill_rrect(rect, 5.0, th.well_deep);
        let first = v.playhead_col - inner.w * 0.5 * v.cols_per_px;
        if let Some(wid) = v.wave {
            self.p.push_clip(inner);
            self.p.waveform(wid, WaveLevel::Fine, inner, first, v.cols_per_px, 1.0);
            self.p.pop_clip();
        }

        self.p.set_layer(1);
        if let (Some(g), true) = (v.grid, v.wave.is_some()) {
            let bar_cols = g.beat_cols * 4.0;
            if bar_cols > 0.5 {
                let last = first + inner.w * v.cols_per_px;
                let k0 = ((first - g.anchor_col) / bar_cols).floor() as i64;
                let k1 = ((last - g.anchor_col) / bar_cols).ceil() as i64;
                let n = (k1 - k0 + 1).clamp(0, MAX_GRID_LINES as i64);
                self.p.push_clip(inner);
                for k in k0..k0 + n {
                    let col = g.anchor_col + k as f32 * bar_cols;
                    if col < 0.0 || (v.columns > 0 && col > v.columns as f32) {
                        continue;
                    }
                    let x = inner.x + (col - first) / v.cols_per_px;
                    let c = if k % 4 == 0 { th.fg.with_alpha(0.6) } else { th.fg_dim.with_alpha(0.35) };
                    self.p.line(Vec2::new(x, rect.y), Vec2::new(x, rect.bottom()), th.line, c);
                }
                self.p.pop_clip();
            }
        }
        self.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        let cx = inner.x + inner.w * 0.5;
        self.p.line(Vec2::new(cx, rect.y), Vec2::new(cx, rect.bottom()), th.line, th.fg);
        let tag = Rect::new(rect.x + 20.0, rect.y, 400.0, rect.h.min(35.0));
        self.text_left(tag, label, th.text_small, color);
        self.p.set_layer(0);
        out
    }
}
