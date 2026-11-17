use std::{collections::HashMap, sync::Arc, time::Duration};

use oxide_config::{Action, Config, LayoutKind};
use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker,
            element::solid::SolidColorBuffer,
            gles::{element::PixelShaderElement, GlesRenderer},
        },
        winit::WinitGraphicsBackend,
    },
    desktop::{PopupManager, Space, Window},
    input::{
        keyboard::{Keysym, XkbConfig},
        pointer::{CursorImageStatus, PointerHandle},
        Seat, SeatState,
    },
    output::Output,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Clock, Logical, Monotonic, Rectangle},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        dmabuf::DmabufState,
        output::OutputManagerState,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        shell::{wlr_layer::WlrLayerShellState, xdg::{decoration::XdgDecorationState, XdgShellState}},
        shm::ShmState,
        socket::ListeningSocketSource,
        viewporter::ViewporterState,
        cursor_shape::CursorShapeManagerState,
        fractional_scale::FractionalScaleManagerState,
        xdg_activation::XdgActivationState,
    },
};
use tracing::{info, warn};

use crate::{
    anim::{RectF, Tween},
    shaders::Shaders,
};

pub struct Win {
    pub window: Window,
    pub ws: u32,
    pub floating: bool,
    pub fullscreen: bool,
    pub float_geo: Option<Rectangle<i32, Logical>>,
    /// a window that cannot change size (dialogs): floated at exactly this size, centred
    pub fixed: Option<(i32, i32)>,
    /// Target outer rect (including border) of the last layout pass.
    pub rect: Rectangle<i32, Logical>,
    /// Outer rect after the move/resize animation (before open scaling).
    pub base: RectF,
    /// Currently displayed outer rect (base plus the open pop-in).
    pub cur: RectF,
    pub mv: Option<Tween<RectF>>,
    /// Start time of the open (pop-in) animation.
    pub open: Option<Duration>,
    /// A tiled Chromium/Electron window gets its real size only after its first frame (see `apply_visual`); until
    /// it has drawn at that size it is not shown, so nothing jumps.
    pub size_deferred: bool,
    /// a floating window that chooses its own size: placed at that size after its first frame
    pub auto_float: bool,
    pub held: Option<std::time::Instant>,
    /// Opacity multiplier from the fade animation, 0..=1.
    pub fade: f32,
    pub fade_anim: Option<Tween<f32>>,
    /// 0 = inactive border color, 1 = active border gradient.
    pub border_mix: f32,
    pub border_anim: Option<Tween<f32>>,
    /// Border size and tiled state used by the last layout pass.
    pub border: i32,
    pub tiled: bool,
    /// Whether the window is on a workspace shown on some monitor.
    pub visible: bool,
    /// Opacity multiplier from window rules.
    pub opacity: Option<f32>,
    /// what the window is doing (telemetry), since when
    pub signal: Option<(crate::telemetry::Sig, std::time::Instant)>,
    /// Blur override from window rules.
    pub blur: Option<bool>,
    /// Identity of this window's blur element; renewed whenever the blurred texture changes.
    pub blur_id: smithay::backend::renderer::element::Id,
    /// top, bottom, left, right (used when corners are square)
    pub borders: [SolidColorBuffer; 4],
    pub dim: SolidColorBuffer,
    pub border_el: Option<(PixelShaderElement, ElKey)>,
    pub shadow_el: Option<(PixelShaderElement, ElKey)>,
}

/// Cache key deciding when a shader element needs its uniforms refreshed.
pub type ElKey = (Rectangle<i32, Logical>, Vec<String>);

/// A window that was closed and is fading out from a snapshot of its last frame.
pub struct Closing {
    pub texture: smithay::backend::renderer::gles::GlesTexture,
    pub id: smithay::backend::renderer::element::Id,
    /// Outer rect (with border) where the window was.
    pub rect: RectF,
    pub border: i32,
    pub radius: i32,
    pub ws: u32,
    pub start: Duration,
    pub border_mix: f32,
}

