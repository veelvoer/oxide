//! Window management: workspaces, tiling, focus and actions.
use oxide_config::{Action, Dir, LayoutKind, WindowRule, SPECIAL_WS};
use oxide_ipc::{FocusedWindow, OutputInfo, State, WindowInfo, WorkspaceInfo};
use oxide_layout::{apply_gaps, compute, neighbor, Rect};
use smithay::{
    backend::renderer::element::solid::SolidColorBuffer,
    desktop::{layer_map_for_output, Window},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::State as TState,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size, SERIAL_COUNTER},
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};
use tracing::info;

use std::time::Duration;

use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use smithay::wayland::seat::WaylandFocus;

use crate::{
    anim::{popin_scale, progress, RectF, Tween},
    state::{Closing, Oxide, Win, WsAnim},
};

fn to_rect(r: Rectangle<i32, Logical>) -> Rect {
    Rect::new(r.loc.x, r.loc.y, r.size.w, r.size.h)
}
fn from_rect(r: Rect) -> Rectangle<i32, Logical> {
    Rectangle::new((r.x, r.y).into(), (r.w, r.h).into())
}

/// The size of a window whose minimum and maximum size are equal.
fn fixed_size(window: &Window) -> Option<(i32, i32)> {
    let t = window.toplevel()?;
    smithay::wayland::compositor::with_states(t.wl_surface(), |st| {
        let c = *st.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>().current();
        (c.min_size.w > 0 && c.min_size.h > 0 && c.min_size == c.max_size).then_some((c.min_size.w, c.min_size.h))
    })
}

/// Dialogs always float: a window that cannot be resized, an authentication prompt, a file chooser, a small utility.
fn is_dialog_like(window: &Window, app_id: &str, title: &str) -> bool {
    // fixed size (min == max): a dialog by definition
    if let Some(t) = window.toplevel() {
        let fixed = smithay::wayland::compositor::with_states(t.wl_surface(), |st| {
            let c = *st.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>().current();
            c.min_size.w > 0 && c.min_size.h > 0 && c.min_size == c.max_size
        });
        if fixed {
            return true;
        }
    }
    if let Some(x) = window.x11_surface() {
        let t = x.window_type();
        if matches!(
            t,
            Some(smithay::xwayland::xwm::WmWindowType::Dialog | smithay::xwayland::xwm::WmWindowType::Utility | smithay::xwayland::xwm::WmWindowType::Splash | smithay::xwayland::xwm::WmWindowType::Toolbar)
        ) {
            return true;
        }
    }
    let id = app_id.to_lowercase();
    let known = [
        "gcr-prompter", "polkit", "pinentry", "zenity", "kdialog", "nm-connection-editor", "pavucontrol", "blueman", "xdg-desktop-portal",
        "org.kde.kwalletd", "kwalletmanager", "seahorse", "gnome-keyring", "ksshaskpass", "lxqt-openssh-askpass", "org.freedesktop.impl.portal",
    ];
    if known.iter().any(|k| id.contains(k)) {
        return true;
    }
    let t = title.to_lowercase();
    ["authentication required", "authenticate", "unlock login keyring", "unlock keyring", "open file", "save file", "save as", "choose a file", "select a file", "file upload"]
        .iter()
        .any(|k| t.contains(k))
}

pub fn surface_of(w: &Window) -> Option<WlSurface> {
    w.wl_surface().map(|s| s.into_owned())
}

/// (app id / class, title) for wayland and X11 windows alike.
fn window_info(w: &Window) -> (String, String) {
    if let Some(x) = w.x11_surface() {
        return (x.class(), x.title());
    }
    surface_of(w).map(|s| app_id_and_title(&s)).unwrap_or_default()
}

fn app_id_and_title(surface: &WlSurface) -> (String, String) {
    with_states(surface, |s| {
        s.data_map
            .get::<XdgToplevelSurfaceData>()
            .map(|d| {
                let d = d.lock().unwrap();
                (d.app_id.clone().unwrap_or_default(), d.title.clone().unwrap_or_default())
            })
            .unwrap_or_default()
    })
}

fn rule_matches(rule: &WindowRule, app_id: &str, title: &str) -> bool {
    if rule.app_id.is_none() && rule.title.is_none() {
        return false;
    }
    rule.app_id.as_ref().map_or(true, |a| a.eq_ignore_ascii_case(app_id))
        && rule.title.as_ref().map_or(true, |t| title.to_lowercase().contains(&t.to_lowercase()))
}

