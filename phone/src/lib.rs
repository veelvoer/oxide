//! Oxide Remote for Android (and a desktop build for testing).
pub mod app;
pub mod net;
pub mod store;
pub mod theme;

#[cfg(target_os = "android")]
mod android {
    use eframe::egui;

    struct Root(crate::app::App);

    impl eframe::App for Root {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.0.frame(ui);
        }
    }

    #[unsafe(no_mangle)]
    fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("oxide-remote"));
        let dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"));
        let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
        let r = eframe::run_native(
            "Oxide Remote",
            options,
            Box::new(move |cc| {
                // phones are dense: scale egui points to roughly Android dp
                let ppp = cc.egui_ctx.pixels_per_point().max(2.0);
                Ok(Box::new(Root(crate::app::App::new(&cc.egui_ctx, dir, ppp))))
            }),
        );
        if let Err(e) = r {
            log::error!("exit: {e}");
        }
    }
}
