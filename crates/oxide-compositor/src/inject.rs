//! Synthetic input: lets scripts, tests and the AI assistant drive the desktop through `oxide-ctl input`.
use oxide_ipc::InputCmd;
use smithay::{
    backend::input::{Axis, AxisSource, ButtonState, KeyState},
    input::{
        keyboard::{xkb, Keycode, Keysym},
        pointer::AxisFrame,
    },
};

use crate::state::Oxide;

fn button_code(name: &str) -> u32 {
    match name {
        "right" => 0x111,
        "middle" => 0x112,
        _ => 0x110,
    }
}

impl Oxide {
    fn now_ms(&self) -> u32 {
        std::time::Duration::from(self.clock.now()).as_millis() as u32
    }

    /// (keycode, needs shift) producing `sym` on the configured layout.
    fn keycode_for(&self, sym: Keysym) -> Option<(Keycode, bool)> {
        let i = &self.config.input;
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let opts = (!i.kb_options.is_empty()).then(|| i.kb_options.clone());
        let km = xkb::Keymap::new_from_names(&ctx, "", "", &i.kb_layout, &i.kb_variant, opts, xkb::KEYMAP_COMPILE_NO_FLAGS)?;
        for raw in km.min_keycode().raw()..=km.max_keycode().raw() {
            let key = Keycode::new(raw);
            for level in 0..km.num_levels_for_key(key, 0).min(2) {
                if km.key_get_syms_by_level(key, 0, level).contains(&sym) {
                    return Some((key, level == 1));
                }
            }
        }
        None
    }

    fn tap_key(&mut self, name: &str, mods: &[String]) -> Result<(), String> {
        let sym = xkb::keysym_from_name(name, xkb::KEYSYM_CASE_INSENSITIVE);
        if sym.raw() == 0 {
            return Err(format!("unknown key {name:?}"));
        }
        let (key, shift) = self.keycode_for(sym).ok_or_else(|| format!("key {name:?} is not on the keyboard layout"))?;
        let mut held: Vec<Keycode> = vec![];
        let mut want = |n: &str| -> Result<(), String> {
            let msym = match n.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => "Control_L",
                "alt" => "Alt_L",
                "shift" => "Shift_L",
                "super" | "logo" | "win" => "Super_L",
                other => return Err(format!("unknown modifier {other:?}")),
            };
            let (k, _) = self.keycode_for(xkb::keysym_from_name(msym, xkb::KEYSYM_NO_FLAGS)).ok_or("modifier key missing")?;
            held.push(k);
            Ok(())
        };
        for m in mods {
            want(m)?;
        }
        if shift && !mods.iter().any(|m| m.eq_ignore_ascii_case("shift")) {
            want("shift")?;
        }
        let t = self.now_ms();
        for k in &held {
            self.key_event(*k, KeyState::Pressed, t);
        }
        self.key_event(key, KeyState::Pressed, t);
        self.key_event(key, KeyState::Released, t);
        for k in held.iter().rev() {
            self.key_event(*k, KeyState::Released, t);
        }
        Ok(())
    }

    /// The send-key action: press `combo` ("alt+Left", "ctrl+shift+t", "Escape") for the focused app.
    /// The modifiers still held for the bind that triggered this are lifted for the app meanwhile, so
    /// Super+H sending "Left" arrives as a plain Left.
    pub fn send_key(&mut self, combo: &str) {
        let parts: Vec<&str> = combo.split(|c| c == '+' || c == ' ').filter(|p| !p.is_empty()).collect();
        let Some((key, mods)) = parts.split_last() else { return };
        let mods: Vec<String> = mods.iter().map(|m| m.to_string()).collect();
        let kb = self.seat.get_keyboard().unwrap();
        // evdev codes of the modifier keys, as xkb keycodes
        let held: Vec<Keycode> = kb
            .pressed_keys()
            .into_iter()
            .filter(|k| matches!(k.raw().saturating_sub(8), 29 | 97 | 42 | 54 | 56 | 100 | 125 | 126))
            .collect();
        self.bypass_binds = true;
        let t = self.now_ms();
        // apps treat a lone Alt (or Super) tap as "open the menu": a key press in between cancels that
        if !held.is_empty() {
            let f24 = Keycode::new(194 + 8);
            self.key_event(f24, KeyState::Pressed, t);
            self.key_event(f24, KeyState::Released, t);
        }
        for k in &held {
            self.key_event(*k, KeyState::Released, t);
        }
        if let Err(e) = self.tap_key(key, &mods) {
            tracing::warn!("send-key {combo:?}: {e}");
        }
        for k in &held {
            self.key_event(*k, KeyState::Pressed, t);
        }
        self.bypass_binds = false;
    }

    pub fn inject(&mut self, cmd: InputCmd) -> Result<(), String> {
        let t = self.now_ms();
        match cmd {
            InputCmd::Move { x, y } => self.pointer_motion((x, y).into(), t),
            InputCmd::Button { button, pressed } => {
                let state = if pressed { ButtonState::Pressed } else { ButtonState::Released };
                self.button_event(button_code(&button), state, t);
            }
            InputCmd::Click { x, y, button } => {
                self.pointer_motion((x, y).into(), t);
                self.button_event(button_code(&button), ButtonState::Pressed, t);
                self.button_event(button_code(&button), ButtonState::Released, t);
            }
            InputCmd::DoubleClick { x, y } => {
                self.pointer_motion((x, y).into(), t);
                for _ in 0..2 {
                    self.button_event(0x110, ButtonState::Pressed, t);
                    self.button_event(0x110, ButtonState::Released, t);
                }
            }
            InputCmd::Scroll { dx, dy } => {
                let mut frame = AxisFrame::new(t).source(AxisSource::Wheel);
                if dx != 0.0 {
                    frame = frame.value(Axis::Horizontal, dx);
                }
                if dy != 0.0 {
                    frame = frame.value(Axis::Vertical, dy);
                }
                let pointer = self.pointer.clone();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            InputCmd::Key { name, mods } => self.tap_key(&name, &mods)?,
            InputCmd::Type { text } => {
                for ch in text.chars() {
                    let name = match ch {
                        '\n' => "Return".to_string(),
                        '\t' => "Tab".to_string(),
                        ' ' => "space".to_string(),
                        c => xkb::keysym_get_name(xkb::utf32_to_keysym(c as u32)),
                    };
                    self.tap_key(&name, &[])?;
                }
            }
        }
        self.dirty = true;
        Ok(())
    }
}