pub struct WsAnim {
    pub mon: usize,
    pub old_ws: u32,
    pub dir: i32,
    pub start: Duration,
}

pub struct Monitor {
    pub output: Output,
    pub ws: u32,
    /// Workspace to return to when the special workspace is hidden.
    pub prev_ws: u32,
    /// a macro scratchpad shown over this monitor's workspace (its workspace id)
    pub overlay: Option<u32>,
}

pub struct CompiledBind {
    pub mods: u8,
    pub sym: Keysym,
    pub action: Action,
}

pub struct CompiledMouseBind {
    pub mods: u8,
    pub button: u32,
    pub action: oxide_config::MouseAction,
}

/// Interactive move/resize of a floating window with the pointer.
pub struct Drag {
    pub window: Window,
    pub resize: bool,
    pub button: u32,
    pub start_ptr: smithay::utils::Point<f64, Logical>,
    pub start_geo: Rectangle<i32, Logical>,
    /// resize from the left / top edge (pointer started in that half)
    pub left: bool,
    pub top: bool,
}

pub const MOD_LOGO: u8 = 1;
pub const MOD_CTRL: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_SHIFT: u8 = 8;

pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub damage_tracker: OutputDamageTracker,
}

#[allow(dead_code)] // protocol states must stay alive even though they are never read
pub struct Oxide {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Oxide>,
    pub loop_signal: LoopSignal,
    pub clock: Clock<Monotonic>,
    pub socket_name: String,
    pub dirty: bool,

    pub config: Config,
    pub binds: Vec<CompiledBind>,
    /// Keys whose press triggered a bind; their release is not forwarded.
    pub suppressed: Vec<Keysym>,
    /// Shortcut recorder: None = off, Some(None) = waiting for a key, Some(Some(..)) = got one.
    pub capture: Option<Option<(String, String)>>,
    /// While true injected keys skip the key binds (used by the send-key action).
    pub bypass_binds: bool,
    /// set while rendering on the DRM backend (see screencopy)
    pub on_drm: bool,
    /// the nested test window holds several monitors (outputs are not flipped; screenshots read bottom-up like DRM)
    pub winit_multi: bool,
    /// theme that follows the wallpaper: when the background layer last changed (first, last), the last wallpaper.json seen
    pub bg_dirty: Option<(std::time::Instant, std::time::Instant)>,
    pub auto_stamp: String,
    /// render only the background layer (to read the colors of whatever wallpaper the shell draws)
    pub sampling_bg: bool,
    /// the config file's last parse error (shown by the shell; the last good config stays active)
    pub config_error: Option<oxide_ipc::ConfigProblem>,
    pub whats_new: Option<oxide_ipc::WhatsNew>,
    pub config_mtime: Option<std::time::SystemTime>,
    /// Super is down with no other key pressed since (the ambient HUD shows)
    pub super_held: bool,
    pub super_down: Option<std::time::Instant>,
    pub super_combo: bool,
    /// scratchpad apps that were started and are expected to map: (app id, group workspace, when)
    pub group_pending: Vec<(String, u32, std::time::Instant)>,
    /// a button press that dismissed a group: its release is swallowed too
    pub group_swallow: Option<u32>,
    /// dims the workspace behind a shown scratchpad group
    pub scrim: SolidColorBuffer,
    /// the zoom-out overview (Super+Tab), when open
    pub overview: Option<crate::overview::Overview>,
    /// system CPU load 0..1 and the window processes the telemetry thread watches
    pub cpu_load: f32,
    pub telemetry_pids: crate::telemetry::PidSet,
    /// oxide-remoted, while Settings > Phone is on
    pub remote: Option<std::process::Child>,
    /// frame recorder (test aid): directory, frames left, frames written
    pub recorder: Option<(String, u32, u32)>,
    /// held send-key bind: (physical keycode, generation) so it repeats like a real key
    pub send_repeat: Option<(u32, u64)>,
    pub repeat_gen: u64,
    /// the recorder gives up after this, so a forgotten capture can never hold the keyboard
    pub capture_deadline: std::time::Instant,
    pub mouse_binds: Vec<CompiledMouseBind>,
    pub drag: Option<Drag>,
    pub xwm: Option<smithay::xwayland::X11Wm>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
    /// X11 override-redirect windows (menus, tooltips): drawn on top, not tiled.
    pub or_windows: Vec<Window>,
    pub session_lock_state: smithay::wayland::session_lock::SessionLockManagerState,
    pub lock: crate::lock::LockState,
    pub idle_notifier_state: smithay::wayland::idle_notify::IdleNotifierState<Oxide>,
    pub idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState,
    pub idle_inhibitors: std::collections::HashSet<WlSurface>,
    /// Set while a screenshot without cursor is rendered.
    pub hide_cursor: bool,
    pub constraint_surface: Option<WlSurface>,
    pub blurs: HashMap<String, crate::blur::Blur>,
    pub ipc_subs: Vec<std::os::unix::net::UnixStream>,
    /// the bundled shell's connection: it takes direct calls (see `shell_call`)
    pub shell_subs: Vec<std::os::unix::net::UnixStream>,
    pub last_state_json: String,
    pub scroll_accum: f64,
    pub shell: Option<crate::shell::ShellProc>,
    pub shell_crashes: u32,
    pub shell_active: (bool, String),
    pub shaders: Option<Shaders>,
    pub shaders_tried: bool,
    pub ws_anim: Option<WsAnim>,
    pub closing: Vec<Closing>,
    pub anim_timer: bool,

