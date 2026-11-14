//! Macro scratchpads: a named group of apps (say a music player and a system monitor) that one key summons and
//! dismisses together. Shown, the group sits side by side in a centered block above the current workspace and
//! moves as one; hidden, its windows wait on their own workspace. Missing members are started on the first toggle.
use std::time::{Duration, Instant};

use oxide_config::GROUP_WS;
use smithay::desktop::layer_map_for_output;
use smithay::utils::{Logical, Point};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::state::Oxide;

fn app_matches(want: &str, app_id: &str) -> bool {
    !want.is_empty() && want.eq_ignore_ascii_case(app_id)
}

impl Oxide {
    /// A click on the dimmed area around a shown group sends the group away (the click goes no further).
    pub fn group_click(&mut self, pos: Point<f64, Logical>) -> bool {
        let Some(mi) = self.monitors.iter().position(|m| {
            m.overlay.is_some() && self.space.output_geometry(&m.output).is_some_and(|g| g.to_f64().contains(pos))
        }) else {
            return false;
        };
        let gws = self.monitors[mi].overlay.unwrap_or_default();
        let ogeo = self.space.output_geometry(&self.monitors[mi].output).unwrap_or_default();
        {
            let layers = layer_map_for_output(&self.monitors[mi].output);
            let local = pos - ogeo.loc.to_f64();
            if layers.layer_under(Layer::Overlay, local).is_some() || layers.layer_under(Layer::Top, local).is_some() {
                return false;
            }
        }
        // anywhere inside the block (also the gaps between members) counts as the group
        let on_group = self
            .wins
            .iter()
            .filter(|x| x.ws == gws)
            .filter_map(|x| self.space.element_geometry(&x.window))
            .reduce(|a, b| a.merge(b))
            .is_some_and(|r| r.to_f64().contains(pos));
        if on_group {
            return false;
        }
        self.monitors[mi].overlay = None;
        self.focus = None;
        self.arrange();
        self.refocus();
        true
    }

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
