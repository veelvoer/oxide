//! Settings pages. Simple mode shows the essentials, advanced mode shows everything.
use eframe::egui::{self, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui};
use oxide_config::{
    anim::{ease, popin_scale, progress, spring},
    Anim, Animations, Curve, LayoutKind, Monitor, WindowRule,
};

use crate::{system as sys, theme::*, widgets::*, App};

pub fn note(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).small().color(MUTED()));
    ui.add_space(2.0);
}

pub fn appearance(app: &mut App, ui: &mut Ui) {
    page_header(ui, "palette-outline", "Appearance", "Colors, borders and effects");
    let adv = app.advanced;
    let cfg = &mut app.cfg;
    let (g, d) = (&mut cfg.general, &mut cfg.decoration);
    card(ui, "Windows", |ui| {
        slider_row(ui, "Gap between windows", "Space between two tiled windows.", false, &mut g.gaps_in, 0..=50, " px");
        slider_row(ui, "Gap around the screen", "Space between windows and the screen edge.", false, &mut g.gaps_out, 0..=100, " px");
        slider_row(ui, "Border thickness", "Set to 0 to hide borders.", false, &mut g.border_size, 0..=10, " px");
        slider_row(ui, "Rounded corners", "How round window corners are.", true, &mut d.rounding, 0..=40, " px");
    });

    card(ui, "Border colors", |ui| {
        ui.add_space(12.0);
        let cols: Vec<Color32> = g.active_border.iter().map(|c| parse_hex(c)).collect();
        ui.label(RichText::new("Focused window").family(medium()));
        ui.add_space(4.0);
        gradient_strip(ui, &cols, 18.0);
        ui.add_space(8.0);
        let mut remove = None;
        let n = g.active_border.len();
        for i in 0..n {
            let last_color = i + 1 == n && !adv;
            row(ui, &format!("Color {}", i + 1), "", last_color, |ui| {
                if adv && n > 1 && ghost_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
                let c = &mut g.active_border[i];
                ui.add_sized([112.0, CONTROL_H], egui::TextEdit::singleline(c).font(egui::TextStyle::Monospace).vertical_align(egui::Align::Center));
                let mut col = parse_hex(c);
                ui.spacing_mut().interact_size = egui::vec2(CONTROL_H + 8.0, CONTROL_H);
                if ui.color_edit_button_srgba(&mut col).changed() {
                    *c = to_hex(col, true);
                }
            });
        }
        if let Some(i) = remove {
            g.active_border.remove(i);
        }
        if adv {
            if g.active_border.len() < 4 && ghost_button(ui, "Add a color").clicked() {
                let last = g.active_border.last().cloned().unwrap_or_else(|| "#ffffff".into());
                g.active_border.push(last);
            }
            ui.add_space(4.0);
            slider_row(ui, "Gradient angle", "Direction the colors flow around the border.", true, &mut g.border_angle, 0.0..=360.0, " deg");
        } else {
            note(ui, "Turn on Advanced mode to add more colors and change the angle.");
        }
        ui.add_space(6.0);
    });
    card(ui, "", |ui| {
        color_row(ui, "Other windows", "Border of every window you are not using. Ends with two digits for transparency.", true, &mut g.inactive_border, true);
    });

    card(ui, "Transparency", |ui| {
        slider_row(ui, "Focused window opacity", "1.0 is solid, lower is see-through.", false, &mut d.active_opacity, 0.1..=1.0, "");
        slider_row(ui, "Other windows opacity", "", !adv, &mut d.inactive_opacity, 0.1..=1.0, "");
        if adv {
            slider_row(ui, "Dim unfocused windows", "Darkens windows you are not using.", true, &mut d.dim_inactive, 0.0..=0.8, "");
        }
    });
    card(ui, "Effects", |ui| {
        row_sub(ui, "speedometer", "Borders show what the app is doing", "A pulse while something builds, red after an error, warmer with high CPU load.", false, |ui| { switch(ui, &mut d.telemetry); });
        toggle_row(ui, "Blur behind windows", "Frosted glass look for see-through windows. Blurs the wallpaper only.", !(adv && d.blur.enabled), &mut d.blur.enabled);
        if adv && d.blur.enabled {
            slider_row(ui, "Blur strength", "", false, &mut d.blur.size, 1..=20, "");
            slider_row(ui, "Blur quality", "More passes look smoother but cost more power.", true, &mut d.blur.passes, 1..=6, "");
        }
    });
    card(ui, "", |ui| {
        toggle_row(ui, "Shadows", "A soft shadow under every window.", !(adv && d.shadow.enabled), &mut d.shadow.enabled);
        if adv && d.shadow.enabled {
            slider_row(ui, "Shadow size", "", false, &mut d.shadow.range, 1..=60, " px");
            color_row(ui, "Shadow color", "The last two digits set transparency.", true, &mut d.shadow.color, true);
        }
    });
}

