//! Performance (2/4 deck), Library/prep, Settings composition.
//!
//! Performance layout (Alva's mockup): four stacked waveforms across the top,
//! decks 1/3 left and 2/4 right around a center mixer column, browser along
//! the bottom. Everything derives from the window rect — no magic coordinates.

use crate::settings::Settings;
use crate::wiring::Audio;
use crate::workers::Loader;
use lmx_analysis::FINE_FRAMES;
use lmx_audio::AudioState;
use lmx_engine::Snapshot;
use lmx_gpu::WaveId;
use lmx_ui::waveform::StripView;
use lmx_ui::{Color, Rect, UiFrame, Vec2};
use std::path::PathBuf;

/// Seconds of audio visible across a waveform strip.
const STRIP_SECONDS: f32 = 24.0;

/// What the UI knows about a loaded deck (the engine knows the audio).
#[derive(Clone, Debug, Default)]
pub struct DeckView {
    pub title: String,
    pub artist: String,
    pub wave: Option<WaveId>,
    pub columns: u32,
    pub sample_rate: u32,
    pub frames: u64,
    pub bpm_tag: Option<f32>,
    pub key_tag: Option<String>,
}

pub struct PerformanceScreen {
    pub decks: [DeckView; 4],
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
    /// Rects of the last frame's drop targets: (deck, rect).
    drop_zones: Vec<(usize, Rect)>,
}

impl Default for PerformanceScreen {
    fn default() -> Self {
        Self {
            decks: Default::default(),
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
            drop_zones: Vec::new(),
        }
    }
}

const DECK_NAMES: [&str; 4] = ["DECK 1", "DECK 2", "DECK 3", "DECK 4"];

fn fmt_time(secs: f64) -> String {
    let neg = secs < 0.0;
    let s = secs.abs();
    let m = (s / 60.0).floor();
    let r = s - m * 60.0;
    format!("{}{:02}:{:04.1}", if neg { "-" } else { "" }, m as u32, r)
}

impl PerformanceScreen {
    /// Deck under `p` for a dropped file, from the last frame's layout.
    pub fn drop_target(&self, p: Vec2) -> usize {
        self.drop_zones.iter().find(|(_, r)| r.contains(p)).map(|(d, _)| *d).unwrap_or(0)
    }

    pub fn draw(&mut self, f: &mut UiFrame, audio: &mut Audio, loader: &Loader, settings: &Settings, snap: &Snapshot, area: Rect, bar_free: Rect) {
        let th = f.theme().clone();
        let gap = th.gap;

        if snap.decks.iter().any(|d| d.playing) || self.tone || (0..4).any(|d| loader.progress(d).is_some()) {
            f.animate();
        }

        // dropped files → load into the deck under the pointer
        let drops: Vec<PathBuf> = f.input.dropped_files.clone();
        if !drops.is_empty() {
            let deck = self.drop_target(f.input.mouse);
            for (i, p) in drops.into_iter().enumerate() {
                loader.load((deck + i) % 4, p);
            }
        }
        self.drop_zones.clear();

        // ── title-bar status: tone + audio state ──
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

        let strip_h = ((waves.h - 3.0 * 5.0) / 4.0).floor();
        for (slot, deck) in settings.wave_order.decks().into_iter().enumerate() {
            let strip = Rect::new(waves.x, waves.y + slot as f32 * (strip_h + 5.0), waves.w, strip_h);
            self.strip(f, deck, strip, snap, loader);
            self.drop_zones.push((deck, strip));
        }

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
            self.deck(f, i, rect, snap, audio, loader);
            f.pop_scope();
            self.drop_zones.push((i, rect));
        }
        self.mixer(f, mixer, snap);
        self.browser(f, browser);

