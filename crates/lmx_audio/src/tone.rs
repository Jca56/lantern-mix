//! Test tone render + level publishing. What the RT thread runs until the engine
//! exists; later the output-test source in Settings.

use crate::host::AudioRender;
use lmx_rt::{triple, AtomicF32, TripleReader, TripleWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Per-block peak levels, dBFS, one per output channel (up to 8).
#[derive(Clone, Copy, Debug)]
pub struct Levels {
    pub peak_db: [f32; 8],
    pub channels: u8,
}

impl Default for Levels {
    fn default() -> Self {
        Self { peak_db: [-120.0; 8], channels: 0 }
    }
}

/// UI-side handles to steer the tone. Cheap to clone.
#[derive(Clone)]
pub struct ToneControl {
    pub on: Arc<AtomicBool>,
    /// 0..1 linear gain.
    pub gain: Arc<AtomicF32>,
    pub freq: Arc<AtomicF32>,
}

impl ToneControl {
    pub fn set_on(&self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
    }
    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }
}

pub struct TestTone {
    ctl: ToneControl,
    levels: TripleWriter<Levels>,
    phase: f32,
    /// Smoothed gain so on/off and knob moves never click.
    gain_s: f32,
}

impl TestTone {
    /// Returns the render object (goes to the audio host), a control handle and
    /// the level reader (both stay with the UI).
    pub fn new(freq: f32, gain: f32) -> (TestTone, ToneControl, TripleReader<Levels>) {
        let ctl = ToneControl {
            on: Arc::new(AtomicBool::new(false)),
            gain: Arc::new(AtomicF32::new(gain)),
            freq: Arc::new(AtomicF32::new(freq)),
        };
        let (w, r) = triple(Levels::default());
        (TestTone { ctl: ctl.clone(), levels: w, phase: 0.0, gain_s: 0.0 }, ctl, r)
    }
}

impl AudioRender for TestTone {
    fn render(&mut self, out: &mut [f32], channels: usize, frames: usize, rate: u32) {
        let target = if self.ctl.on.load(Ordering::Relaxed) { self.ctl.gain.load().clamp(0.0, 1.0) } else { 0.0 };
        let freq = self.ctl.freq.load().max(1.0);
        let step = std::f32::consts::TAU * freq / rate.max(1) as f32;
        // ~5 ms one-pole on the gain.
        let k = 1.0 - (-1.0 / (0.005 * rate as f32)).exp();
        let mut peak = [0.0f32; 8];
        for f in 0..frames {
            self.gain_s += (target - self.gain_s) * k;
            let s = self.phase.sin() * self.gain_s * 0.5; // −6 dBFS at full gain
            self.phase += step;
            if self.phase >= std::f32::consts::TAU {
                self.phase -= std::f32::consts::TAU;
            }
            let base = f * channels;
            // tone on the first stereo pair only; cue/other pairs stay silent
            for c in 0..channels.min(2) {
                out[base + c] = s;
                let a = s.abs();
                if a > peak[c] {
                    peak[c] = a;
                }
            }
        }
        let mut lv = Levels { peak_db: [-120.0; 8], channels: channels.min(8) as u8 };
        for c in 0..channels.min(8) {
            lv.peak_db[c] = if peak[c] > 1e-6 { 20.0 * peak[c].log10() } else { -120.0 };
        }
        self.levels.write(lv);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_when_off_and_reports_levels() {
        let (mut t, ctl, mut r) = TestTone::new(1000.0, 1.0);
        let mut buf = vec![0.0f32; 256 * 2];
        t.render(&mut buf, 2, 256, 48_000);
        assert!(buf.iter().all(|s| *s == 0.0));
        assert!(r.read().peak_db[0] <= -100.0);
        ctl.set_on(true);
        for _ in 0..40 {
            t.render(&mut buf, 2, 256, 48_000);
        }
        let lv = r.read();
        assert!(lv.peak_db[0] > -7.0 && lv.peak_db[0] < -5.0, "peak {}", lv.peak_db[0]);
        assert_eq!(lv.channels, 2);
    }
}
