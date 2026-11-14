//! Keyboard shortcuts, pointer routing and focus.
use oxide_config::Action;
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{layer_map_for_output, WindowSurfaceType},
    input::{
        keyboard::{keysyms, FilterResult},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{IsAlive, Logical, Point, SERIAL_COUNTER},
    wayland::{
        pointer_constraints::{with_pointer_constraint, PointerConstraint},
        shell::wlr_layer::Layer,
    },
};

use crate::state::{Oxide, MOD_ALT, MOD_CTRL, MOD_LOGO, MOD_SHIFT};

enum Hit {
    Bind(Action),
    Vt(i32),
    Capture(String, String),
}

impl Oxide {
    pub fn process_input_event<B: InputBackend>(&mut self, event: InputEvent<B>) {
        self.idle_notifier_state.notify_activity(&self.seat);
        match event {
            InputEvent::Keyboard { event } => self.on_key::<B>(event),
            InputEvent::PointerMotionAbsolute { event } => {
                // the nested test window can hold several monitors: the pointer then spans all of them
                let geos: Vec<_> = self.monitors.iter().filter_map(|m| self.space.output_geometry(&m.output)).collect();
                let geo = if self.winit.is_some() && geos.len() > 1 {
                    let x0 = geos.iter().map(|g| g.loc.x).min().unwrap_or(0);
                    let y0 = geos.iter().map(|g| g.loc.y).min().unwrap_or(0);
                    let x1 = geos.iter().map(|g| g.loc.x + g.size.w).max().unwrap_or(0);
                    let y1 = geos.iter().map(|g| g.loc.y + g.size.h).max().unwrap_or(0);
                    smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new((x0, y0).into(), (x1 - x0, y1 - y0).into())
                } else {
                    let Some(out) = self.monitors.get(self.focused_mon).map(|m| m.output.clone()) else { return };
                    let Some(geo) = self.space.output_geometry(&out) else { return };
                    geo
                };
                let pos = event.position_transformed(geo.size) + geo.loc.to_f64();
                self.pointer_motion(pos, Event::time_msec(&event));
            }
            InputEvent::PointerMotion { event } => {
                let cur = self.pointer.current_location();
                let focus = self.surface_under(cur);
                let pointer = self.pointer.clone();
                pointer.relative_motion(
                    self,
                    focus.clone(),
                    &RelativeMotionEvent { delta: event.delta(), delta_unaccel: event.delta_unaccel(), utime: Event::time(&event) },
                );
                let locked = focus.as_ref().is_some_and(|(s, _)| {
                    with_pointer_constraint(s, &pointer, |c| {
                        c.is_some_and(|c| c.is_active() && matches!(&*c, PointerConstraint::Locked(_)))
                    })
                });
                if locked {
                    pointer.frame(self);
                } else {
                    let pos = self.clamp_to_outputs(cur + event.delta());
                    self.pointer_motion(pos, Event::time_msec(&event));
                }
            }
            InputEvent::PointerButton { event } => self.on_button::<B>(event),
            InputEvent::PointerAxis { event } => self.on_axis::<B>(event),
            _ => {}
        }
    }

    fn on_key<B: InputBackend>(&mut self, event: B::KeyboardKeyEvent) {
        self.key_event(event.key_code(), event.state(), Event::time_msec(&event));
    }

