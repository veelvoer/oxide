//! Oxide Remote for Android (and a desktop build for testing).
pub mod app;
pub mod bridge;
pub mod icon_metrics;
pub mod jstream;
pub mod net;
pub mod platform;
pub mod store;
pub mod theme;
pub mod ui;

#[cfg(target_os = "android")]
mod android {
    use eframe::egui;

    struct Root(crate::app::App, u32);

    impl eframe::App for Root {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            // a panic in one frame (a bad index, an odd answer from a PC) must not close the app: it is logged, the frame is
            // skipped and the next one is drawn
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.0.frame(ui)));
            if r.is_err() {
                self.1 += 1;
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                if self.1 > 20 {
                    // the same panic again and again: better to stop than to freeze
                    std::process::exit(1);
                }
            } else {
                self.1 = 0;
            }
        }
    }

    #[unsafe(no_mangle)]
    fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("oxide-remote"));
        let dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"));
        // every panic is written down (the log of the phone is hard to reach): crash.log next to the settings
        let log = dir.join("crash.log");
        std::panic::set_hook(Box::new(move |info| {
            let text = format!("{info}\n{}\n", std::backtrace::Backtrace::force_capture());
            log::error!("panic: {text}");
            let _ = std::fs::write(&log, text);
        }));
        crate::platform::init(&app);
        let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
        let r = eframe::run_native(
            "Oxide Remote",
            options,
            Box::new(move |cc| Ok(Box::new(Root(crate::app::App::new(&cc.egui_ctx, dir), 0)))),
        );
        if let Err(e) = r {
            log::error!("exit: {e}");
        }
    }
}