    pub space: Space<Window>,
    pub popups: PopupManager,
    pub wins: Vec<Win>,
    pub pending: Vec<Window>,
    pub monitors: Vec<Monitor>,
    pub focused_mon: usize,
    pub focus: Option<Window>,
    pub last_focus: HashMap<u32, Window>,
    pub ws_layout: HashMap<u32, LayoutKind>,
    /// Layer surface that currently holds keyboard focus (launchers etc).
    pub layer_focus: Option<WlSurface>,
    /// layer surfaces currently asking for keyboard focus
    pub kb_layers: Vec<WlSurface>,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Oxide>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub data_control_state: smithay::wayland::selection::wlr_data_control::DataControlState,
    pub dmabuf_state: DmabufState,
    pub viewporter_state: ViewporterState,
    pub cursor_shape_state: CursorShapeManagerState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub xdg_activation_state: XdgActivationState,

    pub seat: Seat<Oxide>,
    pub pointer: PointerHandle<Oxide>,
    pub cursor_status: CursorImageStatus,

    pub winit: Option<WinitData>,
    pub drm: Option<crate::drm::DrmData>,
    pub cursor: crate::cursor::CursorLoader,
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, r: DisconnectReason) { tracing::warn!("client disconnected: {:?}", match r { DisconnectReason::ConnectionClosed => "closed", DisconnectReason::ProtocolError(_) => "protocol error" }); if let DisconnectReason::ProtocolError(e) = r { tracing::warn!("protocol error: {:?}", e); } }
}

fn xkb_config(c: &oxide_config::Input) -> XkbConfig<'_> {
    XkbConfig {
        layout: &c.kb_layout,
        variant: &c.kb_variant,
        options: (!c.kb_options.is_empty()).then(|| c.kb_options.clone()),
        ..Default::default()
    }
}