pub fn layout(app: &mut App, ui: &mut Ui) {
    page_header(ui, "view-dashboard-outline", "Layout", "Tiling and focus");
    let adv = app.advanced;
    let g = &mut app.cfg.general;
    section_label(ui, "Tiling style");
    layout_cards(ui, &mut g.layout);
    let l = &mut app.cfg.layout;
    card(ui, "", |ui| {
        row_sub(ui, "arrow-expand-horizontal", "Make room for what matters", "A window with a compiler error or a video call grows on its own.", !adv || !l.context_aware, |ui| { switch(ui, &mut l.context_aware); });
        if adv && l.context_aware {
            slider_row(ui, "Size of that window", "", true, &mut l.focus_weight, 0.5..=0.8, "");
        }
    });
    if adv {
        card(ui, "Sizes (advanced)", |ui| {
            slider_row(ui, "Dwindle split", "How much of the space the first window takes (0.5 = half).", false, &mut l.split_ratio, 0.1..=0.9, "");
            slider_row(ui, "Master area", "Width of the big window in master layout.", true, &mut l.master_factor, 0.1..=0.9, "");
        });
    }
    let g = &mut app.cfg.general;
    card(ui, "Windows and focus", |ui| {
        toggle_row(ui, "Focus follows mouse", "Hovering a window focuses it.", false, &mut g.focus_follows_mouse);
        toggle_row(ui, "Scroll to change workspace", "Hold Super and use the mouse wheel.", !adv, &mut g.scroll_workspaces);
        if adv {
            slider_row(ui, "Floating window size", "Size of new floating windows, as a share of the screen.", true, &mut g.float_size, 0.2..=1.0, "");
        }
    });
    card(ui, "Workspaces", |ui| {
        row(ui, "Unlimited workspaces", "Go to any number with Super+Number or Super+Shift+L/J. A workspace exists only while it has windows or you are on it.", true, |ui| {
            tag(ui, "always on", GOOD());
        });
    });
}

// ------------------------------------------------------------------ animations

fn preset_smooth() -> Animations {
    Animations {
        windows: Anim::new(4.5, "ease", ""),
        windows_in: Anim::new(4.0, "ease", "popin 92%"),
        windows_out: Anim::new(2.5, "ease", "popin 92%"),
        workspaces: Anim::new(4.5, "ease", "slide"),
        ..Animations::default()
    }
}

fn preset_snappy() -> Animations {
    Animations {
        windows: Anim::new(2.0, "snap", ""),
        windows_in: Anim::new(2.0, "snap", "popin 95%"),
        windows_out: Anim::new(1.5, "snap", "popin 95%"),
        fade_in: Anim::new(1.5, "snap", ""),
        fade_out: Anim::new(1.2, "snap", ""),
        workspaces: Anim::new(2.5, "snap", "slide"),
        border: Anim::new(2.0, "snap", ""),
        ..Animations::default()
    }
}

fn preset_bouncy() -> Animations {
    Animations {
        windows: Anim::new(4.0, "bounce", ""),
        windows_in: Anim::new(4.0, "bounce", "popin 80%"),
        windows_out: Anim::new(2.2, "spring", "popin 80%"),
        workspaces: Anim::new(4.5, "bounce", "slide"),
        ..Animations::default()
    }
}

fn presets() -> Vec<(&'static str, &'static str, Animations)> {
    vec![
        ("Springy", "Soft spring motion with a tiny overshoot. The default.", Animations::default()),
        ("Smooth", "Gentle easing, no overshoot.", preset_smooth()),
        ("Snappy", "Quick and direct.", preset_snappy()),
        ("Bouncy", "Playful and springy.", preset_bouncy()),
        ("Expressive", "Material 3 Expressive springs: lively moves, calm fades.", Animations::expressive()),
        ("Off", "No animations at all.", Animations { enabled: false, ..Animations::default() }),
    ]
}

/// Small plot of a curve's response over time.
fn curve_plot(ui: &mut Ui, c: &Curve, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, radius(8), DEEP());
    let inner = rect.shrink2(vec2(12.0, 14.0));
    let (lo, hi) = (-0.25f32, 1.5f32);
    let map = |x: f32, y: f32| Pos2::new(inner.left() + x * inner.width(), inner.bottom() - ((y - lo) / (hi - lo)) * inner.height());
    for y in [0.0, 1.0] {
        p.line_segment([map(0.0, y), map(1.0, y)], Stroke::new(1.0, BORDER()));
    }
    let samples = 80;
    let settle = if c.is_spring() { spring(c.mass, c.stiffness, c.dampening, 0.0).1 as f32 } else { 1.0 };
    let value = |t: f32| -> f32 {
        if c.is_spring() {
            spring(c.mass, c.stiffness, c.dampening, (t * settle) as f64).0 as f32
        } else {
            ease([c.x1, c.y1, c.x2, c.y2], t as f64) as f32
        }
    };
    let mut prev = map(0.0, value(0.0));
    for i in 1..=samples {
        let t = i as f32 / samples as f32;
        let pt = map(t, value(t).clamp(lo, hi));
        p.line_segment([prev, pt], Stroke::new(2.0, ACCENT()));
        prev = pt;
    }
}

