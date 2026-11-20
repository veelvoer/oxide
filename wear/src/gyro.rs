//! The gyro mouse, running on its own thread: it keeps working while the screen is off (the app is open, a foreground
//! service keeps the process and the sensors alive) and stops when the PC is left or the app is closed.
//!
//! *Point* mode: hold your hand like a pointing finger and thumb, point at the screen and move the hand: the pointer
//! follows the way it points. *Move* mode: the wrist's turn is mouse movement. A pinch clicks (one pinch = left click,
//! three = right click). Lowering the arm pauses the pointer, raising it carries on where it was.
use std::{
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
            let mut applied = cfg0;
            // where the pointer is (fractions of the screen) and where the arm pointed at the last sample
            let mut pos = (0.5f32, 0.5f32);
            let mut last: Option<(f32, f32)> = None;
            let mut sent = (-1.0f32, -1.0f32);
            let mut idle_since: Option<Instant> = None;
            while running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(15));
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
                if recenter.swap(false, Ordering::SeqCst) {
                    pos = (0.5, 0.5);
                    last = None;
                    sent = (-1.0, -1.0);
                }
                // a pinch is in the making (its first tap was just felt): the hand holds still
                let pinching = m.spike_ms.is_some_and(|ms| ms < 380.0);
                if m.pinches > 0 {
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
                            let _ = tx.send(WatchReq::Send { msg: ToPc::Click { button: b.into() } });
                            motion::haptic(1);
                        }
                    }
                    wake();
                }
                if !c.send {
                    continue;
                }
                if c.point {
                    let Some((yaw, pitch)) = m.arm else { continue };
                    // arm lowered: paused; raised again: carries on from where the pointer was
                    let up = pitch > deg(-55.0);
                    shared.arm_up.store(up, Ordering::Relaxed);
                    if !up || pinching {
                        last = None;
                        continue;
                    }
                    if let Some(prev) = last {
                        pos = point_step(pos, prev, (yaw, pitch), &c);
                    }
                    last = Some((yaw, pitch));
                    if (pos.0 - sent.0).abs() > 0.0008 || (pos.1 - sent.1).abs() > 0.0008 {
                        sent = pos;
                        let _ = tx.send(WatchReq::Send { msg: ToPc::PointerTo { x: pos.0 as f64, y: pos.1 as f64 } });
                    }
                } else {
                    if pinching {
                        continue;
                    }
                    const K: f32 = 2600.0;
                    let (mut dx, mut dy) = (-m.yaw * K * c.speed, -m.pitch * K * c.speed);
                    if c.invert_x {
                        dx = -dx;
                    }
                    if c.invert_y {
                        dy = -dy;
                    }
                    if dx.abs() > 0.2 || dy.abs() > 0.2 {
                        let _ = tx.send(WatchReq::Send { msg: ToPc::PointerBy { dx: dx as f64, dy: dy as f64 } });
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

/// The pointer's new place when the arm turned from `prev` to `now` (yaw, pitch in radians): fractions of the screen.
/// About 22 degrees of the hand's travel cross the whole screen at speed 1.
pub fn point_step(pos: (f32, f32), prev: (f32, f32), now: (f32, f32), c: &GyroCfg) -> (f32, f32) {
    let mut dyaw = now.0 - prev.0;
    while dyaw > PI {
        dyaw -= 2.0 * PI;
    }
    while dyaw < -PI {
        dyaw += 2.0 * PI;
    }
    let range = deg(22.0) / c.speed;
    let (mut dx, mut dy) = (dyaw / (2.0 * range), -(now.1 - prev.1) / (2.0 * range * 0.62));
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
        GyroCfg { point: true, speed: 1.0, invert_x: false, invert_y: false, pinch: true, pinch_level: 1.6, right_wrist: false, send: true }
    }

    #[test]
    fn turning_right_moves_the_pointer_right() {
        let (x, y) = point_step((0.5, 0.5), (0.0, 0.0), (deg(11.0), 0.0), &cfg());
        assert!((x - 0.75).abs() < 1e-3 && (y - 0.5).abs() < 1e-3);
    }

    #[test]
    fn the_wrap_around_north_is_not_a_jump() {
        let (x, _) = point_step((0.5, 0.5), (PI - 0.01, 0.0), (-PI + 0.01, 0.0), &cfg());
        assert!(x > 0.5 && x < 0.53);
    }

    #[test]
    fn raising_the_hand_moves_the_pointer_up_and_it_stops_at_the_edge() {
        let (_, y) = point_step((0.5, 0.5), (0.0, 0.0), (0.0, deg(5.0)), &cfg());
        assert!(y < 0.5);
        let (x, _) = point_step((0.95, 0.5), (0.0, 0.0), (deg(40.0), 0.0), &cfg());
        assert_eq!(x, 1.0);
    }
}