impl Oxide {
    pub fn new(
        display: Display<Oxide>,
        loop_handle: LoopHandle<'static, Oxide>,
        loop_signal: LoopSignal,
        config: Config,
    ) -> Self {
        let dh = display.handle();

        let source = ListeningSocketSource::new_auto().expect("cannot create wayland socket");
        let socket_name = source.socket_name().to_string_lossy().into_owned();
        loop_handle
            .insert_source(source, |stream, _, state| {
                if let Err(e) = state.display_handle.insert_client(stream, Arc::new(ClientState::default())) {
                    warn!("error adding wayland client: {e}");
                }
            })
            .expect("wayland socket source");
        info!(name = socket_name, "listening on wayland socket");

        loop_handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
                // Safety: the display is never dropped while the source lives.
                unsafe { display.get_mut().dispatch_clients(state).unwrap() };
                Ok(PostAction::Continue)
            })
            .expect("wayland display source");

        smithay::wayland::tablet_manager::TabletManagerState::new::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        let pointer = seat.add_pointer();
        seat.add_keyboard(xkb_config(&config.input), config.input.repeat_delay, config.input.repeat_rate)
            .expect("keyboard init");

        smithay::wayland::relative_pointer::RelativePointerManagerState::new::<Self>(&dh);
        smithay::wayland::pointer_constraints::PointerConstraintsState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let mut s = Self {
            display_handle: dh.clone(),
            loop_handle: loop_handle.clone(),
            loop_signal,
            clock: Clock::new(),
            socket_name,
            dirty: true,
            binds: vec![],
            suppressed: vec![],
            capture: None,
            bypass_binds: false,
            on_drm: false,
            winit_multi: false,
            bg_dirty: None,
            auto_stamp: String::new(),
            sampling_bg: false,
            config_error: None,
            whats_new: None,
            config_mtime: None,
            super_held: false,
            super_down: None,
            super_combo: false,
            group_pending: vec![],
            group_swallow: None,
            scrim: SolidColorBuffer::default(),
            overview: None,
            cpu_load: 0.0,
            telemetry_pids: Default::default(),
            remote: None,
            recorder: None,
            send_repeat: None,
            repeat_gen: 0,
            capture_deadline: std::time::Instant::now(),
            mouse_binds: vec![],
            drag: None,
            xwm: None,
            xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(&dh),
            or_windows: vec![],
            session_lock_state: smithay::wayland::session_lock::SessionLockManagerState::new::<Self, _>(&dh, |_| true),
            lock: Default::default(),
            idle_notifier_state: smithay::wayland::idle_notify::IdleNotifierState::new(&dh, loop_handle.clone()),
            idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState::new::<Self>(&dh),
            idle_inhibitors: Default::default(),
            hide_cursor: false,
            ipc_subs: vec![],
            shell_subs: vec![],
            last_state_json: String::new(),
            scroll_accum: 0.0,
            blurs: HashMap::new(),
            constraint_surface: None,
            shell: None,
            shell_crashes: 0,
            shell_active: (false, String::new()),
            shaders: None,
            shaders_tried: false,
            ws_anim: None,
            closing: vec![],
            anim_timer: false,
            config: Config::default(),
            space: Space::default(),
            popups: PopupManager::default(),
            wins: vec![],
            pending: vec![],
            monitors: vec![],
            focused_mon: 0,
            focus: None,
            last_focus: HashMap::new(),
            ws_layout: HashMap::new(),
            layer_focus: None,
            kb_layers: Vec::new(),
            compositor_state: CompositorState::new::<Self>(&dh),
            xdg_shell_state: XdgShellState::new::<Self>(&dh),
            xdg_decoration_state: XdgDecorationState::new::<Self>(&dh),
            layer_shell_state: WlrLayerShellState::new::<Self>(&dh),
            shm_state: ShmState::new::<Self>(&dh, vec![]),
            output_manager_state: OutputManagerState::new_with_xdg_output::<Self>(&dh),
            seat_state,
            data_device_state: DataDeviceState::new::<Self>(&dh),
            data_control_state: smithay::wayland::selection::wlr_data_control::DataControlState::new::<Self, _>(
                &dh,
                Some(&primary_selection_state),
                |_| true,
            ),
            primary_selection_state,
            dmabuf_state: DmabufState::new(),
            viewporter_state: ViewporterState::new::<Self>(&dh),
            cursor_shape_state: CursorShapeManagerState::new::<Self>(&dh),
            fractional_scale_state: FractionalScaleManagerState::new::<Self>(&dh),
            xdg_activation_state: XdgActivationState::new::<Self>(&dh),
            seat,
            pointer,
            cursor_status: CursorImageStatus::default_named(),
            winit: None,
            drm: None,
            cursor: crate::cursor::CursorLoader::new(&config.general.cursor_theme, config.general.cursor_size),
        };
        s.config = config.clone();
        s.compile_binds();
        crate::screencopy::init(&dh);
        s
    }

    pub fn compile_binds(&mut self) {
        use smithay::input::keyboard::xkb;
        self.binds = self
            .config
            .binds
            .iter()
            .filter_map(|b| {
                let mods = parse_mods(&b.mods);
                let sym = xkb::keysym_from_name(&b.key, xkb::KEYSYM_CASE_INSENSITIVE);
                if sym.raw() == 0 {
                    warn!("unknown key {:?} in bind", b.key);
                    return None;
                }
                Some(CompiledBind { mods, sym, action: b.action.clone() })
            })
            .collect();
        self.mouse_binds = self
            .config
            .mouse_binds
            .iter()
            .map(|b| CompiledMouseBind {
                mods: parse_mods(&b.mods),
                button: match b.button {
                    oxide_config::MouseButton::Left => 0x110,
                    oxide_config::MouseButton::Right => 0x111,
                    oxide_config::MouseButton::Middle => 0x112,
                },
                action: b.action,
            })
            .collect();
    }

    /// Install a new config and apply everything that can change live.
    pub fn apply_config(&mut self, config: Config) {
        self.config = config;
        self.sync_remote();
        crate::render::set_clear(&oxide_config::theme::palette(&self.config.theme).bg);
        oxide_config::defaults::apply_env(&self.config);
        {
            let c = self.config.clone();
            std::thread::spawn(move || oxide_config::defaults::ensure(&c));
        }
        self.compile_binds();
        let wanted = (self.config.shell.enabled, if self.config.own.shell { self.config.shell.active.clone() } else { "oxide".to_string() });
        if wanted != self.shell_active {
            self.shell_active = wanted;
            self.shell_crashes = 0;
            self.restart_shell();
        }
        let cfg = self.config.clone();
        if let Some(kb) = self.seat.get_keyboard() {
            if let Err(e) = kb.set_xkb_config(self, xkb_config(&cfg.input)) {
                warn!("failed to apply xkb config: {e:?}");
            }
            kb.change_repeat_info(cfg.input.repeat_rate, cfg.input.repeat_delay);
        }
        for (k, v) in &cfg.env {
            std::env::set_var(k, v);
        }
        self.cursor.reload(&cfg.general.cursor_theme, cfg.general.cursor_size);
        self.configure_all_inputs();
        self.drm_apply_modes();
        self.apply_monitors();
        self.arrange();
        self.update_borders();
    }

    pub fn spawn(&self, cmd: &str) {
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .spawn()
        {
            Ok(mut child) => {
                // reap the child without blocking the compositor
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(e) => warn!("failed to spawn {cmd:?}: {e}"),
        }
    }

}