/// Live demo of the "move" and "open" animations as configured.
fn animation_preview(ui: &mut Ui, a: &Animations) {
    let time = ui.input(|i| i.time);
    ui.ctx().request_repaint();
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 14.0) / 2.0;
        // moving
        let (track, _) = ui.allocate_exact_size(vec2(w, 120.0), Sense::hover());
        let p = ui.painter_at(track);
        p.rect_filled(track, radius(10), DEEP());
        let t = time % 4.0;
        let (seg, forward) = if t < 0.3 { (0.0, true) } else if t < 1.9 { (t - 0.3, true) } else if t < 2.4 { (1.6, true) } else if t < 3.9 { (t - 2.4, false) } else { (1.5, false) };
        let (pr, _) = progress(a, &a.windows, seg);
        let pr = if !a.enabled || !a.windows.enabled { if forward { (t >= 0.3) as u8 as f64 } else { 0.0 } } else if forward { if t < 0.3 { 0.0 } else { pr } } else { 1.0 - pr };
        let pr = if !forward && t < 2.4 { 1.0 } else { pr };
        let bw = 70.0;
        let x = track.left() + 16.0 + (track.width() - 32.0 - bw) * pr as f32;
        let r = Rect::from_min_size(Pos2::new(x, track.center().y - 34.0), vec2(bw, 68.0));
        p.rect_filled(r, radius(12), ACCENT().linear_multiply(0.85));
        p.text(track.left_top() + vec2(10.0, 8.0), egui::Align2::LEFT_TOP, "Moving", egui::FontId::proportional(12.0), MUTED());

        // opening
        ui.add_space(14.0);
        let (stage, _) = ui.allocate_exact_size(vec2(w, 120.0), Sense::hover());
        let p = ui.painter_at(stage);
        p.rect_filled(stage, radius(10), DEEP());
        let e = time % 2.2;
        let (sp, _) = progress(a, &a.windows_in, e);
        let (fp, _) = progress(a, &a.fade_in, e);
        let s0 = popin_scale(&a.windows_in.style).filter(|_| a.enabled && a.windows_in.enabled).unwrap_or(1.0);
        let scale = if a.enabled && a.windows_in.enabled { s0 + (1.0 - s0) * sp } else { 1.0 };
        let alpha = if a.enabled && a.fade_in.enabled { fp.clamp(0.0, 1.0) as f32 } else { 1.0 };
        let size = vec2(110.0, 72.0) * scale as f32;
        let r = Rect::from_center_size(stage.center() + vec2(0.0, 6.0), size);
        p.rect_filled(r, radius(12), ACCENT().linear_multiply(0.85 * alpha));
        p.text(stage.left_top() + vec2(10.0, 8.0), egui::Align2::LEFT_TOP, "Opening", egui::FontId::proportional(12.0), MUTED());
    });
}

fn leaf_editor(ui: &mut Ui, title: &str, help: &str, anim: &mut Anim, curves: &[Curve], styles: &[&str], salt: &str) {
    card(ui, "", |ui| {
        toggle_row(ui, title, help, !anim.enabled, &mut anim.enabled);
        if !anim.enabled {
            return;
        }
        let spring_curve = curves.iter().find(|c| c.name == anim.curve).is_some_and(|c| c.is_spring());
        row(ui, "Curve", "How the motion accelerates and settles.", !spring_curve && styles.is_empty() && false, |ui| {
            egui::ComboBox::from_id_salt((salt, "curve")).selected_text(anim.curve.clone()).width(150.0).show_ui(ui, |ui| {
                for c in curves {
                    ui.selectable_value(&mut anim.curve, c.name.clone(), format!("{} ({})", c.name, c.kind));
                }
            });
        });
        if !spring_curve {
            slider_row(ui, "Duration", "Speed x 100 ms. Springs decide their own duration.", styles.is_empty(), &mut anim.speed, 0.5..=20.0, "");
        }
        if !styles.is_empty() {
            row(ui, "Style", "", true, |ui| {
                egui::ComboBox::from_id_salt((salt, "style")).selected_text(if anim.style.is_empty() { "none" } else { &anim.style }).width(150.0).show_ui(ui, |ui| {
                    for s in styles {
                        let label = if s.is_empty() { "none" } else { s };
                        ui.selectable_value(&mut anim.style, s.to_string(), label);
                    }
                });
            });
        }
    });
}

