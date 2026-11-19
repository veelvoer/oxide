//! The watch app in a square window, for development and screenshots.
use eframe::egui;

struct Root(oxide_remote_watch::app::App);

impl eframe::App for Root {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // the watch's screen is a 225 x 225 point square (a circle in real life): shown in the middle of the window
        let full = ui.max_rect();
        let r = egui::Rect::from_center_size(full.center(), egui::vec2(225.0, 225.0));
        ui.painter().rect_filled(full, 0, egui::Color32::from_gray(40));
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
            ui.set_clip_rect(r);
            self.0.frame(ui);
            // the real screen is round: what is outside the circle is hidden
            let ring = 24.0;
            ui.painter().circle_stroke(r.center(), 112.5 + ring / 2.0 + 0.5, egui::Stroke::new(ring + 2.0, egui::Color32::from_gray(40)));
            ui.painter().circle_stroke(r.center(), 112.5 + 1.0 + 32.0, egui::Stroke::new(64.0, egui::Color32::from_gray(40)));
        });
    }
}

fn main() -> eframe::Result {
    let dir = std::env::var_os("OXIDE_WATCH_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share/oxide-watch-desktop")
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([450.0, 450.0]).with_resizable(false).with_app_id("oxide-watch"),
        ..Default::default()
    };
    eframe::run_native(
        "Oxide Remote Watch",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_zoom_factor(2.0);
            Ok(Box::new(Root(oxide_remote_watch::app::App::new(&cc.egui_ctx, dir))))
        }),
    )
}