    pub fn key_event(&mut self, keycode: smithay::input::keyboard::Keycode, state: KeyState, time: u32) {
        let serial = SERIAL_COUNTER.next_serial();
        let keyboard = self.seat.get_keyboard().unwrap();
        let locked = self.locked();
        if locked {
            self.focus_lock();
        }
        let layer_focused = locked || self.layer_focus.as_ref().is_some_and(|s| s.alive());
        let binds = std::mem::take(&mut self.binds);
        let mut suppressed = std::mem::take(&mut self.suppressed);
        if matches!(self.capture, Some(None)) && std::time::Instant::now() > self.capture_deadline {
            self.capture = None;
        }
        let capturing = matches!(self.capture, Some(None));
        let bypass = self.bypass_binds;

        let action = keyboard.input::<Option<Hit>, _>(self, keycode, state, serial, time, |_, mods, handle| {
            let Some(sym) = handle.raw_latin_sym_or_raw_current_sym() else { return FilterResult::Forward };
            let modified = handle.modified_sym().raw();
            if (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12).contains(&modified) {
                if state == KeyState::Pressed {
                    return FilterResult::Intercept(Some(Hit::Vt((modified - keysyms::KEY_XF86Switch_VT_1 + 1) as i32)));
                }
                return FilterResult::Intercept(None);
            }
            if state == KeyState::Pressed {
                if capturing {
                    let is_mod = [
                        keysyms::KEY_Shift_L, keysyms::KEY_Shift_R, keysyms::KEY_Control_L, keysyms::KEY_Control_R,
                        keysyms::KEY_Alt_L, keysyms::KEY_Alt_R, keysyms::KEY_Super_L, keysyms::KEY_Super_R,
                        keysyms::KEY_Meta_L, keysyms::KEY_Meta_R, keysyms::KEY_Caps_Lock, keysyms::KEY_ISO_Level3_Shift,
                    ]
                    .contains(&sym.raw());
                    if is_mod {
                        return FilterResult::Intercept(None);
                    }
                    let mut parts: Vec<&str> = vec![];
                    if mods.logo {
                        parts.push("SUPER");
                    }
                    if mods.ctrl {
                        parts.push("CTRL");
                    }
                    if mods.alt {
                        parts.push("ALT");
                    }
                    if mods.shift {
                        parts.push("SHIFT");
                    }
                    suppressed.push(sym);
                    return FilterResult::Intercept(Some(Hit::Capture(parts.join("+"), smithay::input::keyboard::xkb::keysym_get_name(sym))));
                }
                if locked || bypass {
                    return FilterResult::Forward;
                }
                let mut m = 0;
                if mods.logo {
                    m |= MOD_LOGO;
                }
                if mods.ctrl {
                    m |= MOD_CTRL;
                }
                if mods.alt {
                    m |= MOD_ALT;
                }
                if mods.shift {
                    m |= MOD_SHIFT;
                }
                // the number row counts by position, so Super+1..9 also work on layouts like AZERTY where the
                // unshifted keys give & é " ' ...
                let row = handle.raw_code().raw();
                let digit = (10..=19).contains(&row).then(|| smithay::input::keyboard::Keysym::from(if row == 19 { keysyms::KEY_0 } else { keysyms::KEY_1 + row - 10 }));
                if let Some(b) = binds.iter().find(|b| b.mods == m && (b.sym == sym || digit == Some(b.sym))) {
                    // popups and launchers keep their own keys, except the key remaps (Alt+J as Left works there too)
                    // (Super shortcuts and the media keys keep working over a popup: no text field wants those)
                    let always = m & MOD_LOGO != 0 || (m == 0 && (0x1008_ff00..=0x1008_ffff).contains(&sym.raw()));
                    if layer_focused && !always && !matches!(b.action, Action::SendKey(_)) {
                        return FilterResult::Forward;
                    }
                    suppressed.push(sym);
                    return FilterResult::Intercept(Some(Hit::Bind(b.action.clone())));
                }
                FilterResult::Forward
            } else if let Some(pos) = suppressed.iter().position(|s| *s == sym) {
                // swallow the release of an intercepted key
                suppressed.swap_remove(pos);
                FilterResult::Intercept(None)
            } else {
                FilterResult::Forward
            }
        });
        self.binds = binds;
        self.suppressed = suppressed;
        if state == KeyState::Released && self.send_repeat.is_some_and(|(k, _)| k == keycode.raw()) {
            self.send_repeat = None;
        }
        match action.flatten() {
            Some(Hit::Bind(action)) => {
                if let Action::SendKey(combo) = &action {
                    self.start_send_repeat(keycode.raw(), combo.clone());
                }
                self.dispatch_action(&action)
            }
            Some(Hit::Vt(n)) => self.change_vt(n),
            Some(Hit::Capture(mods, key)) => {
                // Escape alone cancels
                self.capture = if mods.is_empty() && key == "Escape" { None } else { Some(Some((mods, key))) };
            }
            None => {}
        }
    }