impl Oxide {
    pub fn cur_ws(&self) -> u32 {
        self.monitors.get(self.focused_mon).map_or(1, |m| m.ws)
    }

    fn shown_on(&self, ws: u32) -> Option<usize> {
        self.monitors.iter().position(|m| m.ws == ws)
    }

    pub fn layout_of(&self, ws: u32) -> LayoutKind {
        self.ws_layout.get(&ws).copied().unwrap_or(self.config.general.layout)
    }

    pub fn win_idx(&self, window: &Window) -> Option<usize> {
        self.wins.iter().position(|w| &w.window == window)
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.wins.iter().find(|w| surface_of(&w.window).as_ref() == Some(surface)).map(|w| w.window.clone())
    }

    /// Recompute opacity and blur of every open window from the rules (after they changed).
    pub fn reapply_rules(&mut self) {
        for i in 0..self.wins.len() {
            let (app_id, title) = window_info(&self.wins[i].window);
            let (mut opacity, mut blur) = (None, None);
            for rule in self.config.rules.iter().filter(|r| rule_matches(r, &app_id, &title)) {
                if rule.opacity.is_some() {
                    opacity = rule.opacity;
                }
                if rule.blur.is_some() {
                    blur = rule.blur;
                }
            }
            self.wins[i].opacity = opacity;
            self.wins[i].blur = blur;
        }
        self.dirty = true;
    }

    /// Called on the first commit of a toplevel: apply rules, place and focus it.
    pub fn map_pending(&mut self, window: Window) {
        let (app_id, title) = window_info(&window);
        let mut floating = window.toplevel().is_some_and(|t| t.parent().is_some())
            || window.x11_surface().is_some_and(|x| x.is_transient_for().is_some() || x.is_popup())
            || is_dialog_like(&window, &app_id, &title);
        let mut ws = self.cur_ws();
        let mut opacity = None;
        let mut blur = None;
        for rule in self.config.rules.iter().filter(|r| rule_matches(r, &app_id, &title)) {
            if let Some(f) = rule.floating {
                floating = f;
            }
            if let Some(w) = rule.workspace {
                ws = w.clamp(1, SPECIAL_WS - 1);
            }
            if rule.opacity.is_some() {
                opacity = rule.opacity;
            }
            if rule.blur.is_some() {
                blur = rule.blur;
            }
        }
        info!(app_id, title, ws, floating, "mapping window");
        self.wins.push(Win {
            window: window.clone(),
            ws,
            floating,
            fullscreen: false,
            float_geo: None,
            fixed: fixed_size(&window),
            rect: Rectangle::default(),
            base: RectF::default(),
            cur: RectF::default(),
            mv: None,
            open: None,
            fade: 1.0,
            fade_anim: None,
            border_mix: 0.0,
            border_anim: None,
            border: 0,
            tiled: false,
            visible: false,
            opacity,
            blur,
            blur_id: smithay::backend::renderer::element::Id::new(),
            borders: std::array::from_fn(|_| SolidColorBuffer::default()),
            dim: SolidColorBuffer::default(),
            border_el: None,
            shadow_el: None,
        });
        self.arrange();
        if self.shown_on(ws).is_some() {
            self.focus_window(Some(window));
        }
    }

    /// Take a snapshot of a window that is being closed so it can fade out after the client is gone.
    pub fn start_close(&mut self, surface: &WlSurface) {
        let anims = &self.config.animations;
        if !(anims.enabled && anims.windows_out.enabled) {
            return;
        }
        let Some(idx) = self.wins.iter().position(|w| surface_of(&w.window).as_ref() == Some(surface)) else { return };
        let w = &self.wins[idx];
        if !w.visible || w.fullscreen || w.window.x11_surface().is_some() || w.base.w <= 0.0 {
            return;
        }
        let (window, rect, ws, border, mix) = (w.window.clone(), w.cur, w.ws, w.border, w.border_mix);
        let radius = self.config.decoration.rounding.max(0);
        let scale = self
            .monitors
            .iter()
            .find(|m| m.ws == ws)
            .map_or(1.0, |m| m.output.current_scale().fractional_scale());
        let start: Duration = self.clock.now().into();
        let tex = self.with_renderer(|_, r| snapshot_window(r, &window, scale)).flatten();
        if let Some(texture) = tex {
            self.closing.push(Closing {
                texture,
                id: smithay::backend::renderer::element::Id::new(),
                rect,
                border,
                radius,
                ws,
                start,
                border_mix: mix,
            });
            self.kick_animations();
        }
    }

