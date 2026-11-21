//! Power: the performance profile (eco / normal / performance), what the lid does, and what happens when nobody
//! touches the computer.
//!
//! * The profile changes what Oxide draws (eco: no blur, no shadows, no border effects, short animations) and, when
//!   `system_profile` is on, the system's power profile (power-profiles-daemon). `self.config` is the *effective*
//!   configuration; the user's own file is `self.user_config` and is never changed by a profile.
//! * Oxide takes logind's lid-switch inhibitor, so the system does not suspend on its own before Oxide has locked the
//!   screen; `lid_battery` / `lid_ac` = "system" gives the decision back.
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use oxide_config::{Config, PowerCfg};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::Oxide;

/// The configuration as drawn: the user's, adjusted for the profile.
pub fn effective(user: &Config, profile: &str) -> Config {
    let mut c = user.clone();
    match profile {
        "eco" => {
            c.decoration.blur.enabled = false;
            c.decoration.shadow.enabled = false;
            c.decoration.telemetry = false;
            // motion stays, but short and without bounce: the screen still explains what changed
            for a in [&mut c.animations.windows, &mut c.animations.windows_in, &mut c.animations.windows_out, &mut c.animations.workspaces, &mut c.animations.fade_in, &mut c.animations.fade_out, &mut c.animations.border] {
                a.speed = (a.speed * 2.0).min(10.0);
                if a.curve == "bounce" || a.curve == "spring" {
                    a.curve = "ease".into();
                }
            }
        }
        "performance" => {}
        _ => {}
    }
    c
}

/// Whether the computer runs on mains power (true when there is no battery at all).
pub fn on_ac() -> bool {
    let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") else { return true };
    let mut any_battery = false;
    for e in rd.flatten() {
        let t = std::fs::read_to_string(e.path().join("type")).unwrap_or_default();
        // a mouse's or headset's battery is not the computer's
        if std::fs::read_to_string(e.path().join("scope")).is_ok_and(|v| v.trim() == "Device") {
            continue;
        }
        match t.trim() {
            "Mains" | "USB" => {
                if std::fs::read_to_string(e.path().join("online")).is_ok_and(|v| v.trim() == "1") {
                    return true;
                }
            }
            "Battery" => any_battery = true,
            _ => {}
        }
    }
    !any_battery
}

/// Battery charge in percent, if there is a battery.
pub fn battery_percent() -> Option<u32> {
    let rd = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in rd.flatten() {
        let device = std::fs::read_to_string(e.path().join("scope")).is_ok_and(|v| v.trim() == "Device");
        if !device && std::fs::read_to_string(e.path().join("type")).is_ok_and(|t| t.trim() == "Battery") {
            if let Some(p) = std::fs::read_to_string(e.path().join("capacity")).ok().and_then(|v| v.trim().parse().ok()) {
                return Some(p);
            }
        }
    }
    None
}

fn run(cmd: &str, args: &[&str]) {
    let _ = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|mut c| {
        std::thread::spawn(move || {
            let _ = c.wait();
        });
    });
}

impl Oxide {
    /// The profile in force: the chosen one, or eco when the battery is low and "auto eco" is on.
    pub fn active_profile(&self) -> String {
        let p = &self.user_config.power;
        if p.auto_eco_below > 0 && !on_ac() && battery_percent().is_some_and(|b| b < p.auto_eco_below) {
            return "eco".into();
        }
        match p.profile.as_str() {
            "eco" | "performance" => p.profile.clone(),
            _ => "normal".into(),
        }
    }

    /// Make the profile show: the effective config, and the system's power profile.
    pub fn apply_profile(&mut self) {
        let prof = self.active_profile();
        if prof != self.profile_applied {
            self.profile_applied = prof.clone();
            if self.user_config.power.system_profile && std::env::var_os("OXIDE_TEST").is_none() {
                let sys = match prof.as_str() {
                    "eco" => "power-saver",
                    "performance" => "performance",
                    _ => "balanced",
                };
                run("powerprofilesctl", &["set", sys]);
            }
            self.dirty = true;
        }
        self.config = effective(&self.user_config, &prof);
    }

    /// Choose a profile (saved in the config file).
    pub fn set_profile(&mut self, profile: &str) {
        if !matches!(profile, "eco" | "normal" | "performance") {
            return;
        }
        // from the file, not from memory: a change Settings has just written may not have been picked up yet
        let mut c = oxide_config::Config::load().unwrap_or_else(|_| self.user_config.clone());
        c.power.profile = profile.into();
        let _ = c.save();
        self.config_mtime = crate::confwatch::mtime();
        self.apply_config(c);
        self.broadcast_state();
    }

