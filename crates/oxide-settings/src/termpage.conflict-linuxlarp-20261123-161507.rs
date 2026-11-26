//! Terminal: the standard kitty and fish setup Oxide ships. Out of the box nothing else is needed.
use eframe::egui::{self, vec2, FontId, RichText, Stroke, Ui};
use oxide_config::theme;

use crate::{theme::*, widgets::*, App};

pub fn page(app: &mut App, ui: &mut Ui) {
    page_header(ui, "", "Terminal", "How the standard terminal looks and behaves. Applies to windows you open next.");

    if app.cfg.own.kitty {
        card(ui, "", |ui| {
            row_full(ui, None, "You use your own kitty.conf", "These settings are not used. Switch back in Own configs (Advanced mode).", "", true, |_| {});
        });
    }

    // a small preview in the theme's colors
    let user = std::env::var("USER").unwrap_or_else(|_| "you".into());
    let p = theme::palette(&app.cfg.theme);
    let t = app.cfg.terminal.clone();
    ui.add_space(10.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 118.0), egui::Sense::hover());
    let bg = parse_hex(&p.bg);
    ui.painter().rect(rect, radius(8), bg.gamma_multiply(t.opacity.clamp(0.3, 1.0)), Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
    let font = FontId::monospace((t.font_size * 1.15).clamp(10.0, 24.0));
    let pad = (t.padding as f32).clamp(4.0, 28.0);
    let mut y = rect.top() + pad;
    for (parts, _) in [
        (vec![(user.as_str(), &p.good), ("@", &p.muted), ("oxide", &p.accent_soft), (" ~ ", &p.blue), ("$ ", &p.text), ("ls", &p.blue), (" -la", &p.text)], 0),
        (vec![("drwxr-xr-x  ", &p.muted), ("projects", &p.blue)], 1),
        (vec![("-rw-r--r--  ", &p.muted), ("notes.txt", &p.text)], 2),
        (vec![(user.as_str(), &p.good), ("@", &p.muted), ("oxide", &p.accent_soft), (" ~ ", &p.blue), ("$ ", &p.text)], 3),
    ] {
        let mut x = rect.left() + pad;
        for (s, c) in parts {
            let galley = ui.painter().layout_no_wrap(s.to_string(), font.clone(), parse_hex(c));
            let w = galley.size().x;
            ui.painter().galley(egui::pos2(x, y), galley, parse_hex(c));
            x += w;
        }
        y += font.size * 1.3;
    }
    // the cursor
    let w = font.size * 0.6;
    let cx = rect.left() + pad + font.size * 0.6 * 23.0;
    let cur = parse_hex(&p.accent);
    let _ = match t.cursor.as_str() {
        "block" => ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(cx, y - font.size * 1.3), vec2(w, font.size * 1.2)), 0.0, cur),
        "underline" => ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(cx, y - font.size * 0.2), vec2(w, 2.0)), 0.0, cur),
        _ => ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(cx, y - font.size * 1.3), vec2(2.0, font.size * 1.2)), 0.0, cur),
    };

    card(ui, "Text", |ui| {
        text_row(ui, "Font", "A font name that is installed, like \"JetBrains Mono\". \"monospace\" uses your system's.", false, &mut app.cfg.terminal.font, "monospace");
        slider_row(ui, "Size", "", true, &mut app.cfg.terminal.font_size, 8.0..=24.0, " pt");
    });
    card(ui, "Window", |ui| {
        let mut pct = (app.cfg.terminal.opacity * 100.0).round() as i64;
        slider_row(ui, "Opacity", "Below 100 % the desktop shows through.", false, &mut pct, 40..=100, " %");
        app.cfg.terminal.opacity = pct as f32 / 100.0;
        let mut pad = app.cfg.terminal.padding as i64;
        slider_row(ui, "Padding", "Space between the text and the window edge.", false, &mut pad, 0..=30, " px");
        app.cfg.terminal.padding = pad as u32;
        choice_row(ui, "Cursor", "", true, &mut app.cfg.terminal.cursor, &[("beam".to_string(), "Beam"), ("block".to_string(), "Block"), ("underline".to_string(), "Underline")]);
    });
    card(ui, "Shell", |ui| {
        toggle_row(ui, "Show the welcome message", "The line fish prints when a terminal opens.", false, &mut app.cfg.terminal.greeting);
        toggle_row(ui, "Oxide's prompt", "Pills with the folder, git branch and time, in the theme's colors. Off keeps fish's own prompt.", false, &mut app.cfg.terminal.prompt);
        let mut sb = app.cfg.terminal.scrollback as i64;
        slider_row(ui, "History", "Lines you can scroll back through.", true, &mut sb, 1000..=100_000, "");
        app.cfg.terminal.scrollback = sb as u32;
    });
    ui.add_space(8.0);
    ui.label(RichText::new("The terminal's colors come from the theme. Fish uses the same palette.").color(MUTED()).size(12.0));
}