    pub fn remove_window(&mut self, surface: &WlSurface) {
        self.pending.retain(|w| surface_of(w).as_ref() != Some(surface));
        let Some(idx) = self.wins.iter().position(|w| surface_of(&w.window).as_ref() == Some(surface)) else { return };
        self.remove_win_at(idx);
    }

    pub fn remove_win_at(&mut self, idx: usize) {
        let win = self.wins.remove(idx);
        self.space.unmap_elem(&win.window);
        self.last_focus.retain(|_, w| *w != win.window);
        let had_focus = self.focus.as_ref() == Some(&win.window);
        if had_focus {
            self.focus = None;
        }
        self.arrange();
        if had_focus {
            self.refocus();
        }
    }

    /// Focus the remembered or topmost window of the current workspace.
    pub fn refocus(&mut self) {
        let ws = self.cur_ws();
        let next = self
            .last_focus
            .get(&ws)
            .cloned()
            .filter(|w| self.win_idx(w).is_some_and(|i| self.wins[i].ws == ws))
            .or_else(|| self.wins.iter().rev().find(|w| w.ws == ws).map(|w| w.window.clone()));
        self.focus_window(next);
    }

    pub fn focus_window(&mut self, window: Option<Window>) {
        if self.locked() {
            self.focus_lock();
            return;
        }
        let serial = SERIAL_COUNTER.next_serial();
        self.layer_focus = None;
        for w in &self.wins {
            let active = window.as_ref() == Some(&w.window);
            if w.window.set_activated(active) {
                if let Some(t) = w.window.toplevel() {
                    t.send_pending_configure();
                }
            }
        }
        if let Some(w) = &window {
            if let Some(i) = self.win_idx(w) {
                let ws = self.wins[i].ws;
                self.last_focus.insert(ws, w.clone());
                if let Some(mi) = self.shown_on(ws) {
                    self.focused_mon = mi;
                }
                if self.wins[i].floating {
                    self.space.raise_element(w, false);
                }
            }
        }
        let kb = self.seat.get_keyboard().unwrap();
        kb.set_focus(self, window.as_ref().and_then(surface_of), serial);
        if let Some(x) = window.as_ref().and_then(|w| w.x11_surface()) {
            smithay::input::keyboard::KeyboardTarget::enter(x, &self.seat.clone(), self, vec![], serial);
        }
        self.focus = window;
        self.update_borders();
        self.dirty = true;
    }

    /// Focus a layer-shell surface (launcher, lock screen, ...).
    pub fn focus_layer(&mut self, surface: WlSurface) {
        if self.locked() {
            self.focus_lock();
            return;
        }
        let serial = SERIAL_COUNTER.next_serial();
        for w in &self.wins {
            if w.window.set_activated(false) {
                if let Some(t) = w.window.toplevel() {
                    t.send_pending_configure();
                }
            }
        }
        let kb = self.seat.get_keyboard().unwrap();
        kb.set_focus(self, Some(surface.clone()), serial);
        self.layer_focus = Some(surface);
        self.focus = None;
        self.update_borders();
    }

    pub fn update_borders(&mut self) {
        self.retarget_borders();
    }