    fn current_mods(&self) -> u8 {
        let m = self.seat.get_keyboard().map(|k| k.modifier_state()).unwrap_or_default();
        (m.logo as u8 * MOD_LOGO) | (m.ctrl as u8 * MOD_CTRL) | (m.alt as u8 * MOD_ALT) | (m.shift as u8 * MOD_SHIFT)
    }

    fn start_drag(&mut self, button: u32, resize: bool) -> bool {
        let pos = self.pointer.current_location();
        let Some(window) = self.space.element_under(pos).map(|(w, _)| w.clone()) else { return false };
        let Some(i) = self.win_idx(&window) else { return false };
        if self.wins[i].fullscreen {
            return false;
        }
        if !self.wins[i].floating {
            self.wins[i].floating = true;
            self.wins[i].float_geo = Some(self.wins[i].rect);
            self.arrange();
        }
        let geo = self.wins[i].float_geo.unwrap_or(self.wins[i].rect);
        self.focus_window(Some(window.clone()));
        self.drag = Some(crate::state::Drag {
            window,
            resize,
            button,
            start_ptr: pos,
            start_geo: geo,
            left: pos.x < (geo.loc.x + geo.size.w / 2) as f64,
            top: pos.y < (geo.loc.y + geo.size.h / 2) as f64,
        });
        true
    }

    fn update_drag(&mut self, pos: Point<f64, Logical>) {
        let Some(d) = &self.drag else { return };
        let (dx, dy) = ((pos.x - d.start_ptr.x).round() as i32, (pos.y - d.start_ptr.y).round() as i32);
        let g = d.start_geo;
        let min = 120;
        let new = if d.resize {
            let (x, w) = if d.left { (g.loc.x + dx.min(g.size.w - min), (g.size.w - dx).max(min)) } else { (g.loc.x, (g.size.w + dx).max(min)) };
            let (y, h) = if d.top { (g.loc.y + dy.min(g.size.h - min), (g.size.h - dy).max(min)) } else { (g.loc.y, (g.size.h + dy).max(min)) };
            smithay::utils::Rectangle::new((x, y).into(), (w, h).into())
        } else {
            smithay::utils::Rectangle::new((g.loc.x + dx, g.loc.y + dy).into(), g.size)
        };
        let window = d.window.clone();
        if let Some(i) = self.win_idx(&window) {
            self.wins[i].float_geo = Some(new);
            self.arrange();
        }
    }

    fn on_button<B: InputBackend>(&mut self, event: B::PointerButtonEvent) {
        self.button_event(event.button_code(), event.state(), Event::time_msec(&event));
    }

    pub fn button_event(&mut self, code: u32, state: ButtonState, time: u32) {
        let serial = SERIAL_COUNTER.next_serial();
        // interactive move/resize
        if let Some(d) = &self.drag {
            if state == ButtonState::Released && d.button == code {
                self.drag = None;
            }
            return;
        }
        if state == ButtonState::Pressed && !self.pointer.is_grabbed() && !self.locked() {
            let mods = self.current_mods();
            let hit = self.mouse_binds.iter().find(|b| b.mods == mods && b.button == code).map(|b| b.action);
            if let Some(action) = hit {
                let resize = matches!(action, oxide_config::MouseAction::ResizeWindow);
                if self.start_drag(code, resize) {
                    return;
                }
            }
        }
        if state == ButtonState::Pressed && !self.pointer.is_grabbed() {
            let pos = self.pointer.current_location();
            self.focus_under(pos);
        }
        let pointer = self.pointer.clone();
        pointer.button(
            self,
            &ButtonEvent { button: code, state, serial, time },
        );
        pointer.frame(self);
    }

