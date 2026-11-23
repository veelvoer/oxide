//! What the phone itself provides: system bar insets, the system colors (Material You), dark mode, full screen,
//! haptics, small HTTP requests and installing updates. On Android through the Java part (OxideActivity), on the
//! desktop test build with simple stand-ins.

#[cfg(target_os = "android")]
mod imp {
    use jni::{
        objects::{JIntArray, JObject, JString, JValue},
        JNIEnv, JavaVM,
    };
    use std::sync::OnceLock;

    static APP: OnceLock<android_activity::AndroidApp> = OnceLock::new();

    pub fn init(app: &android_activity::AndroidApp) {
        let _ = APP.set(app.clone());
    }

    fn call<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
        let app = APP.get()?;
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.ok()?;
        let mut env = vm.attach_current_thread_permanently().ok()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let r = f(&mut env, &activity);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
            return None;
        }
        r.ok()
    }

    fn ints(env: &mut JNIEnv, obj: JObject) -> jni::errors::Result<Vec<i32>> {
        let arr = JIntArray::from(obj);
        let n = env.get_array_length(&arr)? as usize;
        let mut v = vec![0i32; n];
        env.get_int_array_region(&arr, 0, &mut v)?;
        Ok(v)
    }

    fn string(env: &mut JNIEnv, obj: JObject) -> jni::errors::Result<String> {
        let s = JString::from(obj);
        let text: String = env.get_string(&s)?.into();
        Ok(text)
    }

    pub fn insets() -> [i32; 5] {
        call(|env, a| {
            let o = env.call_method(a, "getInsets", "()[I", &[])?.l()?;
            ints(env, o)
        })
        .and_then(|v| v.try_into().ok())
        .unwrap_or([0; 5])
    }

    pub fn dark() -> Option<bool> {
        call(|env, a| env.call_method(a, "isDark", "()Z", &[])?.z())
    }

    /// Settings pages ask every frame; the answers come from system services (binder), so look once a second.
    fn cached(slot: &std::sync::Mutex<Option<(std::time::Instant, i32)>>, ask: impl FnOnce() -> i32) -> i32 {
        let mut g = slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((t, v)) = *g {
            if t.elapsed() < std::time::Duration::from_secs(1) {
                return v;
            }
        }
        let v = ask();
        *g = Some((std::time::Instant::now(), v));
        v
    }
    static SHOW_SLOT: std::sync::Mutex<Option<(std::time::Instant, i32)>> = std::sync::Mutex::new(None);
    static BRIDGE_SLOT: std::sync::Mutex<Option<(std::time::Instant, i32)>> = std::sync::Mutex::new(None);

    pub fn can_show_on_pc() -> bool {
        cached(&SHOW_SLOT, || call(|env, a| env.call_method(a, "canShowOnPc", "()Z", &[])?.z()).unwrap_or(false) as i32) != 0
    }

    pub fn set_show_on_pc(on: bool) -> bool {
        call(|env, a| env.call_method(a, "setShowOnPc", "(Z)Z", &[JValue::Bool(on as u8)])?.z()).unwrap_or(false)
    }

    /// The watch connection: 0 off, 1 on, 2 waiting for the Bluetooth permission, 3 no Bluetooth.
    pub fn watch_bridge(on: bool) -> i32 {
        *BRIDGE_SLOT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        call(|env, a| env.call_method(a, "watchBridge", "(Z)I", &[JValue::Bool(on as u8)])?.i()).unwrap_or(0)
    }

    pub fn watch_bridge_state() -> i32 {
        cached(&BRIDGE_SLOT, || call(|env, a| env.call_method(a, "watchBridgeState", "()I", &[])?.i()).unwrap_or(0))
    }

    pub fn device_name() -> Option<String> {
        call(|env, a| {
            let o = env.call_method(a, "deviceName", "()Ljava/lang/String;", &[])?.l()?;
            string(env, o)
        })
        .filter(|s| !s.trim().is_empty())
    }

    pub fn system_colors() -> Vec<u32> {
        call(|env, a| {
            let o = env.call_method(a, "systemColors", "()[I", &[])?.l()?;
            ints(env, o)
        })
        .unwrap_or_default()
        .into_iter()
        .map(|c| c as u32)
        .collect()
    }

    fn void_bool(name: &str, on: bool) {
        call(|env, a| env.call_method(a, name, "(Z)V", &[JValue::Bool(on as u8)]).map(|_| ()));
    }

    pub fn light_bars(light: bool) {
        void_bool("lightBars", light)
    }

    pub fn set_fullscreen(on: bool) {
        void_bool("setFullscreen", on)
    }

    pub fn keep_screen_on(on: bool) {
        void_bool("keepScreenOn", on)
    }

    pub fn haptic(kind: i32) {
        call(|env, a| env.call_method(a, "haptic", "(I)V", &[JValue::Int(kind)]).map(|_| ()));
    }

    fn void_str(name: &str, s: &str) {
        call(|env, a| {
            let js = env.new_string(s)?;
            env.call_method(a, name, "(Ljava/lang/String;)V", &[JValue::Object(&js)]).map(|_| ())
        });
    }

    pub fn open_url(url: &str) {
        void_str("openUrl", url)
    }

    pub fn http_get(url: &str) -> Option<String> {
        call(|env, a| {
            let js = env.new_string(url)?;
            let o = env.call_method(a, "httpGet", "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&js)])?.l()?;
            string(env, o)
        })
        .filter(|s| !s.is_empty())
    }

    pub fn install_update(url: &str) {
        void_str("installUpdate", url)
    }

    /// Ask for this phone's fingerprint, face, PIN or pattern; poll `auth_result`.
    pub fn authenticate(title: &str, text: &str) {
        call(|env, a| {
            let (t, x) = (env.new_string(title)?, env.new_string(text)?);
            env.call_method(a, "authenticate", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&t), JValue::Object(&x)]).map(|_| ())
        });
    }

    /// 0 nothing new, 1 still asking, 2 passed, 3 failed or cancelled (2 and 3 are reported once).
    pub fn auth_result() -> i32 {
        call(|env, a| env.call_method(a, "authResult", "()I", &[])?.i()).unwrap_or(0)
    }

    /// (state, progress): 0 idle, 1 downloading, 2 waiting for the user's OK, 3 failed
    pub fn update_status() -> (i32, i32) {
        call(|env, a| {
            let o = env.call_method(a, "updateStatus", "()[I", &[])?.l()?;
            ints(env, o)
        })
        .map(|v| (v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0)))
        .unwrap_or((0, 0))
    }

    pub fn update_error() -> String {
        call(|env, a| {
            let o = env.call_method(a, "updateError", "()Ljava/lang/String;", &[])?.l()?;
            string(env, o)
        })
        .unwrap_or_default()
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    pub fn insets() -> [i32; 5] {
        // a phone-like status bar and gesture bar in the desktop test window (OXIDE_REMOTE_INSETS=top,bottom)
        std::env::var("OXIDE_REMOTE_INSETS")
            .ok()
            .and_then(|v| {
                let mut it = v.split(',').filter_map(|n| n.trim().parse().ok());
                Some([it.next()?, it.next()?, 0, 0, 0])
            })
            .unwrap_or([0; 5])
    }
    pub fn device_name() -> Option<String> {
        None
    }
    pub fn can_show_on_pc() -> bool {
        false
    }
    pub fn set_show_on_pc(_: bool) -> bool {
        false
    }
    pub fn watch_bridge(on: bool) -> i32 {
        i32::from(on)
    }
    pub fn watch_bridge_state() -> i32 {
        1
    }
    pub fn dark() -> Option<bool> {
        std::env::var("OXIDE_REMOTE_LIGHT").ok().map(|_| false).or(Some(true))
    }
    pub fn system_colors() -> Vec<u32> {
        vec![]
    }
    pub fn light_bars(_: bool) {}
    pub fn set_fullscreen(_: bool) {}
    pub fn keep_screen_on(_: bool) {}
    pub fn haptic(_: i32) {}
    pub fn open_url(url: &str) {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    pub fn http_get(url: &str) -> Option<String> {
        let out = std::process::Command::new("curl").args(["-sfL", "--max-time", "10", "-H", "User-Agent: OxideRemote", url]).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
    }
    pub fn install_update(url: &str) {
        open_url(url)
    }
    // the desktop test build has no biometrics: it passes at once
    pub fn authenticate(_: &str, _: &str) {}
    pub fn auth_result() -> i32 {
        2
    }
    pub fn update_status() -> (i32, i32) {
        (0, 0)
    }
    pub fn update_error() -> String {
        String::new()
    }
}

pub use imp::*;
