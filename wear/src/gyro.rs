//! The gyro mouse, running on its own thread: it keeps working while the screen is off (the app is open, a foreground
//! service keeps the process and the sensors alive) and stops when the PC is left or the app is closed.
//!
//! *Point* mode: hold your hand like a pointing finger and thumb, point at the screen and move the hand: the pointer
//! follows the way it points. *Move* mode: the wrist's turn is mouse movement. A pinch clicks.
//!
//! Smooth: the pointer goes through a One Euro filter (steady when the hand is slow, quick when it moves) and the phone
//! chases the target at 160 steps a second. Accurate: closing the fingers moves the hand a little just before the tap is
//! felt, so on a pinch the pointer goes back to where it was about 90 ms earlier, and only then the click is made; and
//! the pointer holds still while the pinch is in the making.
use std::{
    collections::VecDeque,
    f32::consts::PI,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::Sender,
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use oxide_remote_proto::{watch::WatchReq, ToPc};

use crate::motion;

#[derive(Clone, Copy, PartialEq)]
pub struct GyroCfg {
    pub point: bool,
    pub speed: f32,
    pub invert_x: bool,
    pub invert_y: bool,
    pub pinch: bool,
    pub pinch_count: bool,
    pub pinch_level: f32,
    pub right_wrist: bool,
    /// send to the PC (off while only the settings' pinch test runs)
    pub send: bool,
}

/// What the screen shows of it.
#[derive(Default)]
pub struct Shared {
    /// the last pinch (count, when)
    pub last_pinch: Mutex<Option<(u32, Instant)>>,
    /// pinches seen since the sensors started (for the test)
    pub pinches: AtomicU32,
    /// the arm is up (the pointer follows)
    pub arm_up: AtomicBool,
    /// the strongest tap since the last look and what a tap has to reach, in thousandths of m/s2 (the sensitivity meter)
    pub jerk_peak: AtomicU32,
    pub jerk_threshold: AtomicU32,
}

pub struct Gyro {
    pub cfg: Arc<Mutex<GyroCfg>>,
    pub shared: Arc<Shared>,
    recenter: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

fn deg(d: f32) -> f32 {
    d * PI / 180.0
}

/// The One Euro filter: little lag when moving fast, little jitter when slow.
pub struct OneEuro {
    fc_min: f32,
    beta: f32,
    prev: Option<(f32, f32, Instant)>,
}

impl OneEuro {
    pub fn new(fc_min: f32, beta: f32) -> Self {
        Self { fc_min, beta, prev: None }
    }

    fn alpha(dt: f32, fc: f32) -> f32 {
        let tau = 1.0 / (2.0 * PI * fc);
        1.0 / (1.0 + tau / dt)
    }

    pub fn clear(&mut self) {
        self.prev = None;
    }

    pub fn reset(&mut self, x: f32, now: Instant) {
        self.prev = Some((x, 0.0, now));
    }

    pub fn filter(&mut self, x: f32, now: Instant) -> f32 {
        let Some((px, pdx, pt)) = self.prev else {
            self.prev = Some((x, 0.0, now));
            return x;
        };
        let dt = now.saturating_duration_since(pt).as_secs_f32().max(0.001);
        let dx = (x - px) / dt;
        let edx = pdx + (dx - pdx) * Self::alpha(dt, 1.0);
        let fc = self.fc_min + self.beta * edx.abs();
        let out = px + (x - px) * Self::alpha(dt, fc);
        self.prev = Some((out, edx, now));
        out
    }
}

impl Gyro {
    pub fn new(cfg: GyroCfg) -> Self {
        Self { cfg: Arc::new(Mutex::new(cfg)), shared: Arc::new(Shared::default()), recenter: Arc::new(AtomicBool::new(false)), running: Arc::new(AtomicBool::new(false)) }
    }

    pub fn set(&self, cfg: GyroCfg) {
        if let Ok(mut c) = self.cfg.lock() {
            *c = cfg;
        }
    }

    pub fn recenter(&self) {
        self.recenter.store(true, Ordering::SeqCst);
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Start the sensors and the thread (no-op when it already runs).
    pub fn start(&self, tx: Sender<WatchReq>, wake: impl Fn() + Send + 'static) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let cfg0 = self.cfg.lock().map(|c| *c).unwrap_or_else(|_| unreachable!());
        motion::enable(true, cfg0.pinch, cfg0.pinch_level, cfg0.point, if cfg0.right_wrist { -1.0 } else { 1.0 });
        motion::keep_alive(true);
        self.shared.pinches.store(0, Ordering::SeqCst);
        let (cfg, shared, recenter, running) = (self.cfg.clone(), self.shared.clone(), self.recenter.clone(), self.running.clone());
        std::thread::spawn(move || {
            let send = |m: ToPc| {
                let _ = tx.send(WatchReq::Send { msg: m });
            };
            let mut applied = cfg0;
            // where the pointer is (fractions of the screen) and where the arm pointed at the last sample
            let mut pos = (0.5f32, 0.5f32);
            let mut last: Option<(f32, f32)> = None;
            let mut sent = (-1.0f32, -1.0f32);
            // the pointer's own filter (after a pinch took it back), and the filters of the arm's angles
            let (mut fx, mut fy) = (OneEuro::new(1.4, 14.0), OneEuro::new(1.4, 14.0));
            let (mut fyaw, mut fpitch) = (OneEuro::new(0.9, 4.0), OneEuro::new(0.9, 4.0));
            let mut yaw_u = 0.0f32;
            let mut last_raw_yaw: Option<f32> = None;
            let mut last_f: Option<(f32, f32, Instant)> = None;
            // mouse mode: the smoothed movement
            let mut smooth_mv = (0.0f32, 0.0f32);
            // the last 400 ms of where the pointer was (pointing) and of the moves made (mouse mode), to take a pinch back
            let mut hist: VecDeque<(Instant, (f32, f32))> = VecDeque::new();
            let mut moves: VecDeque<(Instant, (f32, f32))> = VecDeque::new();
            let mut idle_since: Option<Instant> = None;
            let mut pinch_times: Vec<Instant> = vec![];
            while running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(8));
                let c = cfg.lock().map(|c| *c).unwrap_or(applied);
                if c != applied {
                    if c.point != applied.point || c.pinch != applied.pinch || c.pinch_level != applied.pinch_level || c.right_wrist != applied.right_wrist {
                        motion::enable(true, c.pinch, c.pinch_level, c.point, if c.right_wrist { -1.0 } else { 1.0 });
                        last = None;
                    }
                    applied = c;
                }
                // the app was closed: the sensors are off, so is this
                if !motion::active() {
                    let t = idle_since.get_or_insert_with(Instant::now);
                    if t.elapsed() > Duration::from_secs(2) {
                        break;
                    }
                    continue;
                }
                idle_since = None;
                let m = motion::poll();
                let now = Instant::now();
                shared.jerk_peak.fetch_max((m.jerk_peak * 1000.0) as u32, Ordering::Relaxed);
                shared.jerk_threshold.store((m.jerk_threshold * 1000.0) as u32, Ordering::Relaxed);
                if recenter.swap(false, Ordering::SeqCst) {
                    pos = (0.5, 0.5);
                    fx.reset(0.5, now);
                    fy.reset(0.5, now);
                    last = None;
                    sent = (-1.0, -1.0);
                }
                // a pinch is in the making (its first tap was just felt): the hand holds still
                let hold = m.spike_ms.is_some_and(|ms| ms < if c.pinch_count { 330.0 } else { 220.0 });
                // ---- pinches
                if m.new_spikes > 0 && c.pinch && c.send && !c.pinch_count {
                    // take the pointer back to where it was before the fingers closed, then click at once
                    let ago = Duration::from_millis(m.spike_ms.unwrap_or(0.0) as u64 + 90);
                    let at = now.checked_sub(ago).unwrap_or(now);
                    if c.point {
                        if let Some(&(_, p)) = hist.iter().rev().find(|(t, _)| *t <= at) {
                            pos = p;
                            fx.reset(p.0, now);
                            fy.reset(p.1, now);
                            sent = p;
                            send(ToPc::PointerTo { x: p.0 as f64, y: p.1 as f64 });
                        }
                    } else {
                        let (sx, sy) = moves.iter().filter(|(t, _)| *t > at).fold((0.0, 0.0), |a, (_, d)| (a.0 + d.0, a.1 + d.1));
                        if sx != 0.0 || sy != 0.0 {
                            send(ToPc::PointerBy { dx: -sx as f64, dy: -sy as f64 });
                        }
                    }
                    for _ in 0..m.new_spikes {
                        send(ToPc::Click { button: "left".into() });
                    }
                    motion::haptic(0);
                    shared.pinches.fetch_add(m.new_spikes, Ordering::SeqCst);
                    if let Ok(mut l) = shared.last_pinch.lock() {
                        *l = Some((m.new_spikes, Instant::now()));
                    }
                    wake();
                } else if m.new_spikes > 0 && !c.pinch_count {
                    shared.pinches.fetch_add(m.new_spikes, Ordering::SeqCst);
                    if let Ok(mut l) = shared.last_pinch.lock() {
                        *l = Some((m.new_spikes, Instant::now()));
                    }
                    wake();
                }
                if m.pinches > 0 && c.pinch_count {
                    // counting: 1 = left click, 3 = right click, after the window has closed
                    pinch_times.clear();
                    shared.pinches.fetch_add(m.pinches, Ordering::SeqCst);
                    if let Ok(mut l) = shared.last_pinch.lock() {
                        *l = Some((m.pinches, Instant::now()));
                    }
                    if c.pinch && c.send {
                        let button = match m.pinches {
                            1 => Some("left"),
                            3 => Some("right"),
                            _ => None,
                        };
                        if let Some(b) = button {
                            send(ToPc::Click { button: b.into() });
                            motion::haptic(1);
                        }
                    }
                    wake();
                }
                if !c.send {
                    continue;
                }
                // ---- the pointer
                if c.point {
                    let Some((yaw, pitch)) = m.arm else { continue };
                    // arm lowered: paused; raised again: carries on from where the pointer was
                    let up = pitch > deg(-55.0);
                    shared.arm_up.store(up, Ordering::Relaxed);
                    if !up || hold {
                        last = None;
                        last_f = None;
                        last_raw_yaw = None;
                        fyaw.clear();
                        fpitch.clear();
                        continue;
                    }
                    // the yaw runs on without the jump at north; both angles are filtered (steady when slow, quick when fast)
                    yaw_u = match last_raw_yaw {
                        Some(l) => {
                            let mut d = yaw - l;
                            while d > PI {
                                d -= 2.0 * PI;
                            }
                            while d < -PI {
                                d += 2.0 * PI;
                            }
                            yaw_u + d
                        }
                        None => yaw,
                    };
                    last_raw_yaw = Some(yaw);
                    let (ay, ap) = (fyaw.filter(yaw_u, now), fpitch.filter(pitch, now));
                    if let Some((py, pp, pt)) = last_f {
                        let dt = now.saturating_duration_since(pt).as_secs_f32().max(0.002);
                        pos = point_step(pos, (py, pp), (ay, ap), dt, &c);
                    }
                    last_f = Some((ay, ap, now));
                    last = Some((ay, ap));
                    let out = pos;
                    hist.push_back((now, out));
                    while hist.front().is_some_and(|(t, _)| now.duration_since(*t) > Duration::from_millis(400)) {
                        hist.pop_front();
                    }
                    if (out.0 - sent.0).abs() > 0.0006 || (out.1 - sent.1).abs() > 0.0006 {
                        sent = out;
                        send(ToPc::PointerTo { x: out.0 as f64, y: out.1 as f64 });
                    }
                } else {
                    if hold {
                        continue;
                    }
                    const K: f32 = 1700.0;
                    let (mut dx, mut dy) = (-m.yaw * K * c.speed, -m.pitch * K * c.speed);
                    if c.invert_x {
                        dx = -dx;
                    }
                    if c.invert_y {
                        dy = -dy;
                    }
                    // a little smoothing, and a gain that grows with the speed: slow = precise, fast = far
                    smooth_mv = (smooth_mv.0 + (dx - smooth_mv.0) * 0.45, smooth_mv.1 + (dy - smooth_mv.1) * 0.45);
                    let speed = (smooth_mv.0.hypot(smooth_mv.1)) * 120.0;
                    let g = 0.6 + 0.9 * crate::round::smooth(speed / 900.0);
                    let (dx, dy) = (smooth_mv.0 * g, smooth_mv.1 * g);
                    if dx.abs() > 0.08 || dy.abs() > 0.08 {
                        moves.push_back((now, (dx, dy)));
                        while moves.front().is_some_and(|(t, _)| now.duration_since(*t) > Duration::from_millis(400)) {
                            moves.pop_front();
                        }
                        send(ToPc::PointerBy { dx: dx as f64, dy: dy as f64 });
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
        });
    }

    /// Switch the sensors off and end the thread.
    pub fn stop(&self) {
        if self.running.swap(false, Ordering::SeqCst) {
            motion::enable(false, false, 1.0, false, 1.0);
            motion::keep_alive(false);
        }
    }
}

/// The pointer's new place when the arm turned from `prev` to `now` (yaw, pitch in radians, within `dt` seconds): fractions of
/// the screen. About 34 degrees of the hand's travel cross the whole screen at speed 1, but slowly moved it travels less
/// (precise) and quickly moved it travels more (far), and the tiniest tremor does not move it at all.
pub fn point_step(pos: (f32, f32), prev: (f32, f32), now: (f32, f32), dt: f32, c: &GyroCfg) -> (f32, f32) {
    let mut dyaw = now.0 - prev.0;
    while dyaw > PI {
        dyaw -= 2.0 * PI;
    }
    while dyaw < -PI {
        dyaw += 2.0 * PI;
    }
    let dpitch = now.1 - prev.1;
    if dyaw.abs() < 0.0003 && dpitch.abs() < 0.0003 {
        return pos;
    }
    let speed = dyaw.hypot(dpitch) / dt.max(0.001);
    let gain = 0.6 + 0.8 * crate::round::smooth(speed / 1.5);
    let range = deg(34.0) / c.speed;
    let (mut dx, mut dy) = (dyaw / (2.0 * range) * gain, -dpitch / (2.0 * range * 0.62) * gain);
    if c.invert_x {
        dx = -dx;
    }
    if c.invert_y {
        dy = -dy;
    }
    ((pos.0 + dx).clamp(0.0, 1.0), (pos.1 + dy).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GyroCfg {
        GyroCfg { point: true, speed: 1.0, invert_x: false, invert_y: false, pinch: true, pinch_count: false, pinch_level: 7.0, right_wrist: false, send: true }
    }

    #[test]
    fn turning_right_moves_the_pointer_right() {
        let (x, y) = point_step((0.5, 0.5), (0.0, 0.0), (deg(5.0), 0.0), 0.5, &cfg());
        assert!(x > 0.5 && (y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn slow_is_precise_and_fast_goes_far() {
        // the same turn, slowly and quickly
        let slow = point_step((0.5, 0.5), (0.0, 0.0), (deg(3.0), 0.0), 2.0, &cfg()).0 - 0.5;
        let fast = point_step((0.5, 0.5), (0.0, 0.0), (deg(3.0), 0.0), 0.02, &cfg()).0 - 0.5;
        assert!(fast > slow * 1.8, "slow {slow} fast {fast}");
    }

    #[test]
    fn a_tremor_does_not_move_it() {
        assert_eq!(point_step((0.5, 0.5), (0.0, 0.0), (0.0002, -0.0002), 0.01, &cfg()), (0.5, 0.5));
    }

    #[test]
    fn the_wrap_around_north_is_not_a_jump() {
        let (x, _) = point_step((0.5, 0.5), (PI - 0.01, 0.0), (-PI + 0.01, 0.0), 0.05, &cfg());
        assert!(x > 0.5 && x < 0.53);
    }

    #[test]
    fn raising_the_hand_moves_the_pointer_up_and_it_stops_at_the_edge() {
        let (_, y) = point_step((0.5, 0.5), (0.0, 0.0), (0.0, deg(5.0)), 0.5, &cfg());
        assert!(y < 0.5);
        let (x, _) = point_step((0.95, 0.5), (0.0, 0.0), (deg(60.0), 0.0), 0.5, &cfg());
        assert_eq!(x, 1.0);
    }

    #[test]
    fn the_filter_calms_jitter_but_follows_a_real_move() {
        let t0 = Instant::now();
        let mut f = OneEuro::new(1.4, 14.0);
        f.reset(0.5, t0);
        // jitter of +-0.002 around 0.5: the output stays much closer to 0.5
        let mut worst = 0.0f32;
        for i in 1..60 {
            let x = 0.5 + if i % 2 == 0 { 0.002 } else { -0.002 };
            let y = f.filter(x, t0 + Duration::from_millis(10 * i));
            worst = worst.max((y - 0.5).abs());
        }
        assert!(worst < 0.0012, "jitter passes: {worst}");
        // a quick move of 0.4 in 100 ms is followed within 0.1 after 150 ms
        let mut y = 0.5;
        for i in 0..15u64 {
            let x = 0.5 + (0.4 * (i as f32 / 10.0)).min(0.4);
            y = f.filter(x, t0 + Duration::from_millis(600 + 10 * i));
        }
        assert!((0.9 - y).abs() < 0.1, "lags: {y}");
    }
}
