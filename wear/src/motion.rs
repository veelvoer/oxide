//! The gyro mouse's sensors (in `WatchActivity.java`), polled from here.
#[derive(Default, Clone, Copy)]
pub struct Motion {
    /// radians since the last poll: hand left/right, hand up/down
    pub yaw: f32,
    pub pitch: f32,
    /// pinches that just ended (1 = click, 3 = right click)
    pub pinches: u32,
    /// where the arm points (yaw, pitch in radians), in the pointing mode
    pub arm: Option<(f32, f32)>,
    /// milliseconds since the last tap that might be a pinch (None when there was none lately)
    pub spike_ms: Option<f32>,
}

#[cfg(target_os = "android")]
mod imp {
    use super::Motion;
    use jni::{objects::JObject, objects::JValue, JNIEnv, JavaVM};
    use std::sync::OnceLock;

    static APP: OnceLock<android_activity::AndroidApp> = OnceLock::new();

    pub fn init(app: &android_activity::AndroidApp) {
        let _ = APP.set(app.clone());
    }

    pub fn call<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
        let app = APP.get()?;
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.ok()?;
        let mut env = vm.attach_current_thread_permanently().ok()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let r = f(&mut env, &activity);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return None;
        }
        r.ok()
    }

    pub fn enable(on: bool, pinch: bool, level: f32, absolute: bool, arm_sign: f32) {
        call(|env, a| env.call_method(a, "motion", "(ZZFZF)V", &[JValue::Bool(on as u8), JValue::Bool(pinch as u8), JValue::Float(level), JValue::Bool(absolute as u8), JValue::Float(arm_sign)]).map(|_| ()));
    }

    /// Keep the sensors and the link alive while the screen is off (a foreground service), or let go.
    pub fn keep_alive(on: bool) {
        call(|env, a| env.call_method(a, "keepAlive", "(Z)V", &[JValue::Bool(on as u8)]).map(|_| ()));
    }

    /// The sensors are on (false once the app is closed).
    pub fn active() -> bool {
        call(|env, a| env.call_method(a, "motionActive", "()Z", &[])?.z()).unwrap_or(false)
    }

    pub fn haptic(kind: i32) {
        crate::platform::haptic(kind);
    }

    pub fn has_gyro() -> bool {
        call(|env, a| env.call_method(a, "hasGyro", "()Z", &[])?.z()).unwrap_or(false)
    }

    pub fn poll() -> Motion {
        call(|env, a| {
            let o = env.call_method(a, "motionPoll", "()[F", &[])?.l()?;
            let arr = jni::objects::JFloatArray::from(o);
            let mut v = [0f32; 7];
            env.get_float_array_region(&arr, 0, &mut v)?;
            Ok(Motion { yaw: v[0], pitch: v[1], pinches: v[2] as u32, arm: (v[5] > 0.5).then_some((v[3], v[4])), spike_ms: (v[6] >= 0.0).then_some(v[6]) })
        })
        .unwrap_or_default()
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    use super::Motion;
    pub fn enable(_: bool, _: bool, _: f32, _: bool, _: f32) {}
    pub fn has_gyro() -> bool {
        true
    }
    pub fn keep_alive(_: bool) {}
    pub fn active() -> bool {
        true
    }
    pub fn haptic(_: i32) {}
    pub fn poll() -> Motion {
        Motion::default()
    }
}

pub use imp::*;

#[cfg(not(target_os = "android"))]
pub fn init(_: &()) {}