    /// Recompute the layout of every visible workspace and configure windows.
    pub fn arrange(&mut self) {
        let g = self.config.general.clone();
        let bs = g.border_size.max(0);

        let mut visible = vec![false; self.wins.len()];
        for mi in 0..self.monitors.len() {
            let out = self.monitors[mi].output.clone();
            let ws = self.monitors[mi].ws;
            let Some(ogeo) = self.space.output_geometry(&out) else { continue };
            // output size/scale may have changed since the layers were last arranged
            let zone = {
                let mut map = layer_map_for_output(&out);
                map.arrange();
                map.non_exclusive_zone()
            };
            let area = Rect::new(ogeo.loc.x + zone.loc.x, ogeo.loc.y + zone.loc.y, zone.size.w, zone.size.h);

            let snap: Vec<(u32, bool, bool)> = self.wins.iter().map(|w| (w.ws, w.floating, w.fullscreen)).collect();
            let pick = |pred: &dyn Fn(bool, bool) -> bool| -> Vec<usize> {
                (0..snap.len()).filter(|&i| snap[i].0 == ws && pred(snap[i].1, snap[i].2)).collect()
            };
            let tiled = pick(&|fl, fs| !fl && !fs);
            let floating = pick(&|fl, fs| fl && !fs);
            let full = pick(&|_, fs| fs);
            let rects = compute(
                self.layout_of(ws),
                area,
                tiled.len(),
                self.config.layout.split_ratio,
                self.config.layout.master_factor,
            );
            let rects = apply_gaps(&rects, area, g.gaps_in, g.gaps_out);
            for (&i, r) in tiled.iter().zip(rects) {
                visible[i] = true;
                self.wins[i].visible = true;
                self.place(i, from_rect(r), bs, true);
            }
            for i in floating {
                visible[i] = true;
                self.wins[i].visible = true;
                let fixed = self.wins[i].fixed;
                let geo = self.wins[i].float_geo.unwrap_or_else(|| {
                    let (w, h) = match fixed {
                        Some((fw, fh)) => (fw + 2 * bs, fh + 2 * bs),
                        None => ((area.w as f32 * g.float_size) as i32, (area.h as f32 * g.float_size) as i32),
                    };
                    Rectangle::new((area.x + (area.w - w) / 2, area.y + (area.h - h) / 2).into(), (w, h).into())
                });
                self.wins[i].float_geo = Some(geo);
                self.place(i, geo, bs, false);
            }
            for i in full {
                visible[i] = true;
                self.wins[i].visible = true;
                self.place(i, ogeo, 0, false);
            }
        }
        for (i, vis) in visible.iter().enumerate() {
            self.wins[i].visible = *vis;
            if !vis {
                self.space.unmap_elem(&self.wins[i].window);
            }
        }
        // keep floating and fullscreen windows above tiled ones
        let order: Vec<Window> = self
            .wins
            .iter()
            .filter(|w| visible[self.win_idx(&w.window).unwrap()] && w.floating)
            .map(|w| w.window.clone())
            .collect();
        for w in order {
            self.space.raise_element(&w, false);
        }
        self.update_borders();
        self.dirty = true;
        self.kick_animations();
    }

    fn place(&mut self, i: usize, outer: Rectangle<i32, Logical>, border: i32, tiled: bool) {
        let now: Duration = self.clock.now().into();
        let anims = &self.config.animations;
        let win = &mut self.wins[i];
        let previous = win.rect;
        win.rect = outer;
        win.border = border;
        win.tiled = tiled;
        let target = RectF::from_rect(outer);
        if win.base.w <= 0.0 {
            // first time this window is placed: it opens
            win.base = target;
            win.mv = None;
            win.open = (!win.fullscreen && anims.enabled && anims.windows_in.enabled).then_some(now);
            if anims.enabled && anims.fade_in.enabled && !win.fullscreen {
                win.fade = 0.0;
                win.fade_anim = Some(Tween { from: 0.0, to: 1.0, start: now });
            } else {
                win.fade = 1.0;
            }
        } else if !(anims.enabled && anims.windows.enabled) {
            win.base = target;
            win.mv = None;
        } else if previous != outer {
            win.mv = Some(Tween { from: win.base, to: target, start: now });
        }
        self.eval_win(i, now);
        self.apply_visual(i);
    }

    /// Advance the animations of window `i` to `now`. Returns true while something still moves.
    fn eval_win(&mut self, i: usize, now: Duration) -> bool {
        let anims = &self.config.animations;
        let w = &mut self.wins[i];
        let target = RectF::from_rect(w.rect);
        let mut active = false;

        match w.mv {
            Some(m) => {
                let (p, done) = progress(anims, &anims.windows, now.saturating_sub(m.start).as_secs_f64());
                if done {
                    w.mv = None;
                    w.base = target;
                } else {
                    w.base = m.from.lerp(target, p);
                    active = true;
                }
            }
            None => w.base = target,
        }

        w.cur = w.base;
        if let Some(start) = w.open {
            let (p, done) = progress(anims, &anims.windows_in, now.saturating_sub(start).as_secs_f64());
            if done {
                w.open = None;
            } else {
                active = true;
                if let Some(s0) = popin_scale(&anims.windows_in.style) {
                    w.cur = w.base.scaled(s0 + (1.0 - s0) * p);
                }
            }
        }

        if let Some(t) = w.fade_anim {
            let leaf = if t.to >= t.from { &anims.fade_in } else { &anims.fade_out };
            let (p, done) = progress(anims, leaf, now.saturating_sub(t.start).as_secs_f64());
            if done {
                w.fade = t.to;
                w.fade_anim = None;
            } else {
                w.fade = (t.from as f64 + (t.to - t.from) as f64 * p).clamp(0.0, 1.0) as f32;
                active = true;
            }
        }

        if let Some(t) = w.border_anim {
            let (p, done) = progress(anims, &anims.border, now.saturating_sub(t.start).as_secs_f64());
            if done {
                w.border_mix = t.to;
                w.border_anim = None;
            } else {
                w.border_mix = (t.from as f64 + (t.to - t.from) as f64 * p).clamp(0.0, 1.0) as f32;
                active = true;
            }
        }
        active
    }

