//! The browser panel: tree column, search field, virtualized track table with
//! sortable headers, selection, keys 1–4 to load, row drag onto decks.

use crate::workers::Loader;
use lmx_library::{search, Library, SortBy, Track, TrackId};
use lmx_ui::{Color, Key, Rect, UiFrame, Vec2};
use std::path::PathBuf;

const ROW_H: f32 = 50.0;
const DRAG_START_PX: f32 = 10.0;

pub struct Browser {
    pub scanning: bool,
    query: String,
    search_focus: bool,
    sort: SortBy,
    desc: bool,
    view: Vec<usize>,
    view_dirty: bool,
    /// Library generation the view was built against.
    view_gen: u64,
    selected: Option<TrackId>,
    scroll: f32,
    /// A row press that may become a drag: (track, press position).
    press: Option<(TrackId, Vec2)>,
    /// Track being dragged over the window.
    drag: Option<TrackId>,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            scanning: false,
            query: String::new(),
            search_focus: false,
            sort: SortBy::Manual,
            desc: false,
            view: Vec::new(),
            view_dirty: true,
            view_gen: u64::MAX,
            selected: None,
            scroll: 0.0,
            press: None,
            drag: None,
        }
    }
}

/// What the browser asked the app to do this frame.
#[derive(Default)]
pub struct BrowserActions {
    /// (deck, track) to load.
    pub load: Vec<(usize, TrackId)>,
    pub add_roots: Vec<PathBuf>,
    /// Put a track before another (`None` = last) in the manual order.
    pub reorder: Option<(TrackId, Option<TrackId>)>,
}

fn fmt_dur(secs: f64) -> String {
    let m = (secs / 60.0).floor();
    format!("{:02}:{:02}", m as u32, (secs - m * 60.0).floor() as u32)
}

impl Browser {
    fn refresh_view(&mut self, lib: &Library) {
        if self.view_dirty || self.view_gen != lib.generation {
            self.view = search::view(lib, &self.query, self.sort, self.desc);
            self.view_dirty = false;
            self.view_gen = lib.generation;
        }
    }

    fn selected_row(&self, lib: &Library) -> Option<usize> {
        let id = self.selected?;
        self.view.iter().position(|&i| lib.tracks[i].id == id)
    }

