//! Performance (2/4 deck), Library/prep, Settings composition.
//!
//! Phase 0: a demo screen that exercises every widget so the kit can be judged by
//! eye — big text, buttons, faders, crossfader, knobs, meters, readouts.

use lmx_ui::layout::hstack;
use lmx_ui::{Rect, UiFrame, Vec2};

pub struct Stats {
    pub adapter: String,
    pub scale: f32,
    pub continuous: bool,
}

pub struct DemoScreen {
    xfader: f32,
    ch_fader: [f32; 4],
    trim: [f32; 4],
    hi: [f32; 4],
    low: [f32; 4],
    master: f32,
    play: [bool; 2],
    sync: [bool; 2],
    keylock: [bool; 2],
    pfl: [bool; 4],
    animate: bool,
    phase: f32,
    level: [f32; 4],
}

impl Default for DemoScreen {
    fn default() -> Self {
        Self {
            xfader: 0.5,
            ch_fader: [0.85, 0.85, 0.0, 0.0],
            trim: [0.5; 4],
            hi: [0.5; 4],
            low: [0.5; 4],
            master: 0.75,
            play: [true, false],
            sync: [true, true],
            keylock: [true, false],
            pfl: [false, true, false, false],
            animate: true,
            phase: 0.0,
            level: [-60.0; 4],
        }
    }
}

impl DemoScreen {
    pub fn draw(&mut self, f: &mut UiFrame, stats: &Stats) {
        let th = f.theme().clone();
        let gap = th.gap;
        let mut r = Rect::new(0.0, 0.0, f.size.x, f.size.y).inset(20.0);

        // fake signal so the meters move
        if self.animate {
            self.phase += f.dt;
            for (i, l) in self.level.iter_mut().enumerate() {
                let t = self.phase * (1.3 + i as f32 * 0.37);
                let gain = if i < 2 { self.ch_fader[i] } else { 0.0 };
                let sig = (t.sin() * 0.5 + 0.5) * (t * 7.0).sin().abs();
                *l = if gain > 0.0 { -40.0 + 46.0 * sig * gain } else { -60.0 };
            }
            f.animate();
        }

        // ── header ──
        let head = r.cut_top(80.0);
        f.title(Vec2::new(head.x, head.y), "LANTERN MIX");
        let live = if stats.continuous { "● LIVE" } else { "○ IDLE" };
        let info = format!("{}   {:.0}×{:.0} @ {:.2}   {}", stats.adapter, f.size.x, f.size.y, stats.scale, live);
        f.text_right(head, &info, th.text_small, th.fg_dim);
        r.cut_top(gap);

        // ── bottom rows first so the mixer takes what's left ──
        let foot = r.cut_bottom(th.button_h);
        r.cut_bottom(gap);
        let xf = r.cut_bottom(70.0);
        r.cut_bottom(gap);

        // ── decks ──
        let decks = r.cut_top(170.0);
        for (i, dr) in decks.columns(2, gap).enumerate() {
            f.push_scope(i as u64);
            self.deck(f, i, dr);
            f.pop_scope();
        }
        r.cut_top(gap);

        // ── mixer ──
        let mixer = r;
        f.panel(mixer);
        let inner = mixer.inset(th.pad);
        let strips = hstack(inner, &[0.0, 0.0, 0.0, 0.0, 10.0, 220.0], gap);
        for i in 0..4 {
            f.push_scope(i as u64);
            self.strip(f, i, strips[i]);
            f.pop_scope();
        }
        self.side(f, strips[5]);

        // ── crossfader ──
        let xr = xf.centered(xf.w.min(750.0), 50.0);
        f.crossfader(xr, &mut self.xfader);
        let xl = format!("{:>3.0}%", self.xfader * 100.0);
        f.text_right(Rect::new(xr.right() + gap, xr.y, 90.0, xr.h), &xl, th.text, th.fg_dim);
        f.text_right(Rect::new(xr.x - 100.0, xr.y, 90.0, xr.h), "A", th.text, th.deck[0]);
        f.text_left(Rect::new(xr.right() + 110.0, xr.y, 60.0, xr.h), "B", th.text, th.deck[1]);

        // ── footer ──
        let mut fr = foot;
        let tb = fr.cut_left(250.0);
        f.toggle(tb, "ANIMATE METERS", &mut self.animate);
    }

