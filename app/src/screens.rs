//! Performance (2/4 deck), Library/prep, Settings composition.
//!
//! Performance layout (Alva's mockup): four stacked waveforms across the top,
//! decks 1/3 left and 2/4 right around a center mixer column, browser along
//! the bottom. Everything derives from the window rect — no magic coordinates.

use crate::wiring::Audio;
use lmx_audio::AudioState;
use lmx_ui::{Color, Rect, UiFrame, Vec2};

pub struct PerformanceScreen {
    /// Per-deck placeholder state until the engine exists.
    play: [bool; 4],
    sync: [bool; 4],
    keylock: [bool; 4],
    slip: [bool; 4],
    quantize: [bool; 4],
    pfl: [bool; 4],
    trim: [f32; 4],
    eq: [[f32; 3]; 4],
    fader: [f32; 4],
    xfader: f32,
    master: f32,
    tone: bool,
    levels: (f32, f32),
}

impl Default for PerformanceScreen {
    fn default() -> Self {
        Self {
            play: [false; 4],
            sync: [false; 4],
            keylock: [true; 4],
            slip: [false; 4],
            quantize: [true; 4],
            pfl: [false; 4],
            trim: [0.5; 4],
            eq: [[0.5; 3]; 4],
            fader: [0.85, 0.85, 0.0, 0.0],
            xfader: 0.5,
            master: 0.75,
            tone: false,
            levels: (-120.0, -120.0),
        }
    }
}

const DECK_NAMES: [&str; 4] = ["DECK 1", "DECK 2", "DECK 3", "DECK 4"];

impl PerformanceScreen {
    pub fn draw(&mut self, f: &mut UiFrame, audio: &mut Audio, area: Rect, bar_free: Rect) {
        let th = f.theme().clone();
        let gap = th.gap;

        let lv = audio.levels();
        self.levels = (lv.peak_db[0], lv.peak_db[1]);
        if self.tone {
            f.animate();
        }

        // ── title-bar status: test tone + audio state (temporary until decks play) ──
        if bar_free.w > 300.0 {
            let mut b = bar_free;
            let tb = b.cut_left(140.0);
            f.toggle(tb, "TONE", &mut self.tone);
            b.cut_left(gap);
            let status = match (audio.state(), &audio.error) {
                (_, Some(e)) => format!("AUDIO ERROR  {e}"),
                (AudioState::Streaming, None) => format!("{} Hz · {}", audio.rate(), audio.block()),
                (s, None) => format!("{s:?}"),
            };
            let col = if audio.error.is_some() { th.warn } else { th.fg_dim };
            f.text_left(b, &status, th.text_small, col);
            let x = audio.xruns();
            if x > 0 {
                f.text_right(b, &format!("XRUNS {x}"), th.text_small, th.warn);
            }
        }

        // ── vertical split: waveforms 32% · decks 40% · browser rest ──
        let mut r = area.inset(10.0);
        let wave_h = (r.h * 0.32).floor();
        let deck_h = (r.h * 0.40).floor();
        let waves = r.cut_top(wave_h);
        r.cut_top(gap);
        let decks = r.cut_top(deck_h);
        r.cut_top(gap);
        let browser = r;

        // waveform strips, deck order 1-2-3-4 top to bottom
        let strip_h = ((waves.h - 3.0 * 5.0) / 4.0).floor();
        for i in 0..4 {
            let strip = Rect::new(waves.x, waves.y + i as f32 * (strip_h + 5.0), waves.w, strip_h);
            self.waveform_placeholder(f, i, strip);
        }

        // decks + mixer: left 38% · mixer 24% · right 38%
        let side_w = ((decks.w - 2.0 * gap) * 0.38).floor();
        let mut d = decks;
        let left = d.cut_left(side_w);
        d.cut_left(gap);
        let right = d.cut_right(side_w);
        d.cut_right(gap);
        let mixer = d;
        let row_h = ((decks.h - gap) * 0.5).floor();
        let panels = [
            (0usize, Rect::new(left.x, left.y, left.w, row_h)),
            (2, Rect::new(left.x, left.y + row_h + gap, left.w, row_h)),
            (1, Rect::new(right.x, right.y, right.w, row_h)),
            (3, Rect::new(right.x, right.y + row_h + gap, right.w, row_h)),
        ];
        for (i, rect) in panels {
            f.push_scope(i as u64);
            self.deck(f, i, rect);
            f.pop_scope();
        }
        self.mixer(f, mixer);

        self.browser(f, browser);

        // control state → audio thread, after the widgets ran
        audio.tone.set_on(self.tone);
        audio.tone.gain.store(self.master);
    }

