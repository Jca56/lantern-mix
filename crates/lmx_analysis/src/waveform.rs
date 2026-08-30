//! Fine (256-frame) and overview (4096-frame) summaries: peak, RMS, 3 bands.
//!
//! Per column, 8 bytes: `[peak_l, peak_r, rms_l, rms_r, low, mid, high, 0]`.
//! Peak/RMS are linear amplitude ×255; bands are the mono RMS of a 3-way
//! Linkwitz-Riley split (200 Hz / 2.5 kHz), mapped −60…0 dB → 0…255. The GPU
//! draws height from peak, a brighter core from RMS, and color from the bands.

use lmx_codec::TrackAudio;
use lmx_dsp::{biquad::BUTTERWORTH_Q, Biquad};

pub const FINE_FRAMES: usize = 256;
pub const OVERVIEW_FRAMES: usize = 4096;
pub const LOW_HZ: f32 = 200.0;
pub const HIGH_HZ: f32 = 2500.0;

#[derive(Clone, Debug, Default)]
pub struct WaveformSummary {
    pub sample_rate: u32,
    pub fine: Vec<[u8; 8]>,
    pub overview: Vec<[u8; 8]>,
}

impl WaveformSummary {
    pub fn fine_columns(&self) -> usize {
        self.fine.len()
    }
    /// Fine column index of a source frame position.
    pub fn column_of(&self, frame: f64) -> f64 {
        frame / FINE_FRAMES as f64
    }
}

/// LR4 three-way split: two cascaded Butterworth sections per edge.
struct Bands {
    lp: [Biquad; 2],
    hp: [Biquad; 2],
    mid_lp: [Biquad; 2],
    mid_hp: [Biquad; 2],
}

impl Bands {
    fn new(sr: f32) -> Self {
        let lp = Biquad::lowpass(LOW_HZ, BUTTERWORTH_Q, sr);
        let hp = Biquad::highpass(HIGH_HZ, BUTTERWORTH_Q, sr);
        let mid_lp = Biquad::lowpass(HIGH_HZ, BUTTERWORTH_Q, sr);
        let mid_hp = Biquad::highpass(LOW_HZ, BUTTERWORTH_Q, sr);
        Self { lp: [lp; 2], hp: [hp; 2], mid_lp: [mid_lp; 2], mid_hp: [mid_hp; 2] }
    }
    #[inline]
    fn split(&mut self, x: f32) -> (f32, f32, f32) {
        let l0 = self.lp[0].process(x);
        let low = self.lp[1].process(l0);
        let h0 = self.hp[0].process(x);
        let high = self.hp[1].process(h0);
        let m0 = self.mid_lp[0].process(x);
        let m1 = self.mid_lp[1].process(m0);
        let m2 = self.mid_hp[0].process(m1);
        let mid = self.mid_hp[1].process(m2);
        (low, mid, high)
    }
}

#[derive(Default, Clone, Copy)]
struct Acc {
    peak_l: f32,
    peak_r: f32,
    sq_l: f64,
    sq_r: f64,
    low: f64,
    mid: f64,
    high: f64,
    n: u32,
}

impl Acc {
    fn finish(&self) -> [u8; 8] {
        let n = self.n.max(1) as f64;
        let lin = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let rms = |sq: f64| lin((sq / n).sqrt() as f32);
        let db = |sq: f64| {
            let r = (sq / n).sqrt().max(1e-9);
            let d = 20.0 * r.log10();
            (((d + 60.0) / 60.0).clamp(0.0, 1.0) * 255.0).round() as u8
        };
        [lin(self.peak_l), lin(self.peak_r), rms(self.sq_l), rms(self.sq_r), db(self.low), db(self.mid), db(self.high), 0]
    }
}

/// Compute both resolutions in one pass. `progress` gets 0..1.
pub fn compute(audio: &TrackAudio, mut progress: impl FnMut(f32)) -> WaveformSummary {
    let sr = audio.sample_rate.max(1) as f32;
    let frames = audio.frame_count();
    let mut bands = Bands::new(sr);
    let mut fine = Vec::with_capacity(frames / FINE_FRAMES + 1);
    let mut overview = Vec::with_capacity(frames / OVERVIEW_FRAMES + 1);
    let mut fa = Acc::default();
    let mut oa = Acc::default();
    let report_every = (frames / 50).max(1);
    for (i, s) in audio.frames.chunks_exact(2).enumerate() {
        let (l, r) = (s[0], s[1]);
        let (lo, mi, hi) = bands.split(0.5 * (l + r));
        for a in [&mut fa, &mut oa] {
            a.peak_l = a.peak_l.max(l.abs());
            a.peak_r = a.peak_r.max(r.abs());
            a.sq_l += (l * l) as f64;
            a.sq_r += (r * r) as f64;
            a.low += (lo * lo) as f64;
            a.mid += (mi * mi) as f64;
            a.high += (hi * hi) as f64;
            a.n += 1;
        }
        if fa.n as usize == FINE_FRAMES {
            fine.push(fa.finish());
            fa = Acc::default();
        }
        if oa.n as usize == OVERVIEW_FRAMES {
            overview.push(oa.finish());
            oa = Acc::default();
        }
        if i % report_every == 0 {
            progress(i as f32 / frames.max(1) as f32);
        }
    }
    if fa.n > 0 {
        fine.push(fa.finish());
    }
    if oa.n > 0 {
        overview.push(oa.finish());
    }
    progress(1.0);
    WaveformSummary { sample_rate: audio.sample_rate, fine, overview }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, secs: f32, sr: u32) -> TrackAudio {
        let n = (secs * sr as f32) as usize;
        let mut frames = Vec::with_capacity(n * 2);
        for i in 0..n {
            let s = 0.5 * (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin();
            frames.push(s);
            frames.push(s);
        }
        TrackAudio { sample_rate: sr, channels: 2, frames }
    }

    #[test]
    fn column_counts_and_peak() {
        let a = tone(440.0, 1.0, 44_100);
        let s = compute(&a, |_| {});
        assert_eq!(s.fine.len(), 44_100 / 256 + 1);
        assert_eq!(s.overview.len(), 44_100 / 4096 + 1);
        let c = s.fine[10];
        assert!((c[0] as i32 - 128).abs() <= 2, "peak {}", c[0]);
        assert!((c[2] as f32 - 0.5 * 0.7071 * 255.0).abs() <= 3.0, "rms {}", c[2]);
    }

    #[test]
    fn bands_follow_frequency() {
        let sr = 44_100;
        let low = compute(&tone(60.0, 1.0, sr), |_| {}).fine[100];
        let mid = compute(&tone(800.0, 1.0, sr), |_| {}).fine[100];
        let high = compute(&tone(8000.0, 1.0, sr), |_| {}).fine[100];
        assert!(low[4] > low[5] + 60 && low[4] > low[6] + 60, "low {:?}", low);
        assert!(mid[5] > mid[4] + 60 && mid[5] > mid[6] + 60, "mid {:?}", mid);
        assert!(high[6] > high[4] + 60 && high[6] > high[5] + 60, "high {:?}", high);
    }
}