    /// Configure the client to the final size and place it in the space. Animations only move
    /// pixels on screen; the client is never resized every frame.
    fn apply_visual(&mut self, i: usize) {
        let win = &self.wins[i];
        let b = win.border;
        let outer = win.rect;
        let inner_loc: Point<i32, Logical> = (outer.loc.x + b, outer.loc.y + b).into();
        let inner_size: Size<i32, Logical> = ((outer.size.w - 2 * b).max(1), (outer.size.h - 2 * b).max(1)).into();
        let (tiled, fullscreen) = (win.tiled, win.fullscreen);
        if let Some(x) = win.window.x11_surface() {
            let _ = x.set_fullscreen(fullscreen);
            let _ = x.configure(Rectangle::new(inner_loc, inner_size));
        }
        if let Some(t) = win.window.toplevel() {
            t.with_pending_state(|st| {
                st.size = Some(inner_size);
                for s in [TState::TiledLeft, TState::TiledRight, TState::TiledTop, TState::TiledBottom] {
                    if tiled {
                        st.states.set(s);
                    } else {
                        st.states.unset(s);
                    }
                }
                if fullscreen {
                    st.states.set(TState::Fullscreen);
                } else {
                    st.states.unset(TState::Fullscreen);
                }
            });
            if t.is_initial_configure_sent() {
                t.send_pending_configure();
            } else {
                t.send_configure();
            }
        }
        if win.visible {
            let window = win.window.clone();
            self.space.map_element(window, inner_loc, false);
        }
    }

    /// Advance all running animations. Returns whether any are still active.
    pub fn tick_animations(&mut self) -> bool {
        let now: Duration = self.clock.now().into();
        let mut active = false;
        for i in 0..self.wins.len() {
            if self.eval_win(i, now) {
                active = true;
            }
        }
        // finished close animations
        let anims = &self.config.animations;
        self.closing.retain(|c| {
            let e = now.saturating_sub(c.start).as_secs_f64();
            let (_, d1) = progress(anims, &anims.windows_out, e);
            let (_, d2) = progress(anims, &anims.fade_out, e);
            !(d1 && d2)
        });
        if !self.closing.is_empty() {
            active = true;
        }
        if let Some(wa) = &self.ws_anim {
            let e = now.saturating_sub(wa.start).as_secs_f64();
            if progress(anims, &anims.workspaces, e).1 {
                self.ws_anim = None;
            } else {
                active = true;
            }
        }
        self.dirty = true;
        active
    }

    pub fn animations_running(&self) -> bool {
        self.ws_anim.is_some()
            || !self.closing.is_empty()
            || self.wins.iter().any(|w| w.mv.is_some() || w.open.is_some() || w.fade_anim.is_some() || w.border_anim.is_some())
    }

    /// Start the frame timer if animations are running and it is not already ticking.
    pub fn kick_animations(&mut self) {
        if self.anim_timer || !self.animations_running() {
            return;
        }
        let refresh = self
            .monitors
            .iter()
            .filter_map(|m| m.output.current_mode())
            .map(|m| m.refresh)
            .max()
            .filter(|r| *r > 0)
            .unwrap_or(60_000);
        let frame = Duration::from_micros(1_000_000_000 / refresh as u64);
        self.anim_timer = true;
        let res = self.loop_handle.insert_source(Timer::from_duration(frame), move |_, _, state| {
            if state.tick_animations() {
                TimeoutAction::ToDuration(frame)
            } else {
                state.anim_timer = false;
                TimeoutAction::Drop
            }
        });
        if res.is_err() {
            self.anim_timer = false;
        }
    }

    /// Fade the border of every window toward its active/inactive state.
    pub fn retarget_borders(&mut self) {
        let now: Duration = self.clock.now().into();
        let animate = self.config.animations.enabled && self.config.animations.border.enabled;
        for w in &mut self.wins {
            let target = if self.focus.as_ref() == Some(&w.window) { 1.0 } else { 0.0 };
            let goal = w.border_anim.map_or(w.border_mix, |t| t.to);
            if (goal - target).abs() < f32::EPSILON {
                continue;
            }
            if animate {
                w.border_anim = Some(Tween { from: w.border_mix, to: target, start: now });
            } else {
                w.border_mix = target;
                w.border_anim = None;
            }
        }
        self.kick_animations();
    }

