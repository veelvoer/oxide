//! Oxide Remote for Wear OS. The protocol, the PC list, the colors and the Android glue are the phone app's own files
//! (one source of truth); this crate adds the watch's screens.
#[path = "../../phone/src/platform.rs"]
pub mod platform;
#[path = "../../phone/src/jstream.rs"]
pub mod jstream;
#[path = "../../phone/src/theme.rs"]
pub mod theme;

pub mod app;
pub mod calib;
pub mod icon_metrics;
pub mod gyro;
pub mod link;
pub mod motion;
pub mod round;
pub mod settings;

#[cfg(target_os = "android")]
mod android {
    use eframe::egui;

    struct Root(crate::app::App, u32);

    impl eframe::App for Root {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            // a panic in one frame must not close the app: it is logged and the next frame is drawn
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.0.frame(ui))).is_err() {
                self.1 += 1;
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                if self.1 > 20 {
                    std::process::exit(1);
                }
            } else {
                self.1 = 0;
            }
        }
    }

    #[unsafe(no_mangle)]
    fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("oxide-watch"));
        let dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"));
        let log = dir.join("crash.log");
        std::panic::set_hook(Box::new(move |info| {
            let text = format!("{info}\n{}\n", std::backtrace::Backtrace::force_capture());
            log::error!("panic: {text}");
            let _ = std::fs::write(&log, text);
        }));
        crate::platform::init(&app);
        crate::motion::init(&app);
        let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
        let r = eframe::run_native("Oxide Remote", options, Box::new(move |cc| Ok(Box::new(Root(crate::app::App::new(&cc.egui_ctx, dir), 0)))));
        if let Err(e) = r {
            log::error!("exit: {e}");
        }
    }
}