    fn on_axis<B: InputBackend>(&mut self, event: B::PointerAxisEvent) {
        let amount = |axis| {
            event.amount(axis).unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.0)
        };
        if self.config.general.scroll_workspaces && self.current_mods() == MOD_LOGO {
            // Super + wheel switches workspace instead of scrolling the client
            self.scroll_accum += amount(Axis::Vertical);
            while self.scroll_accum.abs() >= 15.0 {
                let step = if self.scroll_accum > 0.0 { 1 } else { -1 };
                self.scroll_accum -= 15.0 * step as f64;
                self.dispatch_action(&Action::WorkspaceExisting(step));
            }
            return;
        }
        let mut frame = AxisFrame::new(Event::time_msec(&event)).source(event.source());
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let v = amount(axis);
            if v != 0.0 {
                frame = frame.relative_direction(axis, event.relative_direction(axis)).value(axis, v);
                if let Some(d) = event.amount_v120(axis) {
                    frame = frame.v120(axis, d as i32);
                }
            }
            if event.source() == AxisSource::Finger && event.amount(axis) == Some(0.0) {
                frame = frame.stop(axis);
            }
        }
        let pointer = self.pointer.clone();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// Activate the pointer constraint of the focused surface, release the previous one.
    fn update_constraint(&mut self) {
        let focus = self.pointer.current_focus();
        if let Some(prev) = self.constraint_surface.take() {
            if focus.as_ref() != Some(&prev) {
                with_pointer_constraint(&prev, &self.pointer, |c| {
                    if let Some(c) = c {
                        if c.is_active() {
                            c.deactivate();
                        }
                    }
                });
            } else {
                self.constraint_surface = Some(prev);
            }
        }
        if let Some(surface) = focus {
            let has = with_pointer_constraint(&surface, &self.pointer, |c| {
                c.is_some_and(|c| {
                    if !c.is_active() {
                        c.activate();
                    }
                    true
                })
            });
            if has {
                self.constraint_surface = Some(surface);
            }
        }
    }

    /// Keep the pointer inside the union of all outputs.
    pub fn clamp_to_outputs(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        let geos: Vec<_> = self.monitors.iter().filter_map(|m| self.space.output_geometry(&m.output)).collect();
        if geos.is_empty() || geos.iter().any(|g| g.to_f64().contains(pos)) {
            return pos;
        }
        let mut best = pos;
        let mut best_d = f64::MAX;
        for g in geos {
            let x = pos.x.clamp(g.loc.x as f64, (g.loc.x + g.size.w) as f64 - 1.0);
            let y = pos.y.clamp(g.loc.y as f64, (g.loc.y + g.size.h) as f64 - 1.0);
            let d = (x - pos.x).powi(2) + (y - pos.y).powi(2);
            if d < best_d {
                best_d = d;
                best = (x, y).into();
            }
        }
        best
    }

    pub fn pointer_motion(&mut self, pos: Point<f64, Logical>, time: u32) {
        let serial = SERIAL_COUNTER.next_serial();
        if self.drag.is_some() {
            // clients do not see the pointer while a window is dragged
            let pointer = self.pointer.clone();
            pointer.motion(self, None, &MotionEvent { location: pos, serial, time });
            pointer.frame(self);
            self.update_drag(pos);
            self.dirty = true;
            return;
        }
        let under = self.surface_under(pos);
        let pointer = self.pointer.clone();
        pointer.motion(self, under, &MotionEvent { location: pos, serial, time });
        pointer.frame(self);
        self.dirty = true;
        self.update_constraint();

        // the monitor under the pointer is the active one (new windows open there)
        if !pointer.is_grabbed() && self.layer_focus.is_none() && !self.locked() {
            let mi = self.monitors.iter().position(|m| self.space.output_geometry(&m.output).is_some_and(|g| g.to_f64().contains(pos)));
            if let Some(mi) = mi.filter(|&mi| mi != self.focused_mon) {
                self.focused_mon = mi;
                self.refocus();
            }
        }

        if self.config.general.focus_follows_mouse && !pointer.is_grabbed() && self.layer_focus.is_none() && !self.locked() {
            if let Some((w, _)) = self.space.element_under(pos).map(|(w, l)| (w.clone(), l)) {
                if self.focus.as_ref() != Some(&w) && self.win_idx(&w).is_some() {
                    self.focus_window(Some(w));
                }
            }
        }
    }

    /// Give keyboard focus to whatever is under `pos` (click to focus).
    fn focus_under(&mut self, pos: Point<f64, Logical>) {
        for m in &self.monitors {
            let Some(geo) = self.space.output_geometry(&m.output) else { continue };
            if !geo.to_f64().contains(pos) {
                continue;
            }
            let layers = layer_map_for_output(&m.output);
            let local = pos - geo.loc.to_f64();
            // the layer that really takes input at this point (a popup's empty input region lets the click through)
            let fullscreen = self.fullscreen_on(&m.output).is_some();
            let hit = [Layer::Overlay, Layer::Top].into_iter().filter(|l| !(fullscreen && *l == Layer::Top)).find_map(|layer| {
                layers.layers_on(layer).rev().find(|l| {
                    layers.layer_geometry(l).is_some_and(|g| {
                        g.to_f64().contains(local) && l.surface_under(local - g.loc.to_f64(), WindowSurfaceType::ALL).is_some()
                    })
                })
            });
            if let Some(l) = hit {
                if l.can_receive_keyboard_focus() {
                    let s = l.wl_surface().clone();
                    drop(layers);
                    self.focus_layer(s);
                    return;
                }
                return;
            }
        }
        if let Some(w) = self.space.element_under(pos).map(|(w, _)| w.clone()) {
            if self.win_idx(&w).is_some() {
                self.focus_window(Some(w));
            }
        }
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let m = self.monitors.iter().find(|m| {
            self.space.output_geometry(&m.output).is_some_and(|g| g.to_f64().contains(pos))
        })?;
        let ogeo = self.space.output_geometry(&m.output)?;
        if self.locked() {
            let ls = self.lock_surface_for(&m.output)?;
            return Some((ls.wl_surface().clone(), ogeo.loc.to_f64()));
        }
        let layers = layer_map_for_output(&m.output);
        let local = pos - ogeo.loc.to_f64();

        // topmost layer that really has a surface under the pointer: popups that are hidden stay mapped with an
        // empty input region and must not hide the visible ones below them
        let fullscreen = self.fullscreen_on(&m.output).is_some();
        let layer_hit = |layer: Layer| {
            if fullscreen && layer == Layer::Top {
                return None;
            }
            layers.layers_on(layer).rev().find_map(|l| {
                let lgeo = layers.layer_geometry(l)?;
                if !lgeo.to_f64().contains(local) {
                    return None;
                }
                l.surface_under(local - lgeo.loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, loc)| (s, (loc + lgeo.loc + ogeo.loc).to_f64()))
            })
        };

        if let Some(hit) = layer_hit(Layer::Overlay).or_else(|| layer_hit(Layer::Top)) {
            return Some(hit);
        }
        if let Some((w, loc)) = self.space.element_under(pos) {
            if let Some((s, sloc)) = w.surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL) {
                return Some((s, (sloc + loc).to_f64()));
            }
        }
        layer_hit(Layer::Bottom).or_else(|| layer_hit(Layer::Background))
    }
}

impl Oxide {
    /// A send-key bind (like Alt+J sending Left) keeps sending while the key is held, with the keyboard's repeat settings.
    fn start_send_repeat(&mut self, code: u32, combo: String) {
        use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
        self.repeat_gen += 1;
        let gen = self.repeat_gen;
        self.send_repeat = Some((code, gen));
        let delay = std::time::Duration::from_millis(self.config.input.repeat_delay.max(100) as u64);
        let period = std::time::Duration::from_millis((1000 / self.config.input.repeat_rate.max(1)) as u64);
        let _ = self.loop_handle.insert_source(Timer::from_duration(delay), move |_, _, state| {
            if state.send_repeat != Some((code, gen)) {
                return TimeoutAction::Drop;
            }
            state.send_key(&combo);
            TimeoutAction::ToDuration(period)
        });
    }
}
