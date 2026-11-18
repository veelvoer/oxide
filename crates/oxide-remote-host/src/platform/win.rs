//! Windows: DXGI desktop duplication (through `scrap`) for the picture, SendInput for the keyboard and mouse, the
//! usual system calls for lock, sleep and power.
use std::mem::size_of;

use windows_sys::Win32::{
    Foundation::POINT,
    System::{
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
        Shutdown::LockWorkStation,
        Registry::{RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ},
    },
    UI::{
        Input::KeyboardAndMouse::*,
        WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SetCursorPos, SetProcessDPIAware, SM_CXSCREEN, SM_CYSCREEN},
    },
};


/// Be told about real pixels, not scaled ones (125 % and 150 % screens): needed for capture and pointer positions.
pub fn dpi_aware() {
    unsafe {
        SetProcessDPIAware();
    }
}

pub fn screen_size() -> (u32, u32) {
    unsafe { (GetSystemMetrics(SM_CXSCREEN).max(1) as u32, GetSystemMetrics(SM_CYSCREEN).max(1) as u32) }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

pub use super::capture_win::Capturer;

// ------------------------------------------------------------- input

fn send(inputs: &[INPUT]) {
    if !inputs.is_empty() {
        unsafe {
            SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
        }
    }
}

fn mouse(flags: u32, dx: i32, dy: i32, data: i32) -> INPUT {
    INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn kbd(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

pub fn pointer_to(x: f64, y: f64) {
    let (w, h) = screen_size();
    unsafe {
        SetCursorPos((x.clamp(0.0, 1.0) * (w - 1) as f64) as i32, (y.clamp(0.0, 1.0) * (h - 1) as f64) as i32);
    }
}

pub fn pointer_by(dx: f64, dy: f64) {
    send(&[mouse(MOUSEEVENTF_MOVE, dx.round() as i32, dy.round() as i32, 0)]);
}

pub fn button(name: &str, pressed: bool) {
    let f = match (name, pressed) {
        ("right", true) => MOUSEEVENTF_RIGHTDOWN,
        ("right", false) => MOUSEEVENTF_RIGHTUP,
        ("middle", true) => MOUSEEVENTF_MIDDLEDOWN,
        ("middle", false) => MOUSEEVENTF_MIDDLEUP,
        (_, true) => MOUSEEVENTF_LEFTDOWN,
        (_, false) => MOUSEEVENTF_LEFTUP,
    };
    send(&[mouse(f, 0, 0, 0)]);
}

pub fn scroll(dx: f64, dy: f64) {
    let mut v = vec![];
    // the phone's "down" is Windows' negative wheel
    if dy.abs() >= 0.5 {
        v.push(mouse(MOUSEEVENTF_WHEEL, 0, 0, (-dy * 6.0).round() as i32));
    }
    if dx.abs() >= 0.5 {
        v.push(mouse(MOUSEEVENTF_HWHEEL, 0, 0, (dx * 6.0).round() as i32));
    }
    send(&v);
}

/// Names like the Linux side uses (xkb keysyms) to virtual-key codes.
fn vk_of(name: &str) -> Option<u16> {
    let n = name.to_lowercase();
    Some(match n.as_str() {
        "return" | "enter" => VK_RETURN,
        "escape" | "esc" => VK_ESCAPE,
        "tab" => VK_TAB,
        "backspace" => VK_BACK,
        "delete" => VK_DELETE,
        "insert" => VK_INSERT,
        "space" => VK_SPACE,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        "home" => VK_HOME,
        "end" => VK_END,
        "page_up" | "prior" => VK_PRIOR,
        "page_down" | "next" => VK_NEXT,
        "super_l" | "super" | "win" => VK_LWIN,
        "alt_l" | "alt" => VK_MENU,
        "control_l" | "ctrl" => VK_CONTROL,
        "shift_l" | "shift" => VK_SHIFT,
        "print" => VK_SNAPSHOT,
        "caps_lock" => VK_CAPITAL,
        s if s.len() == 1 => {
            let c = s.as_bytes()[0];
            match c {
                b'a'..=b'z' => (c - b'a') as u16 + 0x41,
                b'0'..=b'9' => c as u16,
                _ => return None,
            }
        }
        s if s.starts_with('f') && s[1..].parse::<u16>().is_ok_and(|k| (1..=24).contains(&k)) => 0x70 + s[1..].parse::<u16>().unwrap() - 1,
        _ => return None,
    })
}

fn is_extended(vk: u16) -> bool {
    matches!(vk, VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_INSERT | VK_DELETE | VK_LWIN)
}

pub fn key(name: &str, mods: &[String]) -> Result<(), String> {
    let vk = vk_of(name).ok_or_else(|| format!("unknown key {name}"))?;
    let mut codes: Vec<u16> = vec![];
    for m in mods {
        if let Some(c) = vk_of(m) {
            codes.push(c);
        }
    }
    let mut v = vec![];
    let flag = |vk: u16, up: bool| (if up { KEYEVENTF_KEYUP } else { 0 }) | if is_extended(vk) { KEYEVENTF_EXTENDEDKEY } else { 0 };
    for &c in &codes {
        v.push(kbd(c, 0, flag(c, false)));
    }
    v.push(kbd(vk, 0, flag(vk, false)));
    v.push(kbd(vk, 0, flag(vk, true)));
    for &c in codes.iter().rev() {
        v.push(kbd(c, 0, flag(c, true)));
    }
    send(&v);
    Ok(())
}

pub fn text(t: &str) -> Result<(), String> {
    let mut v = vec![];
    for u in t.encode_utf16() {
        v.push(kbd(0, u, KEYEVENTF_UNICODE));
        v.push(kbd(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    send(&v);
    Ok(())
}

// ------------------------------------------------------------- actions

fn tap(vk: u16) {
    send(&[kbd(vk, 0, if is_extended(vk) { KEYEVENTF_EXTENDEDKEY } else { 0 }), kbd(vk, 0, KEYEVENTF_KEYUP)]);
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(cmd)
        .args(args)
        .creation_flags(0x0800_0000) // no console window
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{cmd}: {e}"))
}

fn combo(mods: &[u16], key: u16) {
    let mut v = vec![];
    for &m in mods {
        v.push(kbd(m, 0, if is_extended(m) { KEYEVENTF_EXTENDEDKEY } else { 0 }));
    }
    v.push(kbd(key, 0, if is_extended(key) { KEYEVENTF_EXTENDEDKEY } else { 0 }));
    v.push(kbd(key, 0, KEYEVENTF_KEYUP | if is_extended(key) { KEYEVENTF_EXTENDEDKEY } else { 0 }));
    for &m in mods.iter().rev() {
        v.push(kbd(m, 0, KEYEVENTF_KEYUP | if is_extended(m) { KEYEVENTF_EXTENDEDKEY } else { 0 }));
    }
    send(&v);
}

pub fn action(name: &str, arg: Option<&str>) -> Result<(), String> {
    match name {
        "lock" => {
            unsafe { LockWorkStation() };
            Ok(())
        }
        "suspend" => run("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        "reboot" => run("shutdown.exe", &["/r", "/t", "0"]),
        "shutdown" => run("shutdown.exe", &["/s", "/t", "0"]),
        "logout" => run("shutdown.exe", &["/l"]),
        "media_play_pause" => Ok(tap(VK_MEDIA_PLAY_PAUSE)),
        "media_next" => Ok(tap(VK_MEDIA_NEXT_TRACK)),
        "media_prev" => Ok(tap(VK_MEDIA_PREV_TRACK)),
        "mute" => Ok(tap(VK_VOLUME_MUTE)),
        "volume_up" => Ok((0..2).for_each(|_| tap(VK_VOLUME_UP))),
        "volume_down" => Ok((0..2).for_each(|_| tap(VK_VOLUME_DOWN))),
        "volume" => Ok(()), // absolute volume needs the audio API: the buttons above do the job
        "screenshot" => {
            combo(&[VK_LWIN, VK_SHIFT], b'S' as u16);
            Ok(())
        }
        "open_url" => run("cmd.exe", &["/c", "start", "", arg.ok_or("no address")?]),
        "run" => {
            let c = arg.filter(|a| !a.trim().is_empty()).ok_or("nothing to run")?;
            run("cmd.exe", &["/c", "start", "", c])
        }
        "notify" => {
            // a message box in its own thread
            let t = arg.unwrap_or("").to_string();
            let _ = run("msg.exe", &["*", &t]);
            Ok(())
        }
        "clipboard_set" => arboard::Clipboard::new().and_then(|mut c| c.set_text(arg.unwrap_or("").to_string())).map_err(|e| e.to_string()),
        // compositor-style actions from the Remote tab, mapped onto Windows' own shortcuts
        "dispatch" => {
            let j: serde_json::Value = serde_json::from_str(arg.ok_or("no action")?).map_err(|e| e.to_string())?;
            match j["type"].as_str().unwrap_or("") {
                "toggle_overview" => combo(&[VK_LWIN], VK_TAB),
                "close" => combo(&[VK_MENU], VK_F4),
                "toggle_fullscreen" => tap(VK_F11),
                "toggle_floating" => combo(&[VK_LWIN], VK_UP),
                "workspace_existing" | "workspace_rel" => {
                    let d = j["arg"].as_i64().unwrap_or(1);
                    combo(&[VK_LWIN, VK_CONTROL], if d < 0 { VK_LEFT } else { VK_RIGHT });
                }
                other => return Err(format!("{other} does not exist on Windows")),
            }
            Ok(())
        }
        "workspace" => Err("virtual desktops can only be switched one by one here".into()),
        other => Err(format!("unknown action {other}")),
    }
}

pub fn os_name() -> String {
    "Windows".into()
}

pub fn battery() -> (Option<f32>, bool) {
    let mut s = SYSTEM_POWER_STATUS { ACLineStatus: 0, BatteryFlag: 0, BatteryLifePercent: 0, SystemStatusFlag: 0, BatteryLifeTime: 0, BatteryFullLifeTime: 0 };
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 || s.BatteryFlag == 128 || s.BatteryLifePercent > 100 {
        return (None, false);
    }
    (Some(s.BatteryLifePercent as f32), s.ACLineStatus == 1)
}

// ------------------------------------------------------------- start with Windows

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "OxideRemoteHost";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn autostart() -> bool {
    unsafe {
        let mut key = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return false;
        }
        let r = RegQueryValueExW(key, wide(VALUE).as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        RegCloseKey(key);
        r == 0
    }
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    unsafe {
        let mut key = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_WRITE, &mut key) != 0 {
            return Err("cannot open the startup list".into());
        }
        let r = if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = wide(&format!("\"{}\" --minimized", exe.display()));
            RegSetValueExW(key, wide(VALUE).as_ptr(), 0, REG_SZ, cmd.as_ptr() as *const u8, (cmd.len() * 2) as u32)
        } else {
            RegDeleteValueW(key, wide(VALUE).as_ptr())
        };
        RegCloseKey(key);
        if r == 0 || !on {
            Ok(())
        } else {
            Err("could not change it".into())
        }
    }
}