    /// The track currently being dragged (for the ghost + drop handling).
    pub fn dragging<'a>(&self, lib: &'a Library) -> Option<&'a Track> {
        self.drag.and_then(|id| lib.get(id))
    }

    /// Draw the panel. `drop_target` maps a pointer position to a deck.
    pub fn draw(&mut self, f: &mut UiFrame, rect: Rect, lib: &Library, loader: &Loader, drop_target: &dyn Fn(Vec2) -> Option<usize>) -> BrowserActions {
        let th = f.theme().clone();
        let mut actions = BrowserActions::default();
        let _ = loader;
        f.panel(rect);
        let mut r = rect.inset(th.pad);

        // folders dropped on the browser become roots
        for p in &f.input.dropped_files {
            if p.is_dir() {
                actions.add_roots.push(p.clone());
            }
        }

        // ── sidebar: search on top, tree below ──
        let side = r.cut_left(300.0);
        r.cut_left(th.gap);
        f.p.fill_rrect(side, 5.0, th.well_deep);
        f.p.fill_rect(Rect::new(side.right() - 5.0, side.y, 5.0, side.h), th.border);
        let mut side = side.inset(10.0);
        let field = side.cut_top(50.0);
        if f.text_field(field, &mut self.query, &mut self.search_focus) {
            self.view_dirty = true;
        }
        // magnifier at the right edge of the field
        let mc = Vec2::new(field.right() - 25.0, field.center().y - 5.0);
        f.p.circle_stroke(mc, 10.0, 5.0, th.fg_dim);
        f.p.line(mc + Vec2::new(7.0, 7.0), mc + Vec2::new(15.0, 15.0), 5.0, th.fg_dim);
        side.cut_top(th.gap);
        let count = lib.len();
        let names = [
            if self.scanning { "COLLECTION  …".to_string() } else { format!("COLLECTION  {count}") },
            "PLAYLISTS".to_string(),
            "TAGS".to_string(),
            "HISTORY".to_string(),
        ];
        for (n, name) in names.iter().enumerate() {
            if side.h < 50.0 {
                break;
            }
            let row = side.cut_top(50.0);
            if n == 0 {
                f.p.fill_rrect(row, 5.0, th.well);
            }
            f.push_clip(row);
            f.text_left(row.inset_xy(10.0, 0.0), name, th.text, if n == 0 { th.fg } else { th.fg_dim });
            f.pop_clip();
        }

        // ── header: # (manual order) then the sortable columns ──
        let head = r.cut_top(35.0);
        let widths = [70.0, 0.0, 0.0, 120.0, 100.0, 120.0];
        let cols = lmx_ui::layout::hstack(Rect::new(head.x, head.y, head.w - 20.0, head.h), &widths, th.gap);
        let names = ["#", "TITLE", "ARTIST", "BPM", "KEY", "TIME"];
        let sorts = [SortBy::Manual, SortBy::Title, SortBy::Artist, SortBy::Bpm, SortBy::Key, SortBy::Time];
        for (i, c) in cols.iter().enumerate() {
            f.push_scope(i as u64);
            let id = f.id();
            let it = f.interact(id, *c);
            let active = self.sort == sorts[i];
            let label = if active && i > 0 { format!("{} {}", names[i], if self.desc { "▼" } else { "▲" }) } else { names[i].to_string() };
            f.text_left(*c, &label, th.text_small, if active || it.hovered { th.fg } else { th.fg_dim });
            if it.clicked {
                if active && i > 0 {
                    self.desc = !self.desc;
                } else {
                    self.sort = sorts[i];
                    self.desc = false;
                }
                self.view_dirty = true;
            }
            f.pop_scope();
        }
        f.p.fill_rect(Rect::new(r.x, head.bottom(), r.w, 5.0), th.border);
        r.cut_top(10.0);

        // ── keyboard: selection + load ──
        self.refresh_view(lib);
        let mut keep = None;
        if !self.search_focus && !self.view.is_empty() {
            let cur = self.selected_row(lib);
            let mut next = cur;
            for _ in 0..f.input.key_count(Key::Down) {
                next = Some(next.map(|i| (i + 1).min(self.view.len() - 1)).unwrap_or(0));
            }
            for _ in 0..f.input.key_count(Key::Up) {
                next = Some(next.map(|i| i.saturating_sub(1)).unwrap_or(0));
            }
            if f.input.key(Key::Home) {
                next = Some(0);
            }
            if f.input.key(Key::End) {
                next = Some(self.view.len() - 1);
            }
            if next != cur {
                if let Some(i) = next {
                    self.selected = Some(lib.tracks[self.view[i]].id);
                    keep = Some(i);
                }
            }
            for ch in f.input.text.chars() {
                if let Some(d) = ch.to_digit(10) {
                    if (1..=4).contains(&d) {
                        if let Some(id) = self.selected {
                            actions.load.push((d as usize - 1, id));
                        }
                    }
                }
            }
        }

        // ── rows ──
        let mouse = f.input.mouse;
        // dragging near the top/bottom edge scrolls the list
        if self.drag.is_some() && r.contains(mouse) {
            if mouse.y < r.y + 40.0 {
                self.scroll -= 8.0;
            } else if mouse.y > r.bottom() - 40.0 {
                self.scroll += 8.0;
            }
        }
        let rows = f.rows(r, ROW_H, self.view.len(), &mut self.scroll, keep);
        // where a dragged row would land: index of the row boundary nearest the pointer
        let reorder_ok = self.sort == SortBy::Manual;
        let insert_at = if self.drag.is_some() && reorder_ok && rows.area.contains(mouse) {
            Some((((mouse.y - rows.area.y + self.scroll) / ROW_H).round().max(0.0) as usize).min(self.view.len()))
        } else {
            None
        };
        f.push_clip(rows.area);
        let mut clicked: Option<TrackId> = None;
        for i in rows.range.clone() {
            let t = &lib.tracks[self.view[i]];
            let row = rows.rect(i);
            f.push_scope(i as u64);
            let id = f.id();
            let it = f.interact(id, row);
            f.pop_scope();
            let selected = self.selected == Some(t.id);
            if selected {
                f.p.fill_rrect(row, 5.0, th.well);
            } else if i % 2 == 1 {
                f.p.fill_rect(row, Color::rgba(1.0, 1.0, 1.0, 0.03));
            }
            if it.hovered && !selected && self.drag.is_none() {
                f.p.fill_rrect(row, 5.0, Color::rgba(1.0, 1.0, 1.0, 0.05));
            }
            if it.pressed {
                clicked = Some(t.id);
                self.press = Some((t.id, mouse));
            }
            if it.held {
                if let Some((pid, origin)) = self.press {
                    if pid == t.id && self.drag.is_none() && mouse.dist(origin) > DRAG_START_PX {
                        self.drag = Some(t.id);
                    }
                }
            }
            if it.released {
                if let Some(dragged) = self.drag {
                    if let Some(at) = insert_at {
                        let before = self.view.get(at).map(|&vi| lib.tracks[vi].id);
                        if before != Some(dragged) {
                            actions.reorder = Some((dragged, before));
                        }
                    } else if let Some(deck) = drop_target(mouse) {
                        actions.load.push((deck, dragged));
                    }
                }
                self.drag = None;
                self.press = None;
            }
            let cells = lmx_ui::layout::hstack(Rect::new(row.x, row.y, row.w, row.h), &widths, th.gap);
            let fg = if t.missing { th.warn } else { th.fg };
            f.text_right(cells[0].inset_xy(10.0, 0.0), &format!("{}", i + 1), th.text_small, th.fg_dim);
            f.push_clip(cells[1]);
            f.text_left(cells[1].inset_xy(5.0, 0.0), t.display_title(), th.text, fg);
            f.pop_clip();
            f.push_clip(cells[2]);
            f.text_left(cells[2], &t.artist, th.text, th.fg_dim);
            f.pop_clip();
            let bpm = t.bpm().map(|b| format!("{b:.1}")).unwrap_or_else(|| "—".into());
            f.text_left(cells[3], &bpm, th.text, if t.grid.bpm > 0.0 { th.fg } else { th.fg_dim });
            let key = t.key_tag.clone().unwrap_or_else(|| "—".into());
            f.text_left(cells[4], &key, th.text, th.fg_dim);
            f.text_left(cells[5], &fmt_dur(t.duration_secs), th.text, th.fg_dim);
        }
        if let Some(at) = insert_at {
            let y = rows.area.y + at as f32 * ROW_H - self.scroll;
            f.p.set_layer(1);
            f.p.fill_rrect(Rect::new(rows.area.x, y - 2.5, rows.area.w, 5.0), 2.5, th.accent);
            f.p.set_layer(0);
        }
        f.pop_clip();
        if let Some(id) = clicked {
            self.selected = Some(id);
            self.search_focus = false;
        }
        if self.drag.is_some() {
            f.animate();
        }

        // ── drag ghost ──
        if let Some(t) = self.dragging(lib) {
            let label = t.display_title().to_string();
            let w = f.t.width(&label, th.text) + 30.0;
            let g = Rect::new(mouse.x + 15.0, mouse.y - 25.0, w, 50.0);
            f.p.set_layer(2);
            f.p.fill_rrect(g, 5.0, th.well);
            f.p.stroke_rrect(g, 5.0, th.stroke, th.accent);
            f.p.set_layer(0);
            f.occlude(g);
            f.set_late_mode(true);
            f.text_centered(g, &label, th.text, th.fg);
            f.set_late_mode(false);
        }
        actions
    }
}