impl Oxide {
    /// Register an output, placing it to the right of the existing ones.
    pub fn add_monitor(&mut self, output: Output, loc: (i32, i32)) {
        self.space.map_output(&output, loc);
        if self.monitors.is_empty() {
            if let Some(g) = self.space.output_geometry(&output) {
                self.pointer.set_location((g.loc.x as f64 + g.size.w as f64 / 2.0, g.loc.y as f64 + g.size.h as f64 / 2.0).into());
            }
        }
        let used: Vec<u32> = self.monitors.iter().map(|m| m.ws).collect();
        let ws = (1..).find(|n| !used.contains(n)).unwrap_or(1);
        self.monitors.push(Monitor { output, ws, prev_ws: ws, overlay: None });
    }

    /// Run once the socket and ipc are up: env and autostart.
    /// Start or stop oxide-remoted to match the setting.
    pub fn sync_remote(&mut self) {
        let running = self.remote.as_mut().is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        let want = self.config.remote.enabled && std::env::var_os("OXIDE_TEST").is_none();
        if want && !running {
            self.remote = std::process::Command::new("oxide-remoted")
                .env("WAYLAND_DISPLAY", &self.socket_name)
                .stdin(std::process::Stdio::null())
                .spawn()
                .map_err(|e| tracing::warn!("cannot start oxide-remoted: {e}"))
                .ok();
        } else if !want && running {
            if let Some(mut c) = self.remote.take() {
                // ask it to stop (it removes the router's port forwarding), then make sure
                std::thread::spawn(move || {
                    let sock = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join("oxide-remote.sock");
                    if let Ok(mut s) = std::os::unix::net::UnixStream::connect(sock) {
                        use std::io::Write;
                        let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(4)));
                        let _ = writeln!(s, "quit");
                        let _ = std::io::Read::read(&mut s, &mut [0u8; 64]);
                    }
                    let _ = c.kill();
                    let _ = c.wait();
                });
            }
        }
    }

    pub fn startup(&mut self) {
        self.auto_theme_start();
        self.config_watch_start();
        self.telemetry_start();
        self.sync_remote();
        self.updates_start();
        // theme files and app colors: helper tools are involved, so never on the compositor thread
        crate::render::set_clear(&oxide_config::theme::palette(&self.config.theme).bg);
        // kitty windows started from now on read the standard (or the user's own) configuration
        oxide_config::defaults::apply_env(&self.config);
        let theme = self.config.theme.clone();
        std::thread::spawn(move || {
            let _ = oxide_config::theme::write_all(&theme);
        });
        std::env::set_var("XDG_SESSION_TYPE", "wayland");
        std::env::set_var("XDG_CURRENT_DESKTOP", "oxide");
        // LibreOffice draws its own title bar with the GTK backend; the Qt/KDE one follows the compositor's frame
        if std::env::var_os("SAL_USE_VCLPLUGIN").is_none() {
            let have = ["/usr/lib64/libreoffice/program/libvclplug_kf6lo.so", "/usr/lib/libreoffice/program/libvclplug_kf6lo.so"].iter().any(|p| std::path::Path::new(p).exists());
            if have {
                std::env::set_var("SAL_USE_VCLPLUGIN", "kf6");
            }
        }
        std::env::set_var("XCURSOR_THEME", &self.config.general.cursor_theme);
        std::env::set_var("XCURSOR_SIZE", self.config.general.cursor_size.to_string());
        for (k, v) in self.config.env.clone() {
            std::env::set_var(k, v);
        }
        // let portals and dbus-activated apps find this session
        self.spawn(
            "command -v dbus-update-activation-environment >/dev/null && \
             dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE XCURSOR_THEME XCURSOR_SIZE",
        );
        // portals (dark mode and file dialogs for Electron, Flutter and GTK apps) only start while a graphical
        // session target is active: Oxide brings one up, like other compositors do
        let wayland_socket = self.socket_name.clone();
        // (tests set OXIDE_TEST and must not touch the real session)
        if std::env::var_os("OXIDE_TEST").is_none() {
        std::thread::spawn(move || {
            if let Some(conf) = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))) {
                let unit = conf.join("systemd/user/oxide-session.target");
                let mut fresh = false;
                if !unit.exists() {
                    let _ = std::fs::create_dir_all(conf.join("systemd/user"));
                    fresh = std::fs::write(&unit, "[Unit]\nDescription=Oxide graphical session\nBindsTo=graphical-session.target\nWants=graphical-session-pre.target\nAfter=graphical-session-pre.target\n").is_ok();
                }
                let portals = conf.join("xdg-desktop-portal/oxide-portals.conf");
                if !portals.exists() {
                    let _ = std::fs::create_dir_all(conf.join("xdg-desktop-portal"));
                    let _ = std::fs::write(&portals, "[preferred]\ndefault=gtk\n");
                }
                if fresh {
                    let _ = std::process::Command::new("systemctl").args(["--user", "daemon-reload"]).status();
                }
            }
            // the services must see the Wayland display: import it first, then start the target
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg("timeout 10 dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE XCURSOR_THEME XCURSOR_SIZE 2>/dev/null; timeout 10 systemctl --user start oxide-session.target")
                .env("WAYLAND_DISPLAY", &wayland_socket)
                .status();
        });
        }
        if self.config.general.xwayland {
            self.start_xwayland();
        }
        self.shell_active = (self.config.shell.enabled, self.config.shell.active.clone());
        self.restart_shell();
        self.start_shell_watchdog();
        for cmd in self.config.exec_once.clone() {
            self.spawn(&cmd);
        }
    }
}

