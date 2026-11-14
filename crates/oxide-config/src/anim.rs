//! Animation math shared by the compositor and the settings preview:
//! cubic beziers, damped springs, and per-leaf progress.
use crate::{Anim, Animations, Curve};

/// Evaluate a CSS-style cubic bezier (x1, y1, x2, y2) at time `t` in 0..=1.
pub fn ease(curve: [f32; 4], t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let [x1, y1, x2, y2] = curve.map(|v| v as f64);
    let bez = |a: f64, b: f64, s: f64| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if bez(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(y1, y2, (lo + hi) / 2.0)
}

/// Position of a damped spring released from 0 toward 1 after `t` seconds, and when it has settled.
pub fn spring(mass: f32, stiffness: f32, damping: f32, t: f64) -> (f64, f64) {
    let (m, k, c) = (mass.max(0.01) as f64, stiffness.max(1.0) as f64, damping.max(0.0) as f64);
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    let t = t.max(0.0);
    let (x, rate) = if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let e = (-zeta * w0 * t).exp();
        (1.0 - e * ((wd * t).cos() + (zeta * w0 / wd) * (wd * t).sin()), zeta * w0)
    } else if (zeta - 1.0).abs() < 1e-6 {
        let e = (-w0 * t).exp();
        (1.0 - e * (1.0 + w0 * t), w0)
    } else {
        let s = (zeta * zeta - 1.0).sqrt();
        let (r1, r2) = (-w0 * (zeta - s), -w0 * (zeta + s));
        (1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1), r1.abs().min(r2.abs()))
    };
    // settled once the decaying envelope is below 0.05 %
    (x, 7.6 / rate.max(1e-3))
}

fn curve_progress(c: &Curve, speed: f32, elapsed: f64) -> (f64, bool) {
    if c.is_spring() {
        let (x, settle) = spring(c.mass, c.stiffness, c.dampening, elapsed);
        if elapsed >= settle {
            (1.0, true)
        } else {
            (x, false)
        }
    } else {
        let dur = (speed as f64 * 0.1).max(0.01);
        let t = elapsed / dur;
        (ease([c.x1, c.y1, c.x2, c.y2], t), t >= 1.0)
    }
}

/// Progress of animation `a` after `elapsed` seconds: (value, finished).
/// Disabled animations finish immediately. Springs may overshoot 1.0.
pub fn progress(anims: &Animations, a: &Anim, elapsed: f64) -> (f64, bool) {
    if !anims.enabled || !a.enabled {
        return (1.0, true);
    }
    curve_progress(&anims.curve(&a.curve), a.speed, elapsed)
}

/// Starting scale for "popin N%" / "gnomed" styles; None when the style does not scale.
pub fn popin_scale(style: &str) -> Option<f64> {
    let s = style.trim();
    if s == "gnomed" {
        return Some(0.9);
    }
    let rest = s.strip_prefix("popin")?.trim();
    let pct = rest.trim_end_matches('%').trim().parse::<f64>().unwrap_or(80.0);
    Some((pct / 100.0).clamp(0.05, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bezier_endpoints_and_monotonic() {
        let c = [0.25, 0.1, 0.25, 1.0];
        assert!(ease(c, 0.0).abs() < 1e-6);
        assert!((ease(c, 1.0) - 1.0).abs() < 1e-6);
        assert!(ease(c, 0.3) < ease(c, 0.6));
        assert!((ease([0.0, 0.0, 1.0, 1.0], 0.37) - 0.37).abs() < 1e-3);
    }
    #[test]
    fn spring_settles_with_slight_overshoot() {
        let (x0, settle) = spring(0.7, 130.0, 16.0, 0.0);
        assert!(x0.abs() < 1e-9);
        assert!(settle > 0.3 && settle < 1.5, "settle {settle}");
        let max = (0..200).map(|i| spring(0.7, 130.0, 16.0, i as f64 * 0.01).0).fold(0.0, f64::max);
        assert!(max > 1.0 && max < 1.05, "overshoot {max}");
        assert!((spring(0.7, 130.0, 16.0, settle).0 - 1.0).abs() < 0.002);
    }
    #[test]
    fn user_config_defaults() {
        let a = Animations::default();
        let (_, done) = progress(&a, &a.windows, 0.05);
        assert!(!done);
        let (v, done) = progress(&a, &a.windows, 5.0);
        assert!(done && v == 1.0);
        // a disabled leaf finishes at once
        let mut b = a.clone();
        b.windows.enabled = false;
        assert!(progress(&b, &b.windows, 0.0).1);
        // bezier duration is speed x 100ms
        assert!(!progress(&a, &a.fade_in, 0.2).1);
        assert!(progress(&a, &a.fade_in, 0.31).1);
        assert_eq!(popin_scale("gnomed"), Some(0.9));
        assert_eq!(popin_scale("popin 93%"), Some(0.93));
        assert_eq!(popin_scale("slide"), None);
    }
}