pub fn animations(app: &mut App, ui: &mut Ui) {
    page_header(ui, "animation-play-outline", "Animations", "Motion and easing");
    let adv = app.advanced;
    let a = &mut app.cfg.animations;
    card(ui, "", |ui| {
        toggle_row(ui, "Animate things", "Turn everything off for a snappier, calmer desktop.", true, &mut a.enabled);
    });
    if !a.enabled {
        return;
    }
    card(ui, "Feel", |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            egui::Frame::new().fill(DEEP()).stroke(Stroke::new(1.0, BORDER())).corner_radius(radius(8)).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (name, help, preset) in presets() {
                    if name == "Off" {
                        continue;
                    }
                    let on = preset.curves == a.curves && preset.windows == a.windows && preset.windows_in == a.windows_in && preset.windows_out == a.windows_out && preset.workspaces == a.workspaces && preset.border == a.border && preset.fade_in == a.fade_in && preset.fade_out == a.fade_out;
                    let btn = egui::Button::new(RichText::new(name).color(if on { ON_ACCENT() } else { MUTED() }))
                        .fill(if on { ACCENT() } else { egui::Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(radius(8));
                    if ui.add(btn).on_hover_text(help).clicked() {
                        *a = preset;
                    }
                }
            });
        });
        note(ui, "Presets change every animation at once. Fine tune them in Advanced mode.");
        ui.add_space(6.0);
        animation_preview(ui, a);
        ui.add_space(12.0);
    });

    if !adv {
        return;
    }
    let curves = a.curves.clone();
    ui.add_space(6.0);
    ui.label(RichText::new("Every animation (advanced)").heading().size(18.0));
    let open_styles = ["", "gnomed", "popin 95%", "popin 90%", "popin 80%", "popin 60%"];
    leaf_editor(ui, "Window opens", "A new window appears.", &mut a.windows_in, &curves, &open_styles, "win_in");
    leaf_editor(ui, "Window closes", "A window is closed.", &mut a.windows_out, &curves, &open_styles, "win_out");
    leaf_editor(ui, "Window moves or resizes", "Layout changes, floating drags.", &mut a.windows, &curves, &[], "win_move");
    leaf_editor(ui, "Fade in", "Opacity while a window opens.", &mut a.fade_in, &curves, &[], "fade_in");
    leaf_editor(ui, "Fade out", "Opacity while a window closes.", &mut a.fade_out, &curves, &[], "fade_out");
    leaf_editor(ui, "Switch workspace", "Windows slide away and in.", &mut a.workspaces, &curves, &["slide", ""], "ws");
    leaf_editor(ui, "Border color", "Border changes when focus moves.", &mut a.border, &curves, &[], "border");

    ui.add_space(6.0);
    ui.label(RichText::new("Curves (advanced)").heading().size(18.0));
    note(ui, "A curve is a recipe for motion. Bezier curves have a fixed duration; springs bounce and settle by physics.");
    let mut remove = None;
    for (i, c) in a.curves.iter_mut().enumerate() {
        card(ui, "", |ui| {
            row(ui, &c.name, "", false, |ui| {
                if a_len_is_many() && ghost_button(ui, "Delete").clicked() {
                    remove = Some(i);
                }
                egui::ComboBox::from_id_salt(("ckind", i)).selected_text(c.kind.clone()).width(100.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.kind, "bezier".to_string(), "bezier");
                    ui.selectable_value(&mut c.kind, "spring".to_string(), "spring");
                });
            });
            ui.horizontal(|ui| {
                curve_plot(ui, c, vec2(180.0, 120.0));
                ui.vertical(|ui| {
                    if c.is_spring() {
                        ui.add(egui::Slider::new(&mut c.mass, 0.1..=3.0).text("mass"));
                        ui.add(egui::Slider::new(&mut c.stiffness, 20.0..=400.0).text("stiffness"));
                        ui.add(egui::Slider::new(&mut c.dampening, 1.0..=60.0).text("dampening"));
                    } else {
                        ui.add(egui::Slider::new(&mut c.x1, 0.0..=1.0).text("x1"));
                        ui.add(egui::Slider::new(&mut c.y1, -0.5..=1.5).text("y1"));
                        ui.add(egui::Slider::new(&mut c.x2, 0.0..=1.0).text("x2"));
                        ui.add(egui::Slider::new(&mut c.y2, -0.5..=1.5).text("y2"));
                    }
                });
            });
            ui.add_space(10.0);
        });
    }
    if let Some(i) = remove {
        a.curves.remove(i);
    }
    ui.add_space(8.0);
    if ghost_button(ui, "Add a curve").clicked() {
        let n = a.curves.len() + 1;
        a.curves.push(Curve { name: format!("custom{n}"), ..Curve::default() });
    }
}

fn a_len_is_many() -> bool {
    true
}

pub fn input(app: &mut App, ui: &mut Ui) {
    page_header(ui, "keyboard-outline", "Input", "Keyboard, mouse and touchpad");
    let adv = app.advanced;
    let i = &mut app.cfg.input;
    card(ui, "Keyboard", |ui| {
        text_row(ui, "Layout", "Language code(s), e.g. us or be. Separate several with commas.", false, &mut i.kb_layout, "us");
        slider_row(ui, "Key repeat speed", "Characters per second while holding a key.", !adv, &mut i.repeat_rate, 1..=100, "");
        if adv {
            slider_row(ui, "Repeat delay", "Wait before a held key starts repeating.", false, &mut i.repeat_delay, 100..=1500, " ms");
            text_row(ui, "Variant", "Optional, e.g. intl or dvorak.", false, &mut i.kb_variant, "");
            text_row(ui, "Extra options", "For example caps:escape makes Caps Lock act as Escape.", true, &mut i.kb_options, "");
        }
    });
    card(ui, "Touchpad", |ui| {
        toggle_row(ui, "Natural scrolling", "Content follows your fingers, like a phone. Only for the touchpad: the mouse has its own switch below.", false, &mut i.natural_scroll);
        toggle_row(ui, "Tap to click", "", false, &mut i.tap_to_click);
        choice_row(
            ui,
            "Swipe between workspaces",
            "Swipe sideways with this many fingers to go to the next or previous workspace.",
            false,
            &mut i.swipe_fingers,
            &[(0u32, "Off"), (2, "2"), (3, "3"), (4, "4")],
        );
        if i.swipe_fingers > 0 {
            toggle_row(ui, "Reverse the swipe", "Swiping left goes to the previous workspace instead of the next.", false, &mut i.swipe_invert);
            slider_row(ui, "Swipe distance", "How far the fingers travel before the workspace changes. Smaller is more sensitive.", !adv, &mut i.swipe_distance, 20.0..=200.0, "");
        }
        if adv {
            toggle_row(ui, "Pause while typing", "Ignore the touchpad while you type.", true, &mut i.disable_while_typing);
        }
    });
    card(ui, "Mouse", |ui| {
        toggle_row(ui, "Natural scrolling", "The wheel scrolls the content the way a touchpad does. Off is the classic wheel.", true, &mut i.mouse_natural_scroll);
    });
    if adv {
        card(ui, "Pointer (advanced)", |ui| {
            choice_row(
                ui,
                "Acceleration",
                "Adaptive speeds up with faster movement. Flat is the same at any speed.",
                false,
                &mut i.accel_profile,
                &[("adaptive".to_string(), "Adaptive"), ("flat".to_string(), "Flat")],
            );
            slider_row(ui, "Pointer speed", "-1 is slowest, 1 is fastest, 0 is normal.", true, &mut i.sensitivity, -1.0..=1.0, "");
        });
    }
}