impl Oxide {
    /// Forget an output (unplugged); windows stay on their workspace.
    pub fn remove_monitor(&mut self, output: &Output) {
        if let Some(g) = output.user_data().get::<smithay::reexports::wayland_server::backend::GlobalId>() {
            self.display_handle.remove_global::<Oxide>(g.clone());
        }
        self.space.unmap_output(output);
        self.monitors.retain(|m| &m.output != output);
        if self.focused_mon >= self.monitors.len() {
            self.focused_mon = 0;
        }
        self.arrange();
    }

    /// Run `f` with the active backend's renderer (taken out of `self` so `f` may use `self`).
    pub fn with_renderer<R>(&mut self, f: impl FnOnce(&mut Self, &mut GlesRenderer) -> R) -> Option<R> {
        if let Some(mut w) = self.winit.take() {
            self.on_drm = self.winit_multi;
            let r = f(self, w.backend.renderer());
            self.winit = Some(w);
            return Some(r);
        }
        if let Some(mut d) = self.drm.take() {
            self.on_drm = true;
            let r = f(self, &mut d.renderer);
            self.drm = Some(d);
            return Some(r);
        }
        None
    }

    /// The renderer of whichever backend is active.
    pub fn renderer_mut(&mut self) -> Option<&mut GlesRenderer> {
        if self.winit.is_some() {
            return self.winit.as_mut().map(|w| w.backend.renderer());
        }
        self.drm_renderer()
    }
}