    fn waveform_placeholder(&mut self, f: &mut UiFrame, i: usize, rect: Rect) {
        let th = f.theme().clone();
        let color = th.deck[i];
        f.p.fill_rrect(rect, 5.0, th.well_deep);
        f.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        // playhead
        let cx = rect.center().x;
        f.p.line(Vec2::new(cx, rect.y), Vec2::new(cx, rect.bottom()), th.line, th.fg.with_alpha(0.9));
        let tag = Rect::new(rect.x + 20.0, rect.y, 200.0, rect.h.min(35.0));
        f.text_left(tag, DECK_NAMES[i], th.text_small, color);
    }

    fn deck(&mut self, f: &mut UiFrame, i: usize, rect: Rect) {
        let th = f.theme().clone();
        let color = th.deck[i];
        f.panel(rect);
        f.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        let mut r = rect.inset(th.pad);
        r.cut_left(5.0);

        // header: deck tag · title · artist
        let mut head = r.cut_top(35.0);
        let tag = head.cut_left(110.0);
        f.text_left(tag, DECK_NAMES[i], th.text, color);
        let title = "No track";
        f.text_left(head, title, th.text, th.fg_dim);

        // readouts
        r.cut_top(5.0);
        let mut row = r.cut_top(60.0);
        let bpm = row.cut_left((row.w * 0.36).max(180.0).min(260.0));
        f.readout(bpm, "---.--", "BPM", th.fg);
        let key = row.cut_left((row.w * 0.3).max(120.0).min(160.0));
        f.readout(key, "--", "KEY", th.fg_dim);
        f.readout(row, "--:--.-", "", th.fg);

        // state lamps
        r.cut_top(5.0);
        let lamps = r.cut_top(45.0);
        let names = ["PLAY", "SYNC", "KEYLOCK", "SLIP", "Q"];
        let widths = [0.0, 0.0, 0.0, 0.0, 60.0];
        let cells = lmx_ui::layout::hstack(lamps, &widths, 5.0);
        f.push_scope(1);
        f.toggle_colored(cells[0], names[0], &mut self.play[i], color);
        f.toggle(cells[1], names[1], &mut self.sync[i]);
        f.toggle(cells[2], names[2], &mut self.keylock[i]);
        f.toggle(cells[3], names[3], &mut self.slip[i]);
        f.toggle(cells[4], names[4], &mut self.quantize[i]);
        f.pop_scope();

        // hot cues, only when there's room for a real row
        r.cut_top(5.0);
        if r.h >= 50.0 {
            let pads = r.cut_top(r.h.min(60.0));
            let cells = lmx_ui::layout::hstack(pads, &[0.0; 8], 5.0);
            for (n, c) in cells.iter().enumerate() {
                f.p.fill_rrect(*c, 5.0, th.well_deep);
                f.p.stroke_rrect(*c, 5.0, th.stroke, th.border);
                let label = ["A", "B", "C", "D", "E", "F", "G", "H"][n];
                f.text_centered(*c, label, th.text_small, th.fg_dim);
            }
        }
    }