pub fn displays(app: &mut App, ui: &mut Ui) {
    page_header(ui, "monitor", "Displays", "Brightness and screens");
    let adv = app.advanced;
    let ctx = app.ctx.clone();

    if let Some(b) = app.power.value.as_ref().and_then(|p| p.brightness) {
        card(ui, "Brightness", |ui| {
            let mut pct = b * 100.0;
            if let Some(v) = commit_slider_row(ui, "Built-in screen", "", true, &mut pct, 1.0..=100.0, "%") {
                app.jobs.spawn(&ctx, "Brightness", move || sys::run("brightnessctl", &["set", &format!("{}%", v.round())]).map(|_| String::new()));
                if let Some(p) = app.power.value.as_mut() {
                    p.brightness = Some(v / 100.0);
                }
            }
        });
    }

    let detected = app.outputs.clone();
    card(ui, "Connected screens", |ui| {
        if detected.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new("Oxide is not running, so screens cannot be detected.").color(MUTED()));
            ui.add_space(14.0);
        }
        for (n, o) in detected.iter().enumerate() {
            let configured = app.cfg.monitors.iter().any(|m| m.name == o.name);
            row_sub(
                ui,
                "monitor",
                &o.name,
                &format!("{} x {}  -  {:.0} Hz  -  scale {}", o.width, o.height, o.refresh, o.scale),
                n + 1 == detected.len(),
                |ui| {
                    if !configured && primary_button(ui, "Customize").clicked() {
                        app.cfg.monitors.push(Monitor { name: o.name.clone(), width: o.width, height: o.height, refresh: Some(o.refresh), x: None, y: None, scale: o.scale });
                    }
                    if configured {
                        tag(ui, "customized", GOOD());
                    }
                },
            );
        }
    });

    let mut remove = None;
    for (idx, m) in app.cfg.monitors.iter_mut().enumerate() {
        let connected = detected.iter().any(|o| o.name == m.name);
        card(ui, &format!("Screen {}", m.name), |ui| {
            if !connected {
                row_sub(ui, "monitor", "Not connected right now", "Settings are kept for when it is plugged in.", false, |_| {});
            }
            slider_row(ui, "Scale", "Make everything bigger on high resolution screens.", !adv, &mut m.scale, 0.5..=4.0, "x");
            if adv {
                text_row(ui, "Name", "As shown above, e.g. eDP-1 or HDMI-A-1.", false, &mut m.name, "eDP-1");
                slider_row(ui, "Width", "", false, &mut m.width, 320..=7680, " px");
                slider_row(ui, "Height", "", false, &mut m.height, 240..=4320, " px");
                row(ui, "Refresh rate", "0 uses the screen's preferred rate.", false, |ui| {
                    let mut r = m.refresh.unwrap_or(0.0);
                    ui.add(egui::DragValue::new(&mut r).range(0.0..=480.0).suffix(" Hz"));
                    m.refresh = (r > 0.0).then_some(r);
                });
                row(ui, "Position", "Where the top-left corner sits. Automatic places it to the right of the others.", false, |ui| {
                    let mut auto = m.x.is_none() && m.y.is_none();
                    if switch(ui, &mut auto).changed() {
                        if auto {
                            m.x = None;
                            m.y = None;
                        } else {
                            m.x = Some(0);
                            m.y = Some(0);
                        }
                    }
                    ui.label("automatic");
                    if !auto {
                        let mut y = m.y.unwrap_or(0);
                        let mut x = m.x.unwrap_or(0);
                        ui.add(egui::DragValue::new(&mut y).prefix("y "));
                        ui.add(egui::DragValue::new(&mut x).prefix("x "));
                        m.x = Some(x);
                        m.y = Some(y);
                    }
                });
            }
            row(ui, "", "", true, |ui| {
                if ghost_button(ui, "Forget this screen").clicked() {
                    remove = Some(idx);
                }
            });
        });
    }
    if let Some(i) = remove {
        app.cfg.monitors.remove(i);
    }
    if adv {
        ui.add_space(10.0);
        if ghost_button(ui, "Add a screen by hand").clicked() {
            app.cfg.monitors.push(Monitor { name: "HDMI-A-1".into(), width: 1920, height: 1080, refresh: None, x: None, y: None, scale: 1.0 });
        }
        arrangement(app, ui);
    }
}

