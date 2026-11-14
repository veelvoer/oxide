//! Macro scratchpads: a named group of apps (say a music player and a system monitor) that one key summons and
//! dismisses together. Shown, the group sits side by side in a centered block above the current workspace and
//! moves as one; hidden, its windows wait on their own workspace. Missing members are started on the first toggle.
use std::time::{Duration, Instant};

use oxide_config::GROUP_WS;

use crate::state::Oxide;

fn app_matches(want: &str, app_id: &str) -> bool {
    !want.is_empty() && want.eq_ignore_ascii_case(app_id)
}

impl Oxide {
    pub fn toggle_scratchpad(&mut self, name: &str) {
        let Some(idx) = self.config.scratchpads.iter().position(|s| s.name.eq_ignore_ascii_case(name)) else {
            tracing::warn!("no scratchpad named {name:?}");
            return;
        };
        let gws = GROUP_WS + idx as u32;
        let mi = self.focused_mon;
        if self.monitors.get(mi).is_some_and(|m| m.overlay == Some(gws)) {
            self.monitors[mi].overlay = None;
            self.focus = None;
            self.arrange();
            self.refocus();
            return;
        }
        for m in self.monitors.iter_mut() {
            if m.overlay == Some(gws) {
                m.overlay = None;
            }
        }
        let spec = self.config.scratchpads[idx].clone();
        self.group_pending.retain(|p| p.2.elapsed() < Duration::from_secs(20));
        for app in &spec.apps {
            let found = self.wins.iter().position(|w| app_matches(&app.app_id, &crate::wm::window_info(&w.window).0));
            match found {
                Some(i) => {
                    if self.wins[i].ws != gws {
                        // take it out of wherever it was
                        self.wins[i].ws = gws;
                        self.wins[i].floating = false;
                        self.wins[i].fullscreen = false;
                    }
                }
                None => {
                    if !app.exec.is_empty() && !self.group_pending.iter().any(|p| app_matches(&app.app_id, &p.0)) {
                        self.group_pending.push((app.app_id.clone(), gws, Instant::now()));
                        self.spawn(&app.exec);
                    }
                }
            }
        }
        if let Some(m) = self.monitors.get_mut(mi) {
            m.overlay = Some(gws);
        }
        // the group arrives like windows that open (scale and fade in)
        for w in self.wins.iter_mut().filter(|w| w.ws == gws) {
            w.base.w = 0.0;
        }
        self.arrange();
        if let Some(w) = self.wins.iter().find(|w| w.ws == gws).map(|w| w.window.clone()) {
            self.focus_window(Some(w));
        }
    }

    /// For a window that just opened: the scratchpad workspace it was started for, if any.
    pub fn pending_group_for(&mut self, app_id: &str) -> Option<u32> {
        self.group_pending.retain(|p| p.2.elapsed() < Duration::from_secs(20));
        let i = self.group_pending.iter().position(|p| app_matches(&p.0, app_id))?;
        Some(self.group_pending.remove(i).1)
    }
}