    fn mixer(&mut self, f: &mut UiFrame, rect: Rect) {
        let th = f.theme().clone();
        f.panel(rect);
        let mut r = rect.inset(th.pad);
        let gutter_w = 60.0;
        let kr = 25.0;
        let knob_row = kr * 2.0 + 5.0;

        // column headers
        let mut head = r.cut_top(30.0);
        head.cut_left(gutter_w);
        let cols = lmx_ui::layout::hstack(head, &[0.0; 5], 5.0);
        let col_names = ["1", "2", "3", "4", "MST"];
        for (c, name) in cols.iter().zip(col_names) {
            let color = if c == &cols[4] { th.fg_dim } else { th.deck[cols.iter().position(|x| x == c).unwrap()] };
            f.text_centered(*c, name, th.text_small, color);
        }
        r.cut_top(5.0);

        // knob rows with a label gutter: TRIM HI MID LOW
        let labels = ["TRIM", "HI", "MID", "LOW"];
        for (row_i, label) in labels.iter().enumerate() {
            if r.h < knob_row + 60.0 {
                break;
            }
            let mut row = r.cut_top(knob_row);
            let g = row.cut_left(gutter_w);
            f.text_left(g, label, th.text_small, th.fg_dim);
            let cells = lmx_ui::layout::hstack(row, &[0.0; 5], 5.0);
            for ch in 0..4 {
                let c = cells[ch].center();
                f.push_scope((row_i * 4 + ch) as u64);
                let v = if row_i == 0 { &mut self.trim[ch] } else { &mut self.eq[ch][row_i - 1] };
                f.knob(c, kr, v, "", th.deck[ch]);
                f.pop_scope();
            }
            if row_i == 0 {
                f.push_scope(99);
                f.knob(cells[4].center(), kr, &mut self.master, "", th.fg_dim);
                f.pop_scope();
            }
        }

        // faders + meters, CUE lamps, crossfader
        r.cut_top(5.0);
        let xf_h = if r.h > 160.0 { 45.0 } else { 0.0 };
        let cue_h = 40.0;
        let mut body = r.cut_top((r.h - xf_h - cue_h - 10.0).max(40.0));
        body.cut_left(gutter_w);
        let cells = lmx_ui::layout::hstack(body, &[0.0; 5], 5.0);
        for ch in 0..4 {
            let mut c = cells[ch];
            let meter = c.cut_right(20.0);
            c.cut_right(5.0);
            let fader = c.centered(c.w.min(40.0), c.h);
            f.push_scope(200 + ch as u64);
            f.vfader(fader, &mut self.fader[ch]);
            f.pop_scope();
            let db = if ch == 0 { self.levels.0 } else { -120.0 };
            f.meter_mono(meter, db);
        }
        let mst = cells[4].centered(cells[4].w.min(60.0), cells[4].h);
        f.meter(mst, self.levels.0, self.levels.1);

        r.cut_top(5.0);
        let mut cue_row = r.cut_top(cue_h);
        cue_row.cut_left(gutter_w);
        let cells = lmx_ui::layout::hstack(cue_row, &[0.0; 5], 5.0);
        for ch in 0..4 {
            f.push_scope(300 + ch as u64);
            f.toggle_colored(cells[ch], "CUE", &mut self.pfl[ch], th.ok);
            f.pop_scope();
        }

        if xf_h > 0.0 {
            r.cut_top(5.0);
            let mut xr = r.cut_top(xf_h);
            xr.cut_left(gutter_w);
            f.crossfader(xr, &mut self.xfader);
        }
    }

    fn browser(&mut self, f: &mut UiFrame, rect: Rect) {
        let th = f.theme().clone();
        f.panel(rect);
        let mut r = rect.inset(th.pad);

        // tree column
        let mut tree = r.cut_left(250.0);
        r.cut_left(th.gap);
        for (n, name) in ["COLLECTION", "PLAYLISTS", "TAGS", "HISTORY"].iter().enumerate() {
            if tree.h < 50.0 {
                break;
            }
            let row = tree.cut_top(50.0);
            if n == 0 {
                f.p.fill_rrect(row, 5.0, th.well);
            }
            f.text_left(row.inset_xy(10.0, 0.0), name, th.text, if n == 0 { th.fg } else { th.fg_dim });
        }

        // table: header + zebra rows
        let head = r.cut_top(35.0);
        let cols = lmx_ui::layout::hstack(head, &[0.0, 0.0, 120.0, 100.0, 120.0], th.gap);
        for (c, name) in cols.iter().zip(["TITLE", "ARTIST", "BPM", "KEY", "TIME"]) {
            f.text_left(*c, name, th.text_small, th.fg_dim);
        }
        f.p.fill_rect(Rect::new(r.x, head.bottom(), r.w, 5.0), th.border);
        r.cut_top(10.0);
        let mut n = 0;
        while r.h >= 50.0 {
            let row = r.cut_top(50.0);
            if n % 2 == 1 {
                f.p.fill_rect(row, Color::rgba(1.0, 1.0, 1.0, 0.03));
            }
            n += 1;
        }
    }
}
