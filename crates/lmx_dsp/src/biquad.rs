//! RBJ cookbook biquads + f64 magnitude response (imported keeper from lantern_eq).
//!
//! Coefficient recipes, a transposed direct-form-II state, and an analytic
//! magnitude response so displays draw the curve the filter really has.
//! `Mode::LowCut`/`HighCut` with Q = 1/√2 are Butterworth; two in series make
//! a Linkwitz-Riley 4th-order crossover edge.

use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    LowCut,
    LowShelf,
    Bell,
    HighShelf,
    HighCut,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Coeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

/// Butterworth Q for maximally flat 2nd-order sections.
pub const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Cookbook coefficients. `gain_db` is ignored by the cut modes (Q shapes their
/// resonance instead).
pub fn coeffs(mode: Mode, f0: f32, gain_db: f32, q: f32, sr: f32) -> Coeffs {
    let f0 = f0.clamp(10.0, sr * 0.49);
    let q = q.max(0.05);
    let w = TAU * f0 / sr;
    let (sn, cs) = w.sin_cos();
    let alpha = sn / (2.0 * q);
    let a = 10f32.powf(gain_db / 40.0);

    let (b0, b1, b2, a0, a1, a2) = match mode {
        Mode::Bell => (1.0 + alpha * a, -2.0 * cs, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cs, 1.0 - alpha / a),
        Mode::LowCut => {
            let b0 = (1.0 + cs) * 0.5;
            (b0, -(1.0 + cs), b0, 1.0 + alpha, -2.0 * cs, 1.0 - alpha)
        }
        Mode::HighCut => {
            let b0 = (1.0 - cs) * 0.5;
            (b0, 1.0 - cs, b0, 1.0 + alpha, -2.0 * cs, 1.0 - alpha)
        }
        Mode::LowShelf => {
            let sq = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) - (a - 1.0) * cs + sq),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                a * ((a + 1.0) - (a - 1.0) * cs - sq),
                (a + 1.0) + (a - 1.0) * cs + sq,
                -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                (a + 1.0) + (a - 1.0) * cs - sq,
            )
        }
        Mode::HighShelf => {
            let sq = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) + (a - 1.0) * cs + sq),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                a * ((a + 1.0) + (a - 1.0) * cs - sq),
                (a + 1.0) - (a - 1.0) * cs + sq,
                2.0 * ((a - 1.0) - (a + 1.0) * cs),
                (a + 1.0) - (a - 1.0) * cs - sq,
            )
        }
    };
    Coeffs { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
}

/// Magnitude response in dB at `f` — evaluated complex in f64 because the
/// all-real cosine form cancels catastrophically in f32 at low frequencies.
pub fn mag_db(c: &Coeffs, f: f32, sr: f32) -> f32 {
    let w = std::f64::consts::TAU * (f as f64 / sr as f64).min(0.499);
    let (s1, c1) = w.sin_cos();
    let (s2, c2) = (2.0 * w).sin_cos();
    let (b0, b1, b2) = (c.b0 as f64, c.b1 as f64, c.b2 as f64);
    let (a1, a2) = (c.a1 as f64, c.a2 as f64);
    let nre = b0 + b1 * c1 + b2 * c2;
    let nim = b1 * s1 + b2 * s2;
    let dre = 1.0 + a1 * c1 + a2 * c2;
    let dim = a1 * s1 + a2 * s2;
    let num = nre * nre + nim * nim;
    let den = dre * dre + dim * dim;
    (10.0 * (num.max(1e-24) / den.max(1e-24)).log10()) as f32
}

/// Transposed direct-form II state (one channel) with its coefficients.
#[derive(Clone, Copy, Default, Debug)]
pub struct Biquad {
    pub c: Coeffs,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn new(c: Coeffs) -> Self {
        Self { c, z1: 0.0, z2: 0.0 }
    }
    pub fn lowpass(f0: f32, q: f32, sr: f32) -> Self {
        Self::new(coeffs(Mode::HighCut, f0, 0.0, q, sr))
    }
    pub fn highpass(f0: f32, q: f32, sr: f32) -> Self {
        Self::new(coeffs(Mode::LowCut, f0, 0.0, q, sr))
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.c.b0 * x + self.z1;
        self.z1 = self.c.b1 * x - self.c.a1 * y + self.z2;
        self.z2 = self.c.b2 * x - self.c.a2 * y;
        y
    }
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_after(mut f: impl FnMut(f32) -> f32, freq: f32, sr: f32) -> f32 {
        let n = (sr as usize) / 2;
        let mut acc = 0.0f64;
        for i in 0..n * 2 {
            let x = (TAU * freq * i as f32 / sr).sin();
            let y = f(x);
            if i >= n {
                acc += (y * y) as f64;
            }
        }
        ((acc / n as f64).sqrt() * std::f64::consts::SQRT_2) as f32
    }

    #[test]
    fn butterworth_lowpass_passes_and_cuts() {
        let sr = 44_100.0;
        let mut lp = Biquad::lowpass(1000.0, BUTTERWORTH_Q, sr);
        assert!((rms_after(|x| lp.process(x), 100.0, sr) - 1.0).abs() < 0.01);
        let mut lp = Biquad::lowpass(1000.0, BUTTERWORTH_Q, sr);
        let g = rms_after(|x| lp.process(x), 10_000.0, sr);
        assert!(20.0 * g.log10() < -35.0, "10 kHz through 1 kHz LP: {g}");
    }

    #[test]
    fn analytic_matches_measured() {
        let sr = 44_100.0;
        let c = coeffs(Mode::Bell, 500.0, 6.0, 1.0, sr);
        let mut b = Biquad::new(c);
        let measured = 20.0 * rms_after(|x| b.process(x), 500.0, sr).log10();
        assert!((measured - mag_db(&c, 500.0, sr)).abs() < 0.3);
        assert!((mag_db(&c, 500.0, sr) - 6.0).abs() < 0.1);
    }
}
