//! Hermite 4-point interpolation now; windowed-sinc later.

/// 4-point, 3rd-order Hermite interpolation between `x1` and `x2` at fraction
/// `t` (0..1), using neighbours `x0` and `x3`. The DJ standard for vinyl-style
/// pitch: cheap, smooth, no ringing.
#[inline]
pub fn hermite(x0: f32, x1: f32, x2: f32, x3: f32, t: f32) -> f32 {
    let c0 = x1;
    let c1 = 0.5 * (x2 - x0);
    let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
    let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
    ((c3 * t + c2) * t + c1) * t + c0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_the_knots_and_is_smooth_between() {
        assert_eq!(hermite(0.0, 1.0, 2.0, 3.0, 0.0), 1.0);
        assert!((hermite(0.0, 1.0, 2.0, 3.0, 1.0) - 2.0).abs() < 1e-6);
        assert!((hermite(0.0, 1.0, 2.0, 3.0, 0.5) - 1.5).abs() < 1e-6);
        // a sine sampled sparsely interpolates to within a few percent
        let f = |i: f32| (i * 0.4).sin();
        let got = hermite(f(0.0), f(1.0), f(2.0), f(3.0), 0.3);
        assert!((got - f(1.3)).abs() < 0.01);
    }
}