    /// Lock the screen (the shell's lock screen).
    pub fn lock_now(&mut self) {
        if self.locked() {
            return;
        }
        if std::env::var_os("OXIDE_TEST").is_some() {
            tracing::info!("lock requested");
            return;
        }
        run("oxide-ctl", &["lock"]);
    }

    fn has_external_screen(&self) -> bool {
        self.monitors.iter().any(|m| {
            let n = m.output.name();
            !(n.starts_with("eDP") || n.starts_with("LVDS") || n.starts_with("DSI") || n.starts_with("WL") || n.starts_with("winit"))
        })
    }

    fn do_power_action(&mut self, what: &str) {
        let lock = matches!(what, "lock" | "lock_suspend");
        let suspend = matches!(what, "suspend" | "lock_suspend");
        if lock || (suspend && self.user_config.power.lock_before_sleep) {
            self.lock_now();
        }
        if suspend && std::env::var_os("OXIDE_TEST").is_none() {
            // a moment for the lock screen to be up before the screen content is gone
            self.loop_handle
                .insert_source(Timer::from_duration(Duration::from_millis(1200)), |_, _, _| {
                    run("systemctl", &["suspend"]);
                    TimeoutAction::Drop
                })
                .ok();
        }
    }

    /// The lid closed (or opened again).
    pub fn lid_event(&mut self, closed: bool) {
        self.lid_closed = closed;
        if !closed {
            self.lid_pending = None;
            return;
        }
        let p: PowerCfg = self.user_config.power.clone();
        let what = if on_ac() { p.lid_ac.clone() } else { p.lid_battery.clone() };
        if what == "system" || what == "nothing" || (p.lid_ignore_external && self.has_external_screen()) {
            return;
        }
        if p.lid_delay == 0 {
            self.do_power_action(&what);
        } else {
            self.lid_pending = Some((Instant::now() + Duration::from_secs(p.lid_delay as u64), what));
        }
    }

    /// Holds logind's lid-switch inhibitor while Oxide decides what the lid does.
    fn sync_lid_inhibitor(&mut self) {
        let p = &self.user_config.power;
        let want = (p.lid_battery != "system" || p.lid_ac != "system") && std::env::var_os("OXIDE_TEST").is_none();
        let running = self.lid_inhibitor.as_mut().is_some_and(|c: &mut Child| c.try_wait().ok().flatten().is_none());
        if want && !running {
            self.lid_inhibitor = Command::new("systemd-inhibit")
                .args(["--what=handle-lid-switch", "--who=Oxide", "--why=Oxide decides what the lid does", "--mode=block", "sleep", "infinity"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok();
        } else if !want && running {
            if let Some(mut c) = self.lid_inhibitor.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }

    /// Called whenever the configuration is applied.
    pub fn power_config_changed(&mut self) {
        self.sync_lid_inhibitor();
    }

    /// Once at startup: the checks that run all the time.
    pub fn power_start(&mut self) {
        self.sync_lid_inhibitor();
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(5)), |_, _, state| {
            state.power_tick();
            TimeoutAction::ToDuration(Duration::from_secs(5))
        });
    }

    fn power_tick(&mut self) {
        // auto eco follows the battery
        if self.active_profile() != self.profile_applied {
            self.apply_profile();
            self.broadcast_state();
        }
        // a lid that stays closed for `lid_delay`
        if let Some((at, what)) = self.lid_pending.clone() {
            if Instant::now() >= at && self.lid_closed {
                self.lid_pending = None;
                self.do_power_action(&what);
            }
        }
        // idle
        let p = self.user_config.power.clone();
        let ac = on_ac();
        let (lock_min, susp_min) = if ac { (p.idle_lock_ac, p.idle_suspend_ac) } else { (p.idle_lock_battery, p.idle_suspend_battery) };
        let idle = self.last_input.elapsed();
        let fullscreen = p.idle_inhibit_fullscreen && self.wins.iter().any(|w| w.fullscreen && w.visible);
        if fullscreen || std::env::var_os("OXIDE_TEST").is_some() {
            return;
        }
        if susp_min > 0 && idle >= Duration::from_secs(susp_min as u64 * 60) && self.idle_suspended_at.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
            self.idle_suspended_at = Some(Instant::now());
            self.do_power_action("suspend");
            self.last_input = Instant::now();
        } else if lock_min > 0 && idle >= Duration::from_secs(lock_min as u64 * 60) && !self.locked() {
            self.lock_now();
        }
    }
}