    fn deck(&mut self, f: &mut UiFrame, i: usize, rect: Rect) {
        let th = f.theme().clone();
        let color = th.deck[i];
        f.panel(rect);
        f.p.fill_rrect(Rect::new(rect.x, rect.y, 10.0, rect.h), 5.0, color);
        let inner = rect.inset(th.pad);
        let mut top = inner;
        let mut head = top.cut_top(35.0);
        let tag = format!("DECK {}", i + 1);
        f.text_bold(&tag, th.text, Vec2::new(head.x, head.y), color);
        head.cut_left(120.0);
        let name = if i == 0 { "Untitled Wub — Someone" } else { "Nothing loaded" };
        f.text_left(head, name, th.text, if i == 0 { th.fg } else { th.fg_dim });

        top.cut_top(5.0);
        let mut row = top.cut_top(60.0);
        let bpm = row.cut_left(250.0);
        f.readout(bpm, if i == 0 { "140.00" } else { "---.--" }, "BPM", th.fg);
        let key = row.cut_left(150.0);
        f.readout(key, if i == 0 { "8A" } else { "--" }, "KEY", if i == 0 { th.ok } else { th.fg_dim });
        let time = row.cut_left(250.0);
        f.readout(time, if i == 0 { "-02:14.5" } else { "--:--.-" }, "", th.fg);

        // transport buttons on the right of the readout row
        let bw = 120.0;
        let mut br = Rect::new(inner.right() - (bw * 3.0 + th.gap * 2.0), row.y, bw * 3.0 + th.gap * 2.0, th.button_h);
        let b1 = br.cut_left(bw);
        br.cut_left(th.gap);
        let b2 = br.cut_left(bw);
        br.cut_left(th.gap);
        let b3 = br.cut_left(bw);
        f.toggle_colored(b1, "PLAY", &mut self.play[i], color);
        f.toggle(b2, "SYNC", &mut self.sync[i]);
        f.toggle(b3, "KEYLOCK", &mut self.keylock[i]);
    }

    fn strip(&mut self, f: &mut UiFrame, i: usize, rect: Rect) {
        let th = f.theme().clone();
        let color = th.deck[i];
        let mut r = rect;
        let head = r.cut_top(35.0);
        f.text_centered(head, &format!("CH {}", i + 1), th.text, color);
        r.cut_top(th.gap);
        let kr = th.knob_r;
        let knob_h = kr * 2.0 + 40.0 + th.gap;
        let cx = r.center().x;
        let k1 = r.cut_top(knob_h);
        f.knob(Vec2::new(cx, k1.y + kr), kr, &mut self.trim[i], "TRIM", color);
        let k2 = r.cut_top(knob_h);
        f.knob(Vec2::new(cx, k2.y + kr), kr, &mut self.hi[i], "HI", color);
        let k3 = r.cut_top(knob_h);
        f.knob(Vec2::new(cx, k3.y + kr), kr, &mut self.low[i], "LOW", color);
        // PFL + fader + meter
        let cue = r.cut_bottom(th.button_h);
        let mut fr = r;
        fr.cut_bottom(th.gap);
        let meter_w = th.meter_w * 2.0 + 10.0;
        let fw = 60.0;
        let total = fw + th.gap + meter_w;
        let mut mid = Rect::new(fr.center().x - total * 0.5, fr.y, total, fr.h);
        let fader = mid.cut_left(fw);
        mid.cut_left(th.gap);
        let meter = mid.cut_left(meter_w);
        f.vfader(fader, &mut self.ch_fader[i]);
        let l = self.level[i];
        f.meter(meter, l, l - 1.5 + (i as f32) * 0.4);
        let cb = cue.centered(cue.w.min(140.0), th.button_h);
        f.toggle_colored(cb, "CUE", &mut self.pfl[i], th.ok);
    }

    fn side(&mut self, f: &mut UiFrame, rect: Rect) {
        let th = f.theme().clone();
        let mut r = rect;
        let head = r.cut_top(35.0);
        f.text_centered(head, "MASTER", th.text, th.fg_dim);
        r.cut_top(th.gap);
        let kr = 60.0;
        let k = r.cut_top(kr * 2.0 + 40.0 + th.gap);
        f.knob(Vec2::new(k.center().x, k.y + kr), kr, &mut self.master, "LEVEL", th.accent);
        r.cut_top(th.gap);
        let b = r.cut_bottom(th.button_h);
        r.cut_bottom(th.gap);
        let peak = self.level.iter().cloned().fold(-60.0f32, f32::max);
        let mrect = r.centered(th.meter_w * 2.0 + 10.0, r.h);
        f.meter(mrect, peak, peak - 0.7);
        if f.button(b.centered(b.w.min(180.0), th.button_h), "PANIC") {
            self.play = [false, false];
        }
    }
}
