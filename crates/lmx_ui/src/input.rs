//! Pointer, keyboard, text, wheel, file drops — everything the window feeds the UI
//! for one frame, in logical px.

use crate::Vec2;
use std::path::PathBuf;

/// Non-text keys the UI cares about. Printable characters arrive as `text`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Backspace,
    Delete,
    Enter,
    Escape,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    fn idx(self) -> usize {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub mouse: Vec2,
    pub mouse_prev: Vec2,
    pub mouse_in_window: bool,
    down: [bool; 3],
    pressed: [bool; 3],
    released: [bool; 3],
    /// Scroll this frame, logical px (positive y = content moves down / scroll up).
    pub wheel: Vec2,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub dropped_files: Vec<PathBuf>,
    /// Characters typed this frame.
    pub text: String,
    /// Named keys pressed this frame (with auto-repeat).
    pub keys: Vec<Key>,
}

impl Input {
    /// Clear per-frame edges. Call before feeding the next frame's events.
    pub fn begin_frame(&mut self) {
        self.mouse_prev = self.mouse;
        self.pressed = [false; 3];
        self.released = [false; 3];
        self.wheel = Vec2::ZERO;
        self.dropped_files.clear();
        self.text.clear();
        self.keys.clear();
    }

    pub fn on_cursor(&mut self, p: Vec2) {
        self.mouse = p;
        self.mouse_in_window = true;
    }

    pub fn on_cursor_left(&mut self) {
        self.mouse_in_window = false;
    }

    pub fn on_button(&mut self, b: MouseButton, down: bool) {
        let i = b.idx();
        if down && !self.down[i] {
            self.pressed[i] = true;
        }
        if !down && self.down[i] {
            self.released[i] = true;
        }
        self.down[i] = down;
    }

    pub fn on_wheel(&mut self, dx: f32, dy: f32) {
        self.wheel += Vec2::new(dx, dy);
    }

    pub fn on_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.shift = shift;
        self.ctrl = ctrl;
        self.alt = alt;
    }

    pub fn on_drop(&mut self, path: PathBuf) {
        self.dropped_files.push(path);
    }

    pub fn on_text(&mut self, s: &str) {
        // control characters (Enter, Backspace…) come through as keys instead
        self.text.extend(s.chars().filter(|c| !c.is_control()));
    }

    pub fn on_key(&mut self, k: Key) {
        self.keys.push(k);
    }

    pub fn key(&self, k: Key) -> bool {
        self.keys.contains(&k)
    }

    /// Number of times `k` was pressed this frame (auto-repeat).
    pub fn key_count(&self, k: Key) -> usize {
        self.keys.iter().filter(|x| **x == k).count()
    }

    pub fn down(&self, b: MouseButton) -> bool {
        self.down[b.idx()]
    }
    pub fn pressed(&self, b: MouseButton) -> bool {
        self.pressed[b.idx()]
    }
    pub fn released(&self, b: MouseButton) -> bool {
        self.released[b.idx()]
    }
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse - self.mouse_prev
    }
    pub fn any_down(&self) -> bool {
        self.down.iter().any(|d| *d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_and_release_edges() {
        let mut i = Input::default();
        i.on_button(MouseButton::Left, true);
        assert!(i.pressed(MouseButton::Left) && i.down(MouseButton::Left));
        i.begin_frame();
        assert!(!i.pressed(MouseButton::Left) && i.down(MouseButton::Left));
        i.on_button(MouseButton::Left, false);
        assert!(i.released(MouseButton::Left) && !i.down(MouseButton::Left));
        i.begin_frame();
        assert!(!i.released(MouseButton::Left));
    }

    #[test]
    fn mouse_delta_spans_frames() {
        let mut i = Input::default();
        i.on_cursor(Vec2::new(10.0, 10.0));
        i.begin_frame();
        i.on_cursor(Vec2::new(15.0, 12.0));
        assert_eq!(i.mouse_delta(), Vec2::new(5.0, 2.0));
    }
}