fn arrangement(app: &mut App, ui: &mut Ui) {
    // positions of screens as placed by Oxide right now (auto positions included)
    let live: Vec<(String, i32, i32, i32, i32)> = app.outputs.iter().map(|o| (o.name.clone(), o.x, o.y, o.width, o.height)).collect();
    if app.cfg.monitors.is_empty() {
        return;
    }
    card(ui, "Arrangement (drag the screens)", |ui| {
        ui.add_space(10.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 220.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, radius(10), DEEP());
        // resolve every screen to a concrete position and size
        let mut boxes: Vec<(usize, i32, i32, i32, i32)> = vec![];
        let mut next_x = 0;
        for (idx, m) in app.cfg.monitors.iter().enumerate() {
            let (w, h) = ((m.width as f64 / m.scale) as i32, (m.height as f64 / m.scale) as i32);
            let auto = live.iter().find(|l| l.0 == m.name).map(|l| (l.1, l.2));
            let x = m.x.or(auto.map(|a| a.0)).unwrap_or(next_x);
            let y = m.y.or(auto.map(|a| a.1)).unwrap_or(0);
            next_x = next_x.max(x + w);
            boxes.push((idx, x, y, w, h));
        }
        let (mut minx, mut miny, mut maxx, mut maxy) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for b in &boxes {
            minx = minx.min(b.1);
            miny = miny.min(b.2);
            maxx = maxx.max(b.1 + b.3);
            maxy = maxy.max(b.2 + b.4);
        }
        let (w, h) = ((maxx - minx).max(1) as f32 + 800.0, (maxy - miny).max(1) as f32 + 800.0);
        let s = (rect.width() / w).min(rect.height() / h);
        let origin = rect.min + vec2(400.0 * s, 400.0 * s);
        for (idx, x, y, bw, bh) in boxes {
            let r = Rect::from_min_size(origin + vec2((x - minx) as f32 * s, (y - miny) as f32 * s), vec2(bw as f32 * s, bh as f32 * s));
            let resp = ui.interact(r, ui.make_persistent_id(("mon_drag", idx)), Sense::drag());
            let m = &mut app.cfg.monitors[idx];
            if resp.dragged() {
                let d = resp.drag_delta() / s;
                m.x = Some(x + d.x.round() as i32);
                m.y = Some(y + d.y.round() as i32);
            }
            if resp.drag_stopped() {
                m.x = m.x.map(|v| (v as f32 / 10.0).round() as i32 * 10);
                m.y = m.y.map(|v| (v as f32 / 10.0).round() as i32 * 10);
            }
            let hot = tween(ui.ctx(), resp.id, if resp.hovered() || resp.dragged() { 1.0 } else { 0.0 }, 0.12);
            p.rect(r, radius(6), lerp_color(BORDER(), ACCENT().linear_multiply(0.3), hot), Stroke::new(1.5, lerp_color(ACCENT().linear_multiply(0.6), ACCENT(), hot)), egui::StrokeKind::Inside);
            p.text(r.center(), egui::Align2::CENTER_CENTER, &m.name, egui::FontId::proportional(14.0), TEXT());
        }
    });
}

fn tri_state(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, v: &mut Option<bool>) {
    let text = match v {
        None => "leave as is",
        Some(true) => "yes",
        Some(false) => "no",
    };
    egui::ComboBox::from_id_salt(id).selected_text(text).width(110.0).show_ui(ui, |ui| {
        ui.selectable_value(v, None, "leave as is");
        ui.selectable_value(v, Some(true), "yes");
        ui.selectable_value(v, Some(false), "no");
    });
}

fn opt_text(ui: &mut Ui, v: &mut Option<String>, hint: &str) {
    let mut s = v.clone().unwrap_or_default();
    if ui.add(egui::TextEdit::singleline(&mut s).hint_text(hint.to_string()).desired_width(180.0)).changed() {
        *v = (!s.is_empty()).then_some(s);
    }
}

