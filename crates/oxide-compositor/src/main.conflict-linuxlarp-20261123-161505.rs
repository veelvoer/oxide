mod anim;
mod autotheme;
mod confwatch;
mod power;
mod updates;
mod wallpaper;
mod groups;
mod overview;
mod telemetry;
mod blur;
mod cursor;
mod drm;
mod handlers;
mod inject;
mod input;
mod lock;
mod ipc;
mod render;
mod screencopy;
mod shell;
mod shaders;
mod state;
mod winit;
mod wm;
mod xwayland;

use tracing::{error, warn};

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let to_file = !std::env::args().any(|a| a == "--winit")
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
        && std::env::var_os("DISPLAY").is_none();
    // on a TTY the terminal is not useful for logs: write to a file instead
    let log_file = to_file
        .then(|| {
            let dir = std::env::var_os("XDG_CACHE_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))?
                .join("oxide");
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::File::create(dir.join("oxide.log")).ok()
        })
        .flatten();
    match log_file {
        Some(f) => tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(std::sync::Mutex::new(f)).init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));

    let config = oxide_config::Config::load().unwrap_or_else(|e| {
        warn!("cannot load config ({e}); using defaults");
        oxide_config::Config::default()
    });

    let nested = std::env::args().any(|a| a == "--winit")
        || (!std::env::args().any(|a| a == "--drm")
            && (std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()));
    let res = if nested { winit::run(config) } else { drm::run(config) };
    if let Err(e) = res {
        error!("fatal: {e}");
        eprintln!("oxide: fatal: {e}");
        std::process::exit(1);
    }
}
