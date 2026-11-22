//! What the watch's own controls say: the touch bezel (rotary) and the back button. Android hands them to the activity
//! (`WatchActivity.java`), not to the drawing loop, so they are collected here and the screen is woken to read them.
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};

/// the bezel's turns since the last look, as the bits of an f32 (positive = clockwise, content moves up)
static ROTARY: AtomicU32 = AtomicU32::new(0);
static BACK: AtomicBool = AtomicBool::new(false);
static REOPENED: AtomicBool = AtomicBool::new(false);
type Repaint = Arc<dyn Fn() + Send + Sync>;
static REPAINT: Mutex<Option<Repaint>> = Mutex::new(None);

/// How the screen is woken when something arrives (the app gives its `request_repaint`).
pub fn set_repaint(f: impl Fn() + Send + Sync + 'static) {
    *REPAINT.lock().unwrap() = Some(Arc::new(f));
}

fn wake() {
    if let Some(f) = REPAINT.lock().unwrap().as_ref() {
        f();
    }
}

pub fn push_rotary(v: f32) {
    let mut cur = ROTARY.load(Ordering::Relaxed);
    loop {
        let next = (f32::from_bits(cur) + v).to_bits();
        match ROTARY.compare_exchange_weak(cur, next, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(_) => break,
            Err(c) => cur = c,
        }
    }
    wake();
}

pub fn push_back() {
    BACK.store(true, Ordering::SeqCst);
    wake();
}

pub fn push_reopened() {
    REOPENED.store(true, Ordering::SeqCst);
    wake();
}

/// The app was opened again from the launcher (it was running already) since the last call.
pub fn take_reopened() -> bool {
    REOPENED.swap(false, Ordering::SeqCst)
}

/// The bezel's movement since the last call.
pub fn take_rotary() -> f32 {
    f32::from_bits(ROTARY.swap(0, Ordering::SeqCst))
}

/// The back button was pressed since the last call.
pub fn take_back() -> bool {
    BACK.swap(false, Ordering::SeqCst)
}

/// `WatchActivity.nativeInput(kind, value)`: 0 = the bezel turned by `value`, 1 = back.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_oxide_remote_WatchActivity_nativeInput(_env: jni::JNIEnv, _this: jni::objects::JObject, kind: jni::sys::jint, value: jni::sys::jfloat) {
    match kind {
        0 => push_rotary(value),
        1 => push_back(),
        2 => push_reopened(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bezel_adds_up_and_is_read_once() {
        take_rotary();
        push_rotary(3.0);
        push_rotary(-1.0);
        assert_eq!(take_rotary(), 2.0);
        assert_eq!(take_rotary(), 0.0);
        push_back();
        assert!(take_back());
        assert!(!take_back());
    }
}