pub fn apps(app: &mut App, ui: &mut Ui) {
    page_header(ui, "apps", "Apps", "Startup, rules and variables");

    card(ui, "Start with Oxide", |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("These commands run every time Oxide starts, for example your bar or wallpaper.").small().color(MUTED()));
        ui.add_space(6.0);
        let mut remove = None;
        for (i, c) in app.cfg.exec_once.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(c).desired_width(ui.available_width() - 90.0).hint_text("command"));
                if ghost_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            app.cfg.exec_once.remove(i);
        }
        ui.add_space(6.0);
        if ghost_button(ui, "Add command").clicked() {
            app.cfg.exec_once.push(String::new());
        }
        ui.add_space(10.0);
    });

    card(ui, "Window rules", |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("When a window opens that matches, Oxide applies these settings to it.").small().color(MUTED()));
        ui.add_space(6.0);
        let mut remove = None;
        for (i, r) in app.cfg.rules.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::Frame::new().fill(SIDEBAR()).corner_radius(radius(10)).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("When app is").color(MUTED()));
                        opt_text(ui, &mut r.app_id, "e.g. firefox");
                        ui.label(RichText::new("or title has").color(MUTED()));
                        opt_text(ui, &mut r.title, "text");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("then  floating").color(MUTED()));
                        tri_state(ui, ("fl", i), &mut r.floating);
                        ui.label(RichText::new("blur").color(MUTED()));
                        tri_state(ui, ("bl", i), &mut r.blur);
                        ui.label(RichText::new("workspace").color(MUTED()));
                        let mut ws = r.workspace.unwrap_or(0);
                        ui.add(egui::DragValue::new(&mut ws).range(0..=99).custom_formatter(|v, _| if v == 0.0 { "any".into() } else { format!("{v}") }));
                        r.workspace = (ws > 0).then_some(ws);
                        ui.label(RichText::new("opacity").color(MUTED()));
                        let mut op = r.opacity.unwrap_or(0.0);
                        ui.add(egui::DragValue::new(&mut op).speed(0.01).range(0.0..=1.0).custom_formatter(|v, _| if v == 0.0 { "default".into() } else { format!("{v:.2}") }));
                        r.opacity = (op > 0.0).then_some(op);
                        if ghost_button(ui, "Remove rule").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
        if let Some(i) = remove {
            app.cfg.rules.remove(i);
        }
        if ghost_button(ui, "Add rule").clicked() {
            app.cfg.rules.push(WindowRule::default());
        }
        ui.add_space(10.0);
    });

    card(ui, "Environment variables", |ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Variables every app started by Oxide receives.").small().color(MUTED()));
        ui.add_space(6.0);
        let mut remove = None;
        for (i, (k, v)) in app.cfg.env.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(k).desired_width(170.0).hint_text("NAME"));
                ui.label("=");
                ui.add(egui::TextEdit::singleline(v).desired_width(ui.available_width() - 90.0).hint_text("value"));
                if ghost_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            app.cfg.env.remove(i);
        }
        ui.add_space(6.0);
        if ghost_button(ui, "Add variable").clicked() {
            app.cfg.env.push((String::new(), String::new()));
        }
        ui.add_space(10.0);
    });

    card(ui, "System", |ui| {
        toggle_row(ui, "Run X11 apps (XWayland)", "Needed for older apps. Takes effect after restarting Oxide.", true, &mut app.cfg.general.xwayland);
    });
}

pub fn config(app: &mut App, ui: &mut Ui) {
    page_header(ui, "cog-outline", "Config", "Updates, import, reset and file");
    updates_card(app, ui);
    let path = oxide_config::Config::path();
    card(ui, "Your config file", |ui| {
        row(ui, "Location", &path.display().to_string(), false, |ui| {
            if ghost_button(ui, "Copy path").clicked() {
                ui.ctx().copy_text(path.display().to_string());
                app.say("Path copied");
            }
        });
        row(ui, "Reload from file", "Use this after editing the file by hand.", true, |ui| {
            if ghost_button(ui, "Reload now").clicked() {
                app.reload_compositor();
            }
        });
    });
    card(ui, "Import", |ui| {
        row(ui, "From Hyprland", "Reads ~/.config/hypr/hyprland.conf and converts what it can. Nothing is saved until you press Save.", true, |ui| {
            if ghost_button(ui, "Import").clicked() {
                let path = format!("{}/.config/hypr/hyprland.conf", std::env::var("HOME").unwrap_or_default());
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        let imp = oxide_config::hyprland::import(&text);
                        let skipped = imp.skipped.len();
                        app.cfg = imp.config;
                        app.say(format!("Imported. {skipped} lines could not be converted."));
                    }
                    Err(_) => app.say("No hyprland.conf found (newer Lua configs cannot be imported)"),
                }
            }
        });
    });
    card(ui, "Reset", |ui| {
        row(ui, "Back to defaults", "Restores every setting. You can still discard this before saving.", true, |ui| {
            if app.confirm_reset {
                if primary_button(ui, "Yes, reset").clicked() {
                    app.cfg = oxide_config::Config::default();
                    app.confirm_reset = false;
                    app.say("Reset to defaults (not saved yet)");
                }
                if ghost_button(ui, "Cancel").clicked() {
                    app.confirm_reset = false;
                }
            } else if ghost_button(ui, "Reset everything").clicked() {
                app.confirm_reset = true;
            }
        });
    });
}