    pub fn switch_workspace(&mut self, n: u32) {
        if n == 0 || n >= SPECIAL_WS {
            return;
        }
        let leaving = self.cur_ws();
        if let Some(mi) = self.shown_on(n) {
            self.focused_mon = mi;
        } else if let Some(m) = self.monitors.get_mut(self.focused_mon) {
            let old = m.ws;
            m.ws = n;
            let a = &self.config.animations;
            if a.enabled && a.workspaces.enabled && a.workspaces.style.trim() == "slide" && old != n {
                self.ws_anim = Some(WsAnim {
                    mon: self.focused_mon,
                    old_ws: old,
                    dir: if n > old { 1 } else { -1 },
                    start: self.clock.now().into(),
                });
            }
        }
        // a workspace nobody is on and without windows stops existing
        if leaving != n && !self.wins.iter().any(|w| w.ws == leaving) {
            self.last_focus.remove(&leaving);
            self.ws_layout.remove(&leaving);
        }
        self.arrange();
        self.refocus();
        self.kick_animations();
    }

    /// Workspace id `d` steps away from the current one (never below 1). Workspaces are unlimited.
    fn rel_workspace(&self, d: i32) -> u32 {
        let cur = self.monitors.get(self.focused_mon).map_or(1, |m| if m.ws == SPECIAL_WS { m.prev_ws } else { m.ws });
        (cur as i64 + d as i64).clamp(1, SPECIAL_WS as i64 - 1) as u32
    }

    /// The next (d > 0) or previous (d < 0) workspace that has windows or is on a screen.
    fn existing_workspace(&self, d: i32) -> u32 {
        let cur = self.monitors.get(self.focused_mon).map_or(1, |m| if m.ws == SPECIAL_WS { m.prev_ws } else { m.ws });
        let mut ids: Vec<u32> = self
            .wins
            .iter()
            .map(|w| w.ws)
            .chain(self.monitors.iter().map(|m| m.ws))
            .filter(|&id| id != SPECIAL_WS)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if d > 0 {
            ids.into_iter().find(|&id| id > cur).unwrap_or(cur)
        } else {
            ids.into_iter().rev().find(|&id| id < cur).unwrap_or(cur)
        }
    }

    /// Show the scratchpad workspace over the current one, or go back.
    fn toggle_special(&mut self) {
        let Some(m) = self.monitors.get_mut(self.focused_mon) else { return };
        if m.ws == SPECIAL_WS {
            m.ws = m.prev_ws;
        } else {
            m.prev_ws = m.ws;
            m.ws = SPECIAL_WS;
        }
        self.arrange();
        self.refocus();
    }

    pub fn move_to_workspace(&mut self, n: u32) {
        if n == 0 || n >= SPECIAL_WS {
            return;
        }
        let Some(i) = self.focused_idx() else { return };
        self.wins[i].ws = n;
        self.focus = None;
        self.arrange();
        self.refocus();
    }

    /// Monitor whose center is nearest in direction `dir` from the focused monitor.
    fn monitor_in_dir(&self, dir: Dir) -> Option<usize> {
        let geos: Vec<Rect> = self
            .monitors
            .iter()
            .filter_map(|m| self.space.output_geometry(&m.output))
            .map(to_rect)
            .collect();
        if geos.len() != self.monitors.len() {
            return None;
        }
        neighbor(&geos, self.focused_mon, dir)
    }

    fn focused_idx(&self) -> Option<usize> {
        // while a popup holds the keyboard, actions from it (the window options) apply to the window it was opened over
        let w = self.focus.clone().or_else(|| self.layer_focus.as_ref().and_then(|_| self.last_focus.get(&self.cur_ws()).cloned()));
        w.as_ref().and_then(|w| self.win_idx(w))
    }

    fn ws_windows(&self) -> Vec<usize> {
        let ws = self.cur_ws();
        (0..self.wins.len()).filter(|&i| self.wins[i].ws == ws).collect()
    }

