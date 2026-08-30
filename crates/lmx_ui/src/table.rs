//! Virtualized sortable table and list; tree.
//!
//! `rows()` handles scrolling (wheel, scrollbar drag, keep-visible) and tells the
//! caller which rows are on screen; the caller draws them, so any row content
//! works and only visible rows cost anything.

use crate::{Rect, UiFrame};
use std::ops::Range;

const BAR_W: f32 = 15.0;

/// Which rows to draw and where.
pub struct VisibleRows {
    pub range: Range<usize>,
    /// Area rows are drawn in (scrollbar excluded); push a clip on it.
    pub area: Rect,
    row_h: f32,
    scroll: f32,
}

impl VisibleRows {
    pub fn rect(&self, i: usize) -> Rect {
        Rect::new(self.area.x, self.area.y + i as f32 * self.row_h - self.scroll, self.area.w, self.row_h)
    }
}

impl UiFrame<'_> {
    /// Scrollable list of `count` rows of `row_h`. `scroll` persists across
    /// frames (caller-owned, logical px). `keep_visible` scrolls so that row
    /// is fully on screen (selection follows keyboard).
    #[track_caller]
    pub fn rows(&mut self, rect: Rect, row_h: f32, count: usize, scroll: &mut f32, keep_visible: Option<usize>) -> VisibleRows {
        let th = self.ui.theme.clone();
        let content_h = count as f32 * row_h;
        let need_bar = content_h > rect.h;
        let area = if need_bar { Rect::new(rect.x, rect.y, rect.w - BAR_W - 5.0, rect.h) } else { rect };
        let max_scroll = (content_h - area.h).max(0.0);

        if let Some(i) = keep_visible {
            let top = i as f32 * row_h;
            let bottom = top + row_h;
            if top < *scroll {
                *scroll = top;
            } else if bottom > *scroll + area.h {
                *scroll = bottom - area.h;
            }
        }
        // wheel anywhere over the list (including the bar)
        let id = self.id();
        let hovered = self.input.mouse_in_window && rect.contains(self.input.mouse) && self.ui.active().map(|a| a == id).unwrap_or(true);
        if hovered && self.input.wheel.y != 0.0 {
            *scroll -= self.input.wheel.y;
        }
        // scrollbar
        if need_bar {
            let track = Rect::new(rect.right() - BAR_W, rect.y, BAR_W, rect.h);
            let frac = (area.h / content_h).clamp(0.05, 1.0);
            let thumb_h = (track.h * frac).max(30.0);
            let it = self.interact(id, track);
            if it.pressed {
                let thumb_top = track.y + (*scroll / max_scroll.max(1e-3)) * (track.h - thumb_h);
                let mouse = self.input.mouse;
                let on_thumb = mouse.y >= thumb_top && mouse.y <= thumb_top + thumb_h;
                let m = self.mem(id);
                m.drag_start = if on_thumb { *scroll } else { f32::NAN };
                m.drag_origin = mouse;
                if !on_thumb {
                    // page jump toward the click
                    if mouse.y < thumb_top {
                        *scroll -= area.h;
                    } else {
                        *scroll += area.h;
                    }
                }
            }
            if it.held {
                let m = *self.mem(id);
                if !m.drag_start.is_nan() {
                    let dy = self.input.mouse.y - m.drag_origin.y;
                    *scroll = m.drag_start + dy / (track.h - thumb_h).max(1.0) * max_scroll;
                }
            }
            *scroll = scroll.clamp(0.0, max_scroll);
            let thumb_top = track.y + (*scroll / max_scroll.max(1e-3)) * (track.h - thumb_h);
            self.p.fill_rrect(track, 5.0, th.well_deep);
            let tc = if it.hovered || it.held { th.grip } else { th.border_hot };
            self.p.fill_rrect(Rect::new(track.x, thumb_top, track.w, thumb_h), 5.0, tc);
        } else {
            *scroll = 0.0;
        }

        let first = (*scroll / row_h).floor().max(0.0) as usize;
        let last = (((*scroll + area.h) / row_h).ceil() as usize).min(count);
        VisibleRows { range: first.min(last)..last, area, row_h, scroll: *scroll }
    }
}