/// The two tiling styles as picture cards: a small drawing of the layout, its name and one line about it.
fn layout_cards(ui: &mut Ui, value: &mut LayoutKind) {
    let gap = 12.0;
    let w = ((ui.available_width() - gap) / 2.0).floor();
    let h = 172.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (kind, name, help) in [
            (LayoutKind::Dwindle, "Dwindle", "Each new window splits the last one in half."),
            (LayoutKind::Master, "Master", "One main window with the rest stacked beside it."),
        ] {
            let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
            if resp.clicked() {
                *value = kind.clone();
            }
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let sel = tween(ui.ctx(), resp.id, (*value == kind) as u8 as f32, 0.18);
            let hov = tween(ui.ctx(), resp.id.with("h"), resp.hovered() as u8 as f32, 0.12);
            let p = ui.painter();
            let r = if expressive() { egui::CornerRadius::same(20) } else { radius(8) };
            p.rect_filled(rect, r, lerp_color(lerp_color(CARD(), CARD_HOVER(), hov), lerp_color(CARD(), ACCENT(), 0.10), sel));
            p.rect_stroke(rect, r, Stroke::new(1.0 + sel, lerp_color(BORDER(), ACCENT(), sel)), egui::StrokeKind::Inside);
            // the drawing
            let pic = Rect::from_min_max(rect.min + vec2(16.0, 16.0), egui::pos2(rect.right() - 16.0, rect.top() + 106.0));
            p.rect_filled(pic, radius(6), DEEP());
            let inner = pic.shrink(7.0);
            let tile = |x0: f32, y0: f32, x1: f32, y1: f32| {
                let t = Rect::from_min_max(
                    egui::pos2(egui::lerp(inner.left()..=inner.right(), x0), egui::lerp(inner.top()..=inner.bottom(), y0)),
                    egui::pos2(egui::lerp(inner.left()..=inner.right(), x1), egui::lerp(inner.top()..=inner.bottom(), y1)),
                )
                .shrink(2.5);
                p.rect_filled(t, radius(3), lerp_color(FIELD(), lerp_color(FIELD(), ACCENT(), 0.25), sel));
                p.rect_stroke(t, radius(3), Stroke::new(1.0, lerp_color(BORDER(), ACCENT(), sel * 0.6)), egui::StrokeKind::Inside);
            };
            tile(0.0, 0.0, 0.5, 1.0);
            if kind == LayoutKind::Dwindle {
                tile(0.5, 0.0, 1.0, 0.5);
                tile(0.5, 0.5, 0.75, 1.0);
                tile(0.75, 0.5, 1.0, 1.0);
            } else {
                for i in 0..3 {
                    tile(0.5, i as f32 / 3.0, 1.0, (i + 1) as f32 / 3.0);
                }
            }
            p.text(egui::pos2(rect.left() + 16.0, rect.top() + 122.0), egui::Align2::LEFT_TOP, name, egui::FontId::new(14.0, crate::theme::medium()), TEXT());
            p.text(egui::pos2(rect.left() + 16.0, rect.top() + 144.0), egui::Align2::LEFT_TOP, help, egui::FontId::proportional(12.5), MUTED());
        }
    });
}

/// The updater's state file (written by `oxide-ctl update`), read at most once a second.
fn update_status() -> serde_json::Value {
    thread_local! {
        static CACHE: std::cell::RefCell<(Option<std::time::Instant>, serde_json::Value)> = Default::default();
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.0.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(1)) {
            let dir = std::env::var_os("XDG_STATE_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
            c.1 = std::fs::read_to_string(dir.join("oxide/update.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            c.0 = Some(std::time::Instant::now());
        }
        c.1.clone()
    })
}

fn ago(secs: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let d = now.saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86400),
    }
}

fn updates_card(app: &mut App, ui: &mut Ui) {
    let st = update_status();
    // "abc1234", or "abc1234-dirty" for a build with local changes
    let short = |v: &serde_json::Value| v.as_str().map(|s| format!("{}{}", s.chars().take(7).collect::<String>(), if s.ends_with("-dirty") { "-dirty" } else { "" }));
    let installed = short(&st["installed"]);
    let latest = short(&st["latest"]);
    let running = st["running"].as_bool() == Some(true);
    card(ui, "Updates", |ui| {
        let sub = match (&installed, &latest) {
            _ if running => "Building the new version in the background. It is used after you log out and back in.".to_string(),
            (Some(i), Some(l)) if i != l && !i.ends_with("dirty") => format!("Version {i} installed, {l} is available."),
            (Some(i), _) if i.ends_with("dirty") => "Built from a copy with local changes: not updated automatically.".to_string(),
            (Some(i), _) => format!("Version {i}{}", st["checked"].as_u64().map(|t| format!(", checked {}", ago(t))).unwrap_or_default()),
            (None, _) => "Version unknown (installed without git).".to_string(),
        };
        row_sub(ui, "rocket-launch-outline", "Oxide", &sub, false, |ui| {
            if running {
                ui.spinner();
            } else {
                let newer = matches!((&installed, &latest), (Some(i), Some(l)) if i != l && !i.ends_with("dirty"));
                let label = if newer { "Update now" } else { "Check now" };
                let clicked = if newer { primary_button(ui, label).clicked() } else { ghost_button(ui, label).clicked() };
                if clicked {
                    let args: &[&str] = if newer { &["update", "run"] } else { &["update", "check"] };
                    if let Ok(mut c) = std::process::Command::new("oxide-ctl").args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() {
                        std::thread::spawn(move || {
                            let _ = c.wait();
                        });
                        app.say(if newer { "Updating in the background" } else { "Checking…" });
                    }
                }
            }
        });
        if let Some(e) = st["error"].as_str() {
            row_full(ui, Some("alert-circle-outline"), "Last attempt failed", e, "", false, |_| {});
        }
        row_sub(ui, "refresh", "Update automatically", "Now and then, at the lowest priority. What changed is shown after the next login.", true, |ui| {
            switch(ui, &mut app.cfg.update.auto);
        });
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
}