    fn neighbor_of_focus(&self, dir: Dir) -> Option<(usize, usize)> {
        let cur = self.focused_idx()?;
        let idxs = self.ws_windows();
        let rects: Vec<Rect> = idxs.iter().map(|&i| to_rect(self.wins[i].rect)).collect();
        let pos = idxs.iter().position(|&i| i == cur)?;
        neighbor(&rects, pos, dir).map(|n| (cur, idxs[n]))
    }

    pub fn dispatch_action(&mut self, action: &Action) {
        match action {
            Action::Exec(cmd) => self.spawn(cmd),
            Action::Close => {
                if let Some(w) = self.focused_idx().map(|i| self.wins[i].window.clone()) {
                    if let Some(t) = w.toplevel() {
                        t.send_close();
                    } else if let Some(x) = w.x11_surface() {
                        let _ = x.close();
                    }
                }
            }
            Action::ToggleFloating => {
                if let Some(i) = self.focused_idx() {
                    self.wins[i].floating = !self.wins[i].floating;
                    self.wins[i].float_geo = None;
                    self.arrange();
                    if let Some(w) = self.focus.clone() {
                        self.space.raise_element(&w, false);
                    }
                }
            }
            Action::ToggleFullscreen => {
                if let Some(i) = self.focused_idx() {
                    self.wins[i].fullscreen = !self.wins[i].fullscreen;
                    self.arrange();
                }
            }
            Action::FocusDir(d) => {
                if let Some((_, n)) = self.neighbor_of_focus(*d) {
                    let w = self.wins[n].window.clone();
                    self.focus_window(Some(w));
                }
            }
            Action::MoveDir(d) => {
                if let Some((a, b)) = self.neighbor_of_focus(*d) {
                    if !self.wins[a].floating && !self.wins[b].floating {
                        self.wins.swap(a, b);
                        self.arrange();
                    }
                }
            }
            Action::Workspace(n) => self.switch_workspace(*n),
            Action::MoveToWorkspace(n) => self.move_to_workspace(*n),
            Action::WorkspaceExisting(d) => {
                let n = self.existing_workspace(*d);
                self.switch_workspace(n);
            }
            Action::WorkspaceRel(d) => {
                let n = self.rel_workspace(*d);
                self.switch_workspace(n);
            }
            Action::MoveToWorkspaceRel(d) => {
                let n = self.rel_workspace(*d);
                self.move_to_workspace(n);
            }
            Action::ToggleSpecial => self.toggle_special(),
            Action::MoveToSpecial => {
                if let Some(i) = self.focused_idx() {
                    self.wins[i].ws = SPECIAL_WS;
                    self.wins[i].float_geo = None;
                    self.focus = None;
                    self.arrange();
                    self.refocus();
                }
            }
            Action::FocusMonitor(d) => {
                if let Some(mi) = self.monitor_in_dir(*d) {
                    self.focused_mon = mi;
                    self.refocus();
                    // park the pointer on the newly focused monitor
                    if let Some(g) = self.space.output_geometry(&self.monitors[mi].output) {
                        self.pointer.set_location((g.loc.x as f64 + g.size.w as f64 / 2.0, g.loc.y as f64 + g.size.h as f64 / 2.0).into());
                    }
                    self.dirty = true;
                }
            }
            Action::MoveToMonitor(d) => {
                if let (Some(mi), Some(i)) = (self.monitor_in_dir(*d), self.focused_idx()) {
                    self.wins[i].ws = self.monitors[mi].ws;
                    self.wins[i].float_geo = None;
                    self.focus = None;
                    self.arrange();
                    let w = self.wins[i].window.clone();
                    self.focus_window(Some(w));
                }
            }
            Action::CycleLayout => {
                let ws = self.cur_ws();
                let next = match self.layout_of(ws) {
                    LayoutKind::Dwindle => LayoutKind::Master,
                    LayoutKind::Master => LayoutKind::Dwindle,
                };
                self.ws_layout.insert(ws, next);
                self.arrange();
            }
            Action::GrowMaster | Action::ShrinkMaster => {
                let d = if matches!(action, Action::GrowMaster) { 0.05 } else { -0.05 };
                let l = &mut self.config.layout;
                l.master_factor = (l.master_factor + d).clamp(0.1, 0.9);
                l.split_ratio = (l.split_ratio + d).clamp(0.1, 0.9);
                self.arrange();
            }
            Action::ReloadConfig => match oxide_config::Config::load() {
                Ok(c) => self.apply_config(c),
                Err(e) => tracing::warn!("config reload failed: {e}"),
            },
            Action::SendKey(combo) => self.send_key(combo),
            Action::Quit => self.loop_signal.stop(),
        }
    }

