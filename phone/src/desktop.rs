//! The same app in a desktop window, for development and screenshots.
use eframe::egui;

struct Root(oxide_remote::app::App);

impl eframe::App for Root {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.frame(ui);
    }
}

fn main() -> eframe::Result {
    let dir = std::env::var_os("OXIDE_REMOTE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share/oxide-remote-desktop")
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([412.0, 900.0]).with_app_id("oxide-remote"),
        ..Default::default()
    };
    eframe::run_native("Oxide Remote", options, Box::new(move |cc| Ok(Box::new(Root(oxide_remote::app::App::new(&cc.egui_ctx, dir, 1.0))))))
}
