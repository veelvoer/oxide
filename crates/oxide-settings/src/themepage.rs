//! Theme: one palette for the shell, the settings app and the terminal.
use eframe::egui::{self, vec2, Align2, FontId, Rect, RichText, Sense, Stroke, Ui};
use oxide_config::theme;

use crate::{theme::*, widgets::*, App};

fn title(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "", "Theme", "Colors for the bar, launcher, panels and your terminal.");
    let before = app.cfg.theme.clone();

    card(ui, "Appearance", |ui| {
        choice_row(
            ui,
            "Mode",
            "Light or dark for the shell, this app, your terminal and GTK and Qt apps.",
            false,
            &mut app.cfg.theme.mode,
            &[("dark".to_string(), "Dark"), ("light".to_string(), "Light")],
        );
        let mut r = app.cfg.theme.roundness as i64;
        let old = r;
        slider_row(ui, "Roundness", "Corner radius of the bar, panels, windows and this app. 0 is square.", false, &mut r, 0..=20, "");
        if r != old {
            app.cfg.theme.roundness = r as u32;
            // windows follow: 8 matches the default window rounding of 12
            app.cfg.decoration.rounding = (r as f32 * 1.5).round() as i32;
        }
        let mut gaps = app.cfg.general.gaps_in as i64;
        let g_old = gaps;
        slider_row(ui, "Gap between windows", "", true, &mut gaps, 0..=24, " px");
        if gaps != g_old {
            app.cfg.general.gaps_in = gaps as i32;
        }
    });

    card(ui, "Palette", |ui| {
        ui.add_space(12.0);
        let mut presets = if app.cfg.theme.mode == "light" { theme::light_presets() } else { theme::presets() };
        // first: the palette taken from your wallpaper (it follows when the wallpaper changes, also with your own shell)
        presets.insert(0, theme::palette(&oxide_config::theme::ThemeCfg { preset: "auto".into(), mode: app.cfg.theme.mode.clone(), ..Default::default() }));
        let w = ((ui.available_width() - 12.0 * 3.0) / 4.0).floor();
        egui::Grid::new("theme-presets").spacing(vec2(12.0, 12.0)).show(ui, |ui| {
            for (i, p) in presets.iter().enumerate() {
                // the first card is the wallpaper palette (its name is "oxide" until a wallpaper has been read)
                let key: &str = if i == 0 { "auto" } else { &p.name };
                let sel = app.cfg.theme.preset == key;
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 74.0), Sense::click());
                let bg = parse_hex(&p.bg);
                let resp = if i == 0 { resp.on_hover_text("Colors taken from your wallpaper. They change when the wallpaper does.") } else { resp };
                ui.painter().rect(
                    rect,
                    radius(6),
                    bg,
                    Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { ACCENT() } else if resp.hovered() { MUTED() } else { BORDER() }),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(rect.left_top() + vec2(10.0, 10.0), Align2::LEFT_TOP, if i == 0 { "From wallpaper".to_string() } else { title(&p.name) }, FontId::new(13.0, medium()), parse_hex(&p.text));
                if sel {
                    let c = rect.right_top() + vec2(-18.0, 18.0);
                    ui.painter().circle_filled(c, 9.0, ACCENT());
                    let t = Stroke::new(1.8, ON_ACCENT());
                    ui.painter().line_segment([c + vec2(-3.6, 0.2), c + vec2(-1.1, 2.8)], t);
                    ui.painter().line_segment([c + vec2(-1.1, 2.8), c + vec2(3.8, -2.6)], t);
                }
                let colors = [&p.surface, &p.accent, &p.ansi[1], &p.ansi[2], &p.ansi[3], &p.ansi[4], &p.ansi[5], &p.ansi[6]];
                for (k, c) in colors.iter().enumerate() {
                    let r = Rect::from_min_size(rect.left_bottom() + vec2(10.0 + k as f32 * 18.0, -26.0), vec2(14.0, 14.0));
                    ui.painter().rect(r, radius(3), parse_hex(c), Stroke::NONE, egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    app.cfg.theme.preset = key.to_string();
                    app.cfg.theme.accent = None;
                }
                if (i + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(12.0);
    });

    card(ui, "Accent", |ui| {
        let current = theme::palette(&app.cfg.theme).accent;
        let mut custom = app.cfg.theme.accent.is_some();
        let mut hex = app.cfg.theme.accent.clone().unwrap_or(current);
        toggle_row(ui, "Custom accent color", "Replace the palette's accent color.", !custom, &mut custom);
        if custom {
            color_row(ui, "Color", "", true, &mut hex, false);
            app.cfg.theme.accent = Some(hex);
        } else {
            app.cfg.theme.accent = None;
        }
    });

    if app.cfg.default_shell_active() {
        background_card(ui);
    } else {
        card(ui, "Background", |ui| {
            row_full(ui, None, "Wallpaper", "Your own shell draws the wallpaper. Pick \"From wallpaper\" above and the colors follow it.", "", true, |_| {});
        });
    }

    card(ui, "Apps", |ui| {
        toggle_row(
            ui,
            "Terminal follows theme",
            "Writes kitty-theme.conf and includes it from kitty.conf. Open kitty windows update right away.",
            false,
            &mut app.cfg.theme.kitty,
        );
        row_full(ui, None, "Window borders", "Set the focused and unfocused border colors from this palette.", "", true, |ui| {
            if ghost_button(ui, "Match theme").clicked() {
                let p = theme::palette(&app.cfg.theme);
                app.cfg.general.active_border = p.border.clone();
                app.cfg.general.inactive_border = p.border_inactive.clone();
            }
        });
    });

    let dragging = ui.input(|i| i.pointer.any_down());
    if app.cfg.theme != before {
        app.theme_pending = true;
        let t = &app.cfg.theme;
        if t.preset != before.preset || t.mode != before.mode || t.accent != before.accent {
            // a new look carries the window borders along
            let p = theme::palette(t);
            app.cfg.general.active_border = p.border;
            app.cfg.general.inactive_border = p.border_inactive;
        }
    }
    if app.theme_pending && !dragging {
        app.theme_pending = false;
        if let Err(e) = theme::write_all(&app.cfg.theme) {
            app.say(format!("Could not write theme files: {e}"));
        }
    }
    ui.add_space(8.0);
    ui.label(RichText::new("The shell reads the theme live. Other apps keep their own themes.").color(MUTED()).size(12.0));
}

fn wallpaper_path() -> std::path::PathBuf {
    oxide_config::Config::path().with_file_name("wallpaper.json")
}

/// Desktop background: one of the bundled photos, the themed logo, your own picture or one color.
/// Written to wallpaper.json, which the shell watches.
fn background_card(ui: &mut Ui) {
    // (only reached when the bundled shell runs)
    let mut v: serde_json::Value = std::fs::read_to_string(wallpaper_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    let before = v.clone();
    card(ui, "Background", |ui| {
        let mut mode = v["mode"].as_str().unwrap_or("photo").to_string();
        if mode == "scene1" || mode == "scene2" {
            mode = "photo".into();
        }
        choice_row(
            ui,
            "Desktop background",
            "Photos are real pictures. Logo is the Oxide tiles in your palette.",
            mode == "photo",
            &mut mode,
            &[("photo".to_string(), "Photo"), ("theme".to_string(), "Logo"), ("image".to_string(), "Picture"), ("color".to_string(), "Color")],
        );
        v["mode"] = serde_json::json!(mode);
        if mode == "photo" {
            let mut cur = v["photo"].as_str().unwrap_or("dolomites").to_string();
            photo_grid(ui, &mut cur);
            v["photo"] = serde_json::json!(cur);
        } else if mode == "image" {
            let mut path = v["path"].as_str().unwrap_or("").to_string();
            text_row(ui, "Picture", "Full path to a jpg or png.", false, &mut path, "/home/you/Pictures/wall.jpg");
            v["path"] = serde_json::json!(path);
            let mut fit = v["fit"].as_str().unwrap_or("fill").to_string();
            choice_row(ui, "Fit", "", true, &mut fit, &[("fill".to_string(), "Fill"), ("fit".to_string(), "Fit"), ("center".to_string(), "Center")]);
            v["fit"] = serde_json::json!(fit);
        } else if mode == "color" {
            let mut c = v["color"].as_str().unwrap_or("#1b1917").to_string();
            color_row(ui, "Color", "", true, &mut c, false);
            v["color"] = serde_json::json!(c);
        }
    });
    if v != before {
        let _ = std::fs::write(wallpaper_path(), serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}

/// Thumbnails of the photos; the chosen one gets the accent outline.
fn photo_grid(ui: &mut Ui, current: &mut String) {
    let names = oxide_config::wallpaper_names();
    if names.is_empty() {
        crate::pages::note(ui, "No photos found. Run the installer again, or drop jpg files into ~/.local/share/oxide/wallpapers.");
        return;
    }
    let dirs = oxide_config::wallpaper_dirs();
    let find = |n: &str, sub: &str| -> Option<std::path::PathBuf> {
        for d in &dirs {
            for ext in ["jpg", "jpeg", "png"] {
                let p = d.join(sub).join(format!("{n}.{ext}"));
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    };
    ui.add_space(6.0);
    let cols = 4usize;
    let gap = 10.0;
    let w = ((ui.available_width() - 28.0 - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    let h = (w * 9.0 / 16.0).round();
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            for row in names.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for n in row {
                        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
                        let selected = n == current;
                        let hov = tween(ui.ctx(), resp.id.with("hov"), if resp.hovered() { 1.0 } else { 0.0 }, 0.12);
                        let sel = tween(ui.ctx(), resp.id.with("sel"), if selected { 1.0 } else { 0.0 }, 0.16);
                        let uri = find(n, "thumbs").or_else(|| find(n, "")).map(|p| format!("file://{}", p.display()));
                        let r = radius(6);
                        if let Some(uri) = uri {
                            egui::Image::new(uri).fit_to_exact_size(rect.size()).corner_radius(r).paint_at(ui, rect);
                        } else {
                            ui.painter().rect_filled(rect, r, FIELD());
                        }
                        ui.painter().rect_filled(rect, r, egui::Color32::from_black_alpha((60.0 * (1.0 - hov) * (1.0 - sel)) as u8));
                        ui.painter().rect_stroke(rect, r, Stroke::new(1.0 + 1.5 * sel, lerp_color(BORDER(), ACCENT(), sel)), egui::StrokeKind::Inside);
                        let label = title(&n.replace('-', " "));
                        ui.painter().text(rect.left_bottom() + vec2(8.0, -7.0), Align2::LEFT_BOTTOM, label, FontId::proportional(12.0), egui::Color32::WHITE);
                        if resp.clicked() {
                            *current = n.clone();
                        }
                        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                });
                ui.add_space(gap);
            }
        });
    });
}