pub fn parse_mods(spec: &str) -> u8 {
    let mut mods = 0;
    for m in spec.split(|c| c == '+' || c == ' ').filter(|m| !m.is_empty()) {
        mods |= match m.to_ascii_uppercase().as_str() {
            "SUPER" | "LOGO" | "MOD4" | "WIN" => MOD_LOGO,
            "CTRL" | "CONTROL" => MOD_CTRL,
            "ALT" | "MOD1" => MOD_ALT,
            "SHIFT" => MOD_SHIFT,
            other => {
                warn!("unknown modifier {other:?}");
                0
            }
        };
    }
    mods
}

impl Oxide {
    /// Apply configured scale and position to the current outputs. Screens without a configured
    /// position are placed to the right of the ones before them.
    pub fn apply_monitors(&mut self) {
        let cfgs = self.config.monitors.clone();
        let mut next_x = 0;
        for m in &self.monitors {
            let c = cfgs.iter().find(|c| c.name == m.output.name());
            if let Some(c) = c {
                m.output.change_current_state(None, None, Some(smithay::output::Scale::Fractional(c.scale.clamp(0.25, 8.0))), None);
            }
            let pos = (c.and_then(|c| c.x).unwrap_or(next_x), c.and_then(|c| c.y).unwrap_or(0));
            m.output.change_current_state(None, None, None, Some(pos.into()));
            self.space.map_output(&m.output, pos);
            let width = self.space.output_geometry(&m.output).map_or(0, |g| g.size.w);
            next_x = next_x.max(pos.0 + width);
        }
        self.dirty = true;
    }
}

impl Oxide {
    /// Work done after every event-loop dispatch: housekeeping, ipc push, flush.
    pub fn after_dispatch(&mut self) {
        self.popups.cleanup();
        self.space.refresh();
        self.broadcast_state();
        let _ = self.display_handle.flush_clients();
    }
}
