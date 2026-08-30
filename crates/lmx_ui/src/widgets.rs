//! Buttons, toggles, pads, knobs, faders, meters, text field, dropdown, tabs, modal.
//!
//! Every widget takes the rect it should occupy (callers lay out from shared
//! rects) and draws immediately. Methods that can be clicked/dragged are
//! `#[track_caller]` so their identity is the app's call site.

use crate::ui::LateText;
use crate::{Color, Rect, UiFrame, Vec2};
use std::f32::consts::PI;

/// Knob track: starts bottom-left, sweeps 270° clockwise to bottom-right.
const KNOB_A0: f32 = 0.75 * PI;
const KNOB_SWEEP: f32 = 1.5 * PI;
/// Vertical drag distance (logical px) for a knob's full range.
const KNOB_DRAG_PX: f32 = 200.0;

impl UiFrame<'_> {
    // ── text ─────────────────────────────────────────────────────────────

    /// Every text helper ends here: straight to the renderer, or deferred to
    /// the end of the frame while an overlay is being drawn.
    fn emit(&mut self, s: &str, size: f32, x: f32, y: f32, color: Color, bold: bool) {
        if self.late_mode {
            self.late.push(LateText { s: s.to_string(), size, x, y, color, bold });
        } else if bold {
            self.t.draw_bold(s, size, x, y, color);
        } else {
            self.t.draw(s, size, x, y, color);
        }
    }

    pub fn text(&mut self, s: &str, size: f32, pos: Vec2, color: Color) {
        self.emit(s, size, pos.x, pos.y, color, false);
    }

    pub fn text_bold(&mut self, s: &str, size: f32, pos: Vec2, color: Color) {
        self.emit(s, size, pos.x, pos.y, color, true);
    }

    /// Text centered in `rect` (both axes).
    pub fn text_centered(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        let w = self.t.width(s, size);
        let x = rect.center().x - w * 0.5;
        let y = self.ui.theme.text_y(rect.y, rect.h, size);
        self.emit(s, size, x, y, color, false);
    }

    /// Text vertically centered, right-aligned to `rect`.
    pub fn text_right(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        let w = self.t.width(s, size);
        let y = self.ui.theme.text_y(rect.y, rect.h, size);
        self.emit(s, size, rect.right() - w, y, color, false);
    }

    /// Text vertically centered, left-aligned to `rect`.
    pub fn text_left(&mut self, rect: Rect, s: &str, size: f32, color: Color) {
        let y = self.ui.theme.text_y(rect.y, rect.h, size);
        self.emit(s, size, rect.x, y, color, false);
    }

    pub fn label(&mut self, pos: Vec2, s: &str) {
        let (size, c) = (self.ui.theme.text, self.ui.theme.fg);
        self.text(s, size, pos, c);
    }

    pub fn label_dim(&mut self, pos: Vec2, s: &str) {
        let (size, c) = (self.ui.theme.text_small, self.ui.theme.fg_dim);
        self.text(s, size, pos, c);
    }

    pub fn title(&mut self, pos: Vec2, s: &str) {
        let (size, c) = (self.ui.theme.title, self.ui.theme.fg);
        self.text_bold(s, size, pos, c);
    }

    /// Big value with a small unit after it, vertically centered in `rect`.
    pub fn readout(&mut self, rect: Rect, value: &str, unit: &str, color: Color) {
        let th = self.ui.theme.clone();
        let vw = self.t.width(value, th.readout);
        let y = th.text_y(rect.y, rect.h, th.readout);
        self.emit(value, th.readout, rect.x, y, color, true);
        if !unit.is_empty() {
            let uy = th.text_y(rect.y, rect.h, th.text_small) + (th.readout - th.text_small) * 0.42;
            self.emit(unit, th.text_small, rect.x + vw + 10.0, uy, th.fg_dim, false);
        }
    }

    // ── surfaces ─────────────────────────────────────────────────────────

    pub fn panel(&mut self, rect: Rect) {
        let th = &self.ui.theme;
        let (fill, border, r, s) = (th.panel, th.border, th.radius, th.stroke);
        self.p.fill_rrect(rect, r, fill);
        self.p.stroke_rrect(rect, r, s, border);
    }

    pub fn well(&mut self, rect: Rect) {
        let th = &self.ui.theme;
        let (fill, r) = (th.well_deep, th.radius * 0.5);
        self.p.fill_rrect(rect, r, fill);
    }

    pub fn divider(&mut self, rect: Rect) {
        let c = self.ui.theme.border_hot;
        self.p.divider(rect, c);
    }

    // ── buttons ──────────────────────────────────────────────────────────

    #[track_caller]
    pub fn button(&mut self, rect: Rect, label: &str) -> bool {
        let id = self.id();
        let it = self.interact(id, rect);
        let th = self.ui.theme.clone();
        let fill = if it.held { th.accent } else if it.hovered { th.well.lighten(0.08) } else { th.well };
        let border = if it.hovered || it.held { th.border_hot } else { th.border };
        self.p.fill_rrect(rect, th.radius, fill);
        self.p.stroke_rrect(rect, th.radius, th.stroke, border);
        self.text_centered(rect, label, th.text, th.fg);
        it.clicked
    }

    /// Latching button; `on` flips on click. Returns true when it changed.
    #[track_caller]
    pub fn toggle(&mut self, rect: Rect, label: &str, on: &mut bool) -> bool {
        self.toggle_colored(rect, label, on, self.ui.theme.accent)
    }

    #[track_caller]
    pub fn toggle_colored(&mut self, rect: Rect, label: &str, on: &mut bool, color: Color) -> bool {
        let id = self.id();
        let it = self.interact(id, rect);
        if it.clicked {
            *on = !*on;
        }
        let th = self.ui.theme.clone();
        let fill = if *on {
            if it.hovered { color.lighten(0.12) } else { color }
        } else if it.hovered {
            th.well.lighten(0.08)
        } else {
            th.well
        };
        let border = if *on { color.lighten(0.3) } else if it.hovered { th.border_hot } else { th.border };
        let fg = if *on { th.well_deep } else { th.fg_dim };
        self.p.fill_rrect(rect, th.radius, fill);
        self.p.stroke_rrect(rect, th.radius, th.stroke, border);
        self.text_centered(rect, label, th.text, fg);
        it.clicked
    }

    // ── text field ───────────────────────────────────────────────────────

    /// Single-line text entry. Click focuses; typing edits; Backspace deletes;
    /// Escape clears and unfocuses; Enter unfocuses; a click elsewhere unfocuses.
    /// Returns true when `text` changed.
    #[track_caller]
    pub fn text_field(&mut self, rect: Rect, text: &mut String, focused: &mut bool) -> bool {
        let id = self.id();
        let it = self.interact(id, rect);
        if it.pressed {
            *focused = true;
        } else if self.input.pressed(crate::MouseButton::Left) && !rect.contains(self.input.mouse) {
            *focused = false;
        }
        let mut changed = false;
        if *focused {
            if !self.input.text.is_empty() {
                text.push_str(&self.input.text);
                changed = true;
            }
            for _ in 0..self.input.key_count(crate::Key::Backspace) {
                if text.pop().is_some() {
                    changed = true;
                }
            }
            if self.input.key(crate::Key::Escape) {
                if !text.is_empty() {
                    changed = true;
                }
                text.clear();
                *focused = false;
            }
            if self.input.key(crate::Key::Enter) {
                *focused = false;
            }
        }
        let th = self.ui.theme.clone();
        self.p.fill_rrect(rect, 5.0, th.well_deep);
        self.p.stroke_rrect(rect, 5.0, th.stroke, if *focused { th.accent } else if it.hovered { th.border_hot } else { th.border });
        let inner = rect.inset_xy(15.0, 0.0);
        self.push_clip(inner);
        let w = self.t.width(text, th.text);
        let x = if w > inner.w { inner.right() - w } else { inner.x };
        let y = th.text_y(inner.y, inner.h, th.text);
        self.emit(text, th.text, x, y, th.fg, false);
        if *focused {
            let cx = (x + w + 2.0).min(inner.right());
            self.p.fill_rect(Rect::new(cx, inner.y + 10.0, 5.0, inner.h - 20.0), th.accent);
        }
        self.pop_clip();
        changed
    }

    // ── menus ────────────────────────────────────────────────────────────

    /// Menu-bar button: flips `open` on click; drawn "pressed" while open.
    #[track_caller]
    pub fn menu_button(&mut self, rect: Rect, label: &str, open: &mut bool) -> bool {
        let id = self.id();
        let it = self.interact(id, rect);
        if it.clicked {
            *open = !*open;
        }
        let th = self.ui.theme.clone();
        if *open || it.hovered {
            self.p.fill_rrect(rect, 5.0, th.well);
        }
        let fg = if *open { th.fg } else { th.fg_dim };
        self.text_centered(rect, label, th.text_small, fg);
        it.clicked
    }

    /// Dropdown under `anchor` while `open`: one row per item, `checked` items
    /// carry a dot. Returns the clicked index. A press outside closes it. The
    /// menu is modal for the rest of the frame.
    #[track_caller]
    pub fn dropdown(&mut self, anchor: Rect, open: &mut bool, items: &[(&str, bool)]) -> Option<usize> {
        if !*open || items.is_empty() {
            return None;
        }
        let th = self.ui.theme.clone();
        let row_h = 50.0;
        let mut w: f32 = 250.0;
        for (label, _) in items {
            w = w.max(self.t.width(label, th.text) + 80.0);
        }
        let panel = Rect::new(anchor.x, anchor.bottom(), w.min(self.size.x - anchor.x), row_h * items.len() as f32 + 10.0);
        self.set_modal(panel);
        if self.input.pressed(crate::MouseButton::Left) && !panel.contains(self.input.mouse) {
            *open = false;
            return None;
        }
        let base = self.id();
        let old_layer = self.p.layer();
        self.occlude(panel);
        self.set_late_mode(true);
        self.p.set_layer(2);
        self.p.fill_rrect(panel, 5.0, th.panel);
        self.p.stroke_rrect(panel, 5.0, th.stroke, th.border);
        let mut picked = None;
        for (i, (label, checked)) in items.iter().enumerate() {
            let row = Rect::new(panel.x + 5.0, panel.y + 5.0 + i as f32 * row_h, panel.w - 10.0, row_h);
            let it = self.interact(base.with(i as u64), row);
            if it.hovered {
                self.p.fill_rrect(row, 5.0, th.well);
            }
            if *checked {
                self.p.circle(Vec2::new(row.x + 25.0, row.center().y), 5.0, th.fg);
            }
            let text_rect = Rect::new(row.x + 50.0, row.y, row.w - 50.0, row.h);
            self.text_left(text_rect, label, th.text, th.fg);
            if it.clicked {
                picked = Some(i);
                *open = false;
            }
        }
        self.p.set_layer(old_layer);
        self.set_late_mode(false);
        picked
    }

    // ── faders ───────────────────────────────────────────────────────────

    /// Horizontal fader, 0 at the left. Returns true when the value changed.
    #[track_caller]
    pub fn hfader(&mut self, rect: Rect, v: &mut f32) -> bool {
        self.fader_impl(rect, v, false, true)
    }

    /// Vertical fader, 1 at the top.
    #[track_caller]
    pub fn vfader(&mut self, rect: Rect, v: &mut f32) -> bool {
        self.fader_impl(rect, v, true, true)
    }

    /// Crossfader: horizontal, no fill, center detent mark.
    #[track_caller]
    pub fn crossfader(&mut self, rect: Rect, v: &mut f32) -> bool {
        self.fader_impl(rect, v, false, false)
    }

    #[track_caller]
    fn fader_impl(&mut self, rect: Rect, v: &mut f32, vertical: bool, fill: bool) -> bool {
        let id = self.id();
        let it = self.interact(id, rect);
        let th = self.ui.theme.clone();
        let grip = th.fader_grip;
        let usable = if vertical { rect.h - grip } else { rect.w - grip }.max(1.0);
        let before = *v;
        if it.pressed || it.held {
            let t = if vertical {
                1.0 - (it.mouse.y - rect.y - grip * 0.5) / usable
            } else {
                (it.mouse.x - rect.x - grip * 0.5) / usable
            };
            *v = t.clamp(0.0, 1.0);
        }
        // track
        let track = if vertical {
            Rect::from_center(rect.center(), th.fader_track, rect.h - grip * 0.5)
        } else {
            Rect::from_center(rect.center(), rect.w - grip * 0.5, th.fader_track)
        };
        self.p.fill_rrect(track, th.fader_track * 0.5, th.track);
        // grip position
        let gpos = if vertical { rect.y + (1.0 - *v) * usable } else { rect.x + *v * usable };
        if fill {
            let f = if vertical {
                Rect::new(track.x, gpos + grip * 0.5, track.w, track.bottom() - gpos - grip * 0.5)
            } else {
                Rect::new(track.x, track.y, gpos + grip * 0.5 - track.x, track.h)
            };
            if f.w > 0.0 && f.h > 0.0 {
                self.p.fill_rrect(f, th.fader_track * 0.5, th.accent);
            }
        } else {
            // center detent
            let c = rect.center();
            let (a, b) = if vertical {
                (Vec2::new(rect.x, c.y), Vec2::new(rect.right(), c.y))
            } else {
                (Vec2::new(c.x, rect.y), Vec2::new(c.x, rect.bottom()))
            };
            self.p.line(a, b, th.line, th.border);
        }
        let g = if vertical {
            Rect::new(rect.x, gpos, rect.w, grip)
        } else {
            Rect::new(gpos, rect.y, grip, rect.h)
        };
        let gc = if it.held { th.accent_hot } else if it.hovered { th.grip.lighten(0.2) } else { th.grip };
        self.p.fill_rrect(g, 5.0, gc);
        // grip line
        let (a, b) = if vertical {
            (Vec2::new(g.x + 5.0, g.center().y), Vec2::new(g.right() - 5.0, g.center().y))
        } else {
            (Vec2::new(g.center().x, g.y + 5.0), Vec2::new(g.center().x, g.bottom() - 5.0))
        };
        self.p.line(a, b, th.line, th.well_deep);
        (*v - before).abs() > 1e-6
    }

    // ── knob ─────────────────────────────────────────────────────────────

    /// Rotary control drawn at `center` with the theme's knob radius; vertical
    /// drag, shift for fine. `label` sits under it. Returns true on change.
    #[track_caller]
    pub fn knob(&mut self, center: Vec2, radius: f32, v: &mut f32, label: &str, color: Color) -> bool {
        let id = self.id();
        let hit = Rect::from_center(center, radius * 2.0, radius * 2.0);
        let it = self.interact(id, hit);
        let th = self.ui.theme.clone();
        let before = *v;
        if it.held && it.drag.y != 0.0 {
            let fine = if self.input.shift { 0.1 } else { 1.0 };
            *v = (*v - it.drag.y / KNOB_DRAG_PX * fine).clamp(0.0, 1.0);
        }
        let thick = 10.0;
        self.p.arc(center, radius, thick, KNOB_A0, KNOB_A0 + KNOB_SWEEP, th.track);
        let a1 = KNOB_A0 + KNOB_SWEEP * *v;
        if *v > 0.002 {
            self.p.arc(center, radius, thick, KNOB_A0, a1, color);
        }
        let body = radius - thick - 5.0;
        self.p.circle(center, body, th.well_deep);
        self.p.circle_stroke(center, body, th.stroke, if it.hovered || it.held { th.border_hot } else { th.border });
        let tip = center + Vec2::from_angle(a1, body - 5.0);
        let root = center + Vec2::from_angle(a1, body * 0.45);
        self.p.line(root, tip, th.line, th.fg);
        if !label.is_empty() {
            let lr = Rect::new(center.x - radius * 1.5, center.y + radius + 5.0, radius * 3.0, th.text * 1.4);
            self.text_centered(lr, label, th.text, th.fg_dim);
        }
        (*v - before).abs() > 1e-6
    }

    // ── meter ────────────────────────────────────────────────────────────

    /// Single-bar peak meter, same scale as `meter`.
    pub fn meter_mono(&mut self, rect: Rect, db: f32) {
        let th = self.ui.theme.clone();
        self.well(rect);
        let bar = rect.inset(5.0);
        let map = |db: f32| ((db + 60.0) / 63.0).clamp(0.0, 1.0);
        let t = map(db);
        let y_top = bar.bottom() - bar.h * t;
        let y_zero = bar.bottom() - bar.h * map(0.0);
        let green_top = y_top.max(y_zero);
        if green_top < bar.bottom() {
            self.p.fill_rrect(Rect::new(bar.x, green_top, bar.w, bar.bottom() - green_top), 5.0, th.meter_ok);
        }
        if y_top < y_zero {
            self.p.fill_rrect(Rect::new(bar.x, y_top, bar.w, y_zero - y_top), 5.0, th.meter_hot);
        }
    }

    /// Stereo peak meter. Levels in dBFS; scale −60…+3, red above 0.
    pub fn meter(&mut self, rect: Rect, db_l: f32, db_r: f32) {
        let th = self.ui.theme.clone();
        self.well(rect);
        let inner = rect.inset(5.0);
        let gap = 5.0;
        let bw = (inner.w - gap) * 0.5;
        let bars = [Rect::new(inner.x, inner.y, bw, inner.h), Rect::new(inner.x + bw + gap, inner.y, bw, inner.h)];
        let map = |db: f32| ((db + 60.0) / 63.0).clamp(0.0, 1.0);
        let zero_t = map(0.0);
        for (bar, db) in bars.iter().zip([db_l, db_r]) {
            let t = map(db);
            let y_top = bar.bottom() - bar.h * t;
            let y_zero = bar.bottom() - bar.h * zero_t;
            let green_top = y_top.max(y_zero);
            if green_top < bar.bottom() {
                self.p.fill_rrect(Rect::new(bar.x, green_top, bar.w, bar.bottom() - green_top), 5.0, th.meter_ok);
            }
            if y_top < y_zero {
                self.p.fill_rrect(Rect::new(bar.x, y_top, bar.w, y_zero - y_top), 5.0, th.meter_hot);
            }
        }
        // 0 dB mark: short ticks on both outer edges
        let y = inner.bottom() - inner.h * zero_t;
        self.p.fill_rect(Rect::new(rect.x, y - 2.5, 5.0, 5.0), th.fg_dim);
        self.p.fill_rect(Rect::new(rect.right() - 5.0, y - 2.5, 5.0, 5.0), th.fg_dim);
    }
}