    pub fn state_info(&self) -> State {
        // while a popup or launcher holds the keyboard, the window it was opened over still counts as the focused one
        let ctx_focus = if self.focus.is_none() && self.layer_focus.is_some() {
            self.last_focus.get(&self.cur_ws()).filter(|w| self.win_idx(w).is_some()).cloned()
        } else {
            None
        };
        let focus = self.focus.as_ref().or(ctx_focus.as_ref());
        // only workspaces that hold windows or are on a screen exist
        let mut ids: Vec<u32> = self
            .wins
            .iter()
            .map(|w| w.ws)
            .chain(self.monitors.iter().map(|m| m.ws))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let workspaces = ids
            .into_iter()
            .map(|id| WorkspaceInfo {
                id,
                windows: self.wins.iter().filter(|w| w.ws == id).count() as u32,
                visible: self.monitors.iter().any(|m| m.ws == id),
                special: id == SPECIAL_WS,
            })
            .collect();
        let (app_id, title) = focus.map(window_info).unwrap_or_default();
        State {
            active_workspace: self.cur_ws(),
            workspaces,
            layout: match self.layout_of(self.cur_ws()) {
                LayoutKind::Dwindle => "dwindle".into(),
                LayoutKind::Master => "master".into(),
            },
            focused: focus.map(|_| FocusedWindow { app_id, title }),
            windows: self
                .wins
                .iter()
                .map(|w| {
                    let (app_id, title) = window_info(&w.window);
                    let rule = self.config.rules.iter().find(|r| r.title.is_none() && r.app_id.as_ref().is_some_and(|a| a.eq_ignore_ascii_case(&app_id))).cloned();
                    WindowInfo {
                        app_id,
                        title,
                        workspace: w.ws,
                        floating: w.floating,
                        focused: focus == Some(&w.window),
                        fullscreen: w.fullscreen,
                        rule,
                        geometry: {
                            let r = w.cur.rounded();
                            [r.loc.x, r.loc.y, r.size.w, r.size.h]
                        },
                    }
                })
                .collect(),
            outputs: self
                .monitors
                .iter()
                .enumerate()
                .filter_map(|(mi, m)| {
                    let geo = self.space.output_geometry(&m.output)?;
                    let mode = m.output.current_mode()?;
                    Some(OutputInfo {
                        name: m.output.name(),
                        width: geo.size.w,
                        height: geo.size.h,
                        x: geo.loc.x,
                        y: geo.loc.y,
                        scale: m.output.current_scale().fractional_scale(),
                        refresh: mode.refresh as f64 / 1000.0,
                        workspace: m.ws,
                        focused: mi == self.focused_mon,
                    })
                })
                .collect(),
        }
    }
}

/// Render a window's current contents into a texture (physical size = geometry x scale).
fn snapshot_window(
    renderer: &mut smithay::backend::renderer::gles::GlesRenderer,
    window: &Window,
    scale: f64,
) -> Option<smithay::backend::renderer::gles::GlesTexture> {
    use smithay::{
        backend::{
            allocator::Fourcc,
            renderer::{
                element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
                gles::GlesTexture,
                utils::draw_render_elements,
                Bind, Color32F, Frame, Offscreen, Renderer,
            },
        },
        desktop::space::SpaceElement,
        utils::{Physical, Scale, Transform},
    };
    let geo = SpaceElement::geometry(window);
    if geo.size.w <= 0 || geo.size.h <= 0 {
        return None;
    }
    let phys: Size<i32, Physical> = geo.size.to_f64().to_physical(scale).to_i32_ceil();
    let mut tex: GlesTexture =
        Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Abgr8888, (phys.w, phys.h).into()).ok()?;
    let loc: Point<i32, Physical> = Point::<i32, Logical>::from((-geo.loc.x, -geo.loc.y)).to_physical_precise_round(scale);
    let els: Vec<WaylandSurfaceRenderElement<_>> = window.render_elements(renderer, loc, Scale::from(scale), 1.0);
    if els.is_empty() {
        return None;
    }
    {
        let mut fb = renderer.bind(&mut tex).ok()?;
        let mut frame = renderer.render(&mut fb, phys, Transform::Normal).ok()?;
        let full = Rectangle::from_size(phys);
        frame.clear(Color32F::TRANSPARENT, &[full]).ok()?;
        draw_render_elements(&mut frame, scale, &els, &[full]).ok()?;
        let _sync = frame.finish().ok()?;
    }
    Some(tex)
}
