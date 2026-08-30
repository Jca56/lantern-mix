//! Ui: begin/end frame, hot/active/focus, id scopes.
//!
//! `Ui` is the only cross-frame state. `UiFrame` is the per-frame handle every
//! widget method hangs off; dropping it finishes the frame.

use crate::{Id, Input, MouseButton, Painter, Rect, Text, Theme, Vec2};
use std::collections::HashMap;
use std::panic::Location;

/// Small per-widget memory (drag origin, animation phase, open/closed…).
#[derive(Clone, Copy, Debug, Default)]
pub struct Mem {
    pub drag_origin: Vec2,
    pub drag_start: f32,
    pub f: f32,
    pub b: bool,
}

pub struct Ui {
    pub theme: Theme,
    hot: Option<Id>,
    hot_next: Option<Id>,
    active: Option<Id>,
    mem: HashMap<Id, Mem>,
    scope: Vec<u64>,
    scope_hash: u64,
    continuous: bool,
    /// A press/release/click changed state this frame: draw one more frame so
    /// the result is visible (and pushed downstream) without waiting for input.
    repaint_once: bool,
    /// While set (this frame), only widgets intersecting it receive the pointer.
    modal: Option<Rect>,
    time: f64,
}

/// What happened to a widget this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction {
    /// Pointer is over the rect (and nothing else holds the pointer).
    pub hovered: bool,
    /// Left button went down on it this frame.
    pub pressed: bool,
    /// It holds the pointer (button still down since `pressed`).
    pub held: bool,
    /// Button released this frame while it held the pointer.
    pub released: bool,
    /// Released while still over the rect — a click.
    pub clicked: bool,
    /// Pointer movement this frame while held.
    pub drag: Vec2,
    /// Pointer position (logical px).
    pub mouse: Vec2,
}

impl Ui {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            hot: None,
            hot_next: None,
            active: None,
            mem: HashMap::new(),
            scope: Vec::new(),
            scope_hash: 0,
            continuous: false,
            repaint_once: false,
            modal: None,
            time: 0.0,
        }
    }

    /// Begin a frame. `dt` is seconds since the last frame.
    pub fn frame<'a>(&'a mut self, p: &'a mut Painter, t: &'a mut Text, input: &'a Input, dt: f32) -> UiFrame<'a> {
        self.continuous = false;
        self.repaint_once = false;
        self.modal = None;
        self.time += dt as f64;
        self.scope.clear();
        self.scope_hash = 0;
        let size = p.logical_size();
        UiFrame { ui: self, p, t, input, dt, size }
    }

    /// True while something is animating or being dragged, or a click just
    /// changed state: draw again without waiting for input.
    pub fn wants_continuous(&self) -> bool {
        self.continuous || self.active.is_some() || self.repaint_once
    }

    pub fn hot(&self) -> Option<Id> {
        self.hot
    }
    pub fn active(&self) -> Option<Id> {
        self.active
    }
    pub fn time(&self) -> f64 {
        self.time
    }

    fn rehash_scope(&mut self) {
        let mut h: u64 = 0xcbf29ce484222325;
        for s in &self.scope {
            h ^= *s;
            h = h.wrapping_mul(0x100000001b3);
        }
        self.scope_hash = h;
    }
}

pub struct UiFrame<'a> {
    pub ui: &'a mut Ui,
    pub p: &'a mut Painter,
    pub t: &'a mut Text,
    pub input: &'a Input,
    pub dt: f32,
    /// Logical size of the window.
    pub size: Vec2,
}

impl Drop for UiFrame<'_> {
    fn drop(&mut self) {
        self.ui.hot = self.ui.hot_next.take();
        if !self.input.down(MouseButton::Left) {
            self.ui.active = None;
        }
    }
}

impl UiFrame<'_> {
    pub fn theme(&self) -> &Theme {
        &self.ui.theme
    }

    pub fn time(&self) -> f64 {
        self.ui.time
    }

    /// Identity for the calling widget: call site + current scope.
    #[track_caller]
    pub fn id(&self) -> Id {
        Id::from_location(Location::caller(), self.ui.scope_hash)
    }

    /// Widgets built in a loop need a scope per iteration.
    pub fn push_scope(&mut self, salt: u64) {
        self.ui.scope.push(salt);
        self.ui.rehash_scope();
    }

    pub fn pop_scope(&mut self) {
        self.ui.scope.pop();
        self.ui.rehash_scope();
    }

    /// Ask for continuous redraws (animation running).
    pub fn animate(&mut self) {
        self.ui.continuous = true;
    }

    /// Route the pointer only to widgets inside `r` for the rest of this frame
    /// (open menus, modals). Call before drawing what should be blocked.
    pub fn set_modal(&mut self, r: Rect) {
        self.ui.modal = Some(r);
    }

    /// Clip shapes and text to `r` until `pop_clip`.
    pub fn push_clip(&mut self, r: Rect) {
        self.p.push_clip(r);
        self.t.push_clip(r);
    }

    pub fn pop_clip(&mut self) {
        self.p.pop_clip();
        self.t.pop_clip();
    }

    pub fn mem(&mut self, id: Id) -> &mut Mem {
        self.ui.mem.entry(id).or_default()
    }

    pub fn is_active(&self, id: Id) -> bool {
        self.ui.active == Some(id)
    }

    pub fn is_hot(&self, id: Id) -> bool {
        self.ui.hot == Some(id)
    }

    /// Pointer interaction for a rect-shaped widget. Call once per widget per frame.
    pub fn interact(&mut self, id: Id, rect: Rect) -> Interaction {
        let mouse = self.input.mouse;
        let blocked = self.ui.modal.map(|m| m.intersect(&rect).is_empty()).unwrap_or(false);
        let inside = self.input.mouse_in_window && rect.contains(mouse) && !blocked;
        let mine = self.ui.active == Some(id);
        let hovered = inside && (self.ui.active.is_none() || mine);
        if hovered {
            self.ui.hot_next = Some(id);
        }
        let mut it = Interaction { hovered, mouse, ..Default::default() };
        if hovered && self.input.pressed(MouseButton::Left) && self.ui.active.is_none() {
            self.ui.active = Some(id);
            it.pressed = true;
            let m = self.mem(id);
            m.drag_origin = mouse;
        }
        let mine = self.ui.active == Some(id);
        it.held = mine && self.input.down(MouseButton::Left);
        if mine && self.input.released(MouseButton::Left) {
            it.released = true;
            it.clicked = inside;
            self.ui.active = None;
        }
        if it.held || it.released {
            it.drag = self.input.mouse_delta();
        }
        if it.pressed || it.released {
            self.ui.repaint_once = true;
        }
        it
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Interaction logic is tested through `Ui` state directly; drawing needs a GPU.
    #[test]
    fn scope_changes_hash() {
        let mut ui = Ui::new(Theme::default());
        ui.scope.push(1);
        ui.rehash_scope();
        let a = ui.scope_hash;
        ui.scope.push(2);
        ui.rehash_scope();
        assert_ne!(a, ui.scope_hash);
        ui.scope.pop();
        ui.rehash_scope();
        assert_eq!(a, ui.scope_hash);
    }
}