        audio.set_tone(self.tone, self.master);
    }

    fn strip(&mut self, f: &mut UiFrame, i: usize, rect: Rect, snap: &Snapshot, loader: &Loader) {
        let dv = &self.decks[i];
        let ds = snap.decks[i];
        let cols_per_px = if dv.sample_rate > 0 {
            (STRIP_SECONDS * dv.sample_rate as f32 / FINE_FRAMES as f32) / (rect.w - 10.0).max(1.0)
        } else {
            1.0
        };
        let view = StripView {
            wave: if ds.loaded { dv.wave } else { None },
            columns: dv.columns,
            playhead_col: (ds.pos / FINE_FRAMES as f64) as f32,
            cols_per_px,
            deck: i,
            playing: ds.playing,
        };
        let label = match loader.progress(i) {
            Some(p) => format!("{}  LOADING {:.0}%", DECK_NAMES[i], p * 100.0),
            None if dv.title.is_empty() => DECK_NAMES[i].to_string(),
            None => format!("{}  {}", DECK_NAMES[i], dv.title),
        };
        f.waveform_strip(rect, &view, &label);
    }

    fn deck(&mut self, f: &mut UiFrame, i: usize, rect: Rect, snap: &Snapshot, audio: &mut Audio, loader: &Loader) {
        let th = f.theme().clone();
        let color = th.deck[i];
        let ds = snap.decks[i];
        f.panel(rect);
        f.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        let mut r = rect.inset(th.pad);
        r.cut_left(5.0);

        let mut head = r.cut_top(35.0);
        let tag = head.cut_left(110.0);
        f.text_left(tag, DECK_NAMES[i], th.text, color);
        let dv = &self.decks[i];
        let (line, col) = match loader.progress(i) {
            Some(p) => (format!("Loading… {:.0}%", p * 100.0), th.fg_dim),
            None if dv.title.is_empty() => ("No track".to_string(), th.fg_dim),
            None if dv.artist.is_empty() => (dv.title.clone(), th.fg),
            None => (format!("{} — {}", dv.title, dv.artist), th.fg),
        };
        f.push_clip(head);
        f.text_left(head, &line, th.text, col);
        f.pop_clip();

        r.cut_top(5.0);
        let mut row = r.cut_top(60.0);
        let bpm = row.cut_left((row.w * 0.36).max(180.0).min(260.0));
        let bpm_s = dv.bpm_tag.map(|b| format!("{b:.2}")).unwrap_or_else(|| "---.--".into());
        f.readout(bpm, &bpm_s, "BPM", if dv.bpm_tag.is_some() { th.fg } else { th.fg_dim });
        let key = row.cut_left((row.w * 0.3).max(120.0).min(160.0));
        let key_s = dv.key_tag.clone().unwrap_or_else(|| "--".into());
        f.readout(key, &key_s, "KEY", if dv.key_tag.is_some() { th.fg } else { th.fg_dim });
        let time = if ds.loaded && ds.sample_rate > 0 {
            let remaining = (ds.frames as f64 - ds.pos) / ds.sample_rate as f64;
            fmt_time(-remaining)
        } else {
            "--:--.-".into()
        };
        f.readout(row, &time, "", th.fg);

        r.cut_top(5.0);
        let lamps = r.cut_top(45.0);
        let cells = lmx_ui::layout::hstack(lamps, &[0.0, 0.0, 0.0, 0.0, 60.0], 5.0);
        f.push_scope(1);
        self.play[i] = ds.playing;
        if f.toggle_colored(cells[0], "PLAY", &mut self.play[i], color) {
            audio.play(i, self.play[i]);
        }
        f.toggle(cells[1], "SYNC", &mut self.sync[i]);
        f.toggle(cells[2], "KEYLOCK", &mut self.keylock[i]);
        f.toggle(cells[3], "SLIP", &mut self.slip[i]);
        f.toggle(cells[4], "Q", &mut self.quantize[i]);
        f.pop_scope();

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

    fn mixer(&mut self, f: &mut UiFrame, rect: Rect, snap: &Snapshot) {
        let th = f.theme().clone();
        f.panel(rect);
        let mut r = rect.inset(th.pad);
        let gutter_w = 60.0;
        let kr = 25.0;
        let knob_row = kr * 2.0 + 5.0;

        let mut head = r.cut_top(30.0);
        head.cut_left(gutter_w);
        let cols = lmx_ui::layout::hstack(head, &[0.0; 5], 5.0);
        for (n, c) in cols.iter().enumerate() {
            let (name, color) = if n == 4 { ("MST", th.fg_dim) } else { (["1", "2", "3", "4"][n], th.deck[n]) };
            f.text_centered(*c, name, th.text_small, color);
        }
        r.cut_top(5.0);

        for (row_i, label) in ["TRIM", "HI", "MID", "LOW"].iter().enumerate() {
            if r.h < knob_row + 60.0 {
                break;
            }
            let mut row = r.cut_top(knob_row);
            let g = row.cut_left(gutter_w);
            f.text_left(g, label, th.text_small, th.fg_dim);
            let cells = lmx_ui::layout::hstack(row, &[0.0; 5], 5.0);
            for ch in 0..4 {
                f.push_scope((row_i * 4 + ch) as u64);
                let v = if row_i == 0 { &mut self.trim[ch] } else { &mut self.eq[ch][row_i - 1] };
                f.knob(cells[ch].center(), kr, v, "", th.deck[ch]);
                f.pop_scope();
            }
            if row_i == 0 {
                f.push_scope(99);
                f.knob(cells[4].center(), kr, &mut self.master, "", th.fg_dim);
                f.pop_scope();
            }
        }

        r.cut_top(5.0);
        let xf_h = if r.h > 160.0 { 45.0 } else { 0.0 };
        let cue_h = 40.0;
        let mut body = r.cut_top((r.h - xf_h - cue_h - 10.0).max(40.0));
        body.cut_left(gutter_w);
        let cells = lmx_ui::layout::hstack(body, &[0.0; 5], 5.0);
        let db = |lin: f32| if lin > 1e-6 { 20.0 * lin.log10() } else { -120.0 };
        for ch in 0..4 {
            let mut c = cells[ch];
            let meter = c.cut_right(20.0);
            c.cut_right(5.0);
            let fader = c.centered(c.w.min(40.0), c.h);
            f.push_scope(200 + ch as u64);
            f.vfader(fader, &mut self.fader[ch]);
            f.pop_scope();
            f.meter_mono(meter, db(snap.decks[ch].peak));
        }
        let mst = cells[4].centered(cells[4].w.min(60.0), cells[4].h);
        f.meter(mst, db(snap.master_peak[0]), db(snap.master_peak[1]));

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
