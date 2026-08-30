//! Client-side title bar: the compositor's decorations are turned off and this
//! draws our own — drag area, edge/corner resize zones, and the standard
//! minimize / maximize / close controls on the right.

use lmx_ui::{Rect, UiFrame, Vec2};
use std::time::Instant;
use winit::window::{CursorIcon, ResizeDirection};

pub const HEIGHT: f32 = 40.0;
const BUTTON_W: f32 = 50.0;
const ICON: f32 = 15.0;
const EDGE: f32 = 10.0;
const CORNER: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TitleAction {
    None,
    Minimize,
    ToggleMaximize,
    Close,
    Drag,
    Resize(ResizeDirection),
}

#[derive(Default)]
pub struct TitleBar {
    last_click: Option<Instant>,
}

impl TitleBar {
    /// Draws the bar across the top of the window. Returns what the user asked
    /// for, the cursor the window should show, and the bar's free left area
    /// (the screen may put small status widgets there).
    pub fn draw(&mut self, f: &mut UiFrame, maximized: bool) -> (TitleAction, CursorIcon, Rect) {
        let th = f.theme().clone();
        let bar = Rect::new(0.0, 0.0, f.size.x, HEIGHT);
        f.p.fill_rect(bar, th.panel);

        let mut action = TitleAction::None;
        let mut cursor = CursorIcon::Default;

        // ── resize zones (only when not maximized) ──
        if !maximized {
            if let Some(dir) = edge_hit(f.input.mouse, f.size) {
                cursor = match dir {
                    ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
                    ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
                    ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
                    ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
                };
                if f.input.pressed(lmx_ui::MouseButton::Left) {
                    return (TitleAction::Resize(dir), cursor, Rect::ZERO);
                }
            }
        }

        // ── window controls, right side ──
        let mut right = bar;
        let close = right.cut_right(BUTTON_W);
        let max = right.cut_right(BUTTON_W);
        let min = right.cut_right(BUTTON_W);
        if self.control(f, min, Icon::Minimize) {
            action = TitleAction::Minimize;
        }
        if self.control(f, max, if maximized { Icon::Restore } else { Icon::Maximize }) {
            action = TitleAction::ToggleMaximize;
        }
        if self.control(f, close, Icon::Close) {
            action = TitleAction::Close;
        }

        // ── drag area: whatever is left of the bar (status widgets go in
        // the left half; they're drawn by the screen after this, on top) ──
        let free = Rect::new(right.x + 10.0, right.y + 5.0, (right.w * 0.5).max(0.0), right.h - 10.0);
        let id = f.id();
        let it = f.interact(id, right);
        if it.pressed {
            let now = Instant::now();
            let double = self.last_click.map(|t| now.duration_since(t).as_millis() < 400).unwrap_or(false);
            self.last_click = Some(now);
            if double {
                self.last_click = None;
                action = TitleAction::ToggleMaximize;
            } else if action == TitleAction::None {
                action = TitleAction::Drag;
            }
        }
        (action, cursor, free)
    }

    #[track_caller]
    fn control(&mut self, f: &mut UiFrame, rect: Rect, icon: Icon) -> bool {
        let th = f.theme().clone();
        let id = f.id();
        let it = f.interact(id, rect);
        let hot = it.hovered || it.held;
        if hot {
            let fill = if icon == Icon::Close { th.warn } else { th.well };
            f.p.fill_rect(rect, fill);
        }
        let c = if hot { th.fg } else { th.fg_dim };
        let ic = rect.centered(ICON, ICON);
        let w = th.line;
        match icon {
            Icon::Minimize => {
                let y = ic.center().y;
                f.p.line(Vec2::new(ic.x, y), Vec2::new(ic.right(), y), w, c);
            }
            Icon::Maximize => f.p.stroke_rrect(ic, 0.0, w, c),
            Icon::Restore => {
                let a = Rect::new(ic.x, ic.y + 5.0, ICON - 5.0, ICON - 5.0);
                let b = Rect::new(ic.x + 5.0, ic.y, ICON - 5.0, ICON - 5.0);
                f.p.stroke_rrect(b, 0.0, w, c);
                f.p.fill_rect(a, if hot { th.well } else { th.panel });
                f.p.stroke_rrect(a, 0.0, w, c);
            }
            Icon::Close => {
                f.p.line(ic.min(), ic.max(), w, c);
                f.p.line(Vec2::new(ic.right(), ic.y), Vec2::new(ic.x, ic.bottom()), w, c);
            }
        }
        it.clicked
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Icon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn edge_hit(m: Vec2, size: Vec2) -> Option<ResizeDirection> {
    let l = m.x < EDGE;
    let r = m.x > size.x - EDGE;
    let t = m.y < EDGE;
    let b = m.y > size.y - EDGE;
    let cl = m.x < CORNER;
    let cr = m.x > size.x - CORNER;
    let ct = m.y < CORNER;
    let cb = m.y > size.y - CORNER;
    Some(match (l, r, t, b) {
        _ if (l || t) && cl && ct => ResizeDirection::NorthWest,
        _ if (r || t) && cr && ct => ResizeDirection::NorthEast,
        _ if (l || b) && cl && cb => ResizeDirection::SouthWest,
        _ if (r || b) && cr && cb => ResizeDirection::SouthEast,
        (true, _, _, _) => ResizeDirection::West,
        (_, true, _, _) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}
