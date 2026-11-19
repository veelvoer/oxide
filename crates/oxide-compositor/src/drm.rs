//! Native backend: DRM/KMS output, libinput, libseat session (run from a TTY).
use std::{collections::HashMap, time::Duration};

use oxide_config::Config;
use smithay::{
    backend::{
        allocator::{
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode,
        },
        egl::{context::ContextPriority, EGLContext, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles::GlesRenderer, ImportDma, ImportMemWl},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{primary_gpu, UdevBackend, UdevEvent},
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Scale as OutScale},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{connector, crtc, ModeTypeFlags},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
        wayland_server::Display,
    },
    utils::{DeviceFd, Transform},
    wayland::dmabuf::DmabufFeedbackBuilder,
};
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};
use tracing::{debug, error, info, warn};

use crate::{
    render::{OxideElement, clear},
    state::Oxide,
};

const FORMATS: &[Fourcc] = &[Fourcc::Abgr2101010, Fourcc::Argb2101010, Fourcc::Abgr8888, Fourcc::Argb8888];

type OxDrmOutput =
    DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;
type OxManager =
    DrmOutputManager<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

struct OutData {
    output: Output,
    modes: Vec<smithay::reexports::drm::control::Mode>,
    drm_output: OxDrmOutput,
    /// A frame was queued and the vblank has not arrived yet.
    in_flight: bool,
    /// Something changed while a frame was in flight.
    redraw: bool,
}

pub struct DrmData {
    pub session: LibSeatSession,
    node: DrmNode,
    pub renderer: GlesRenderer,
    manager: OxManager,
    scanner: DrmScanner,
    outputs: HashMap<crtc::Handle, OutData>,
    pub libinput: Libinput,
    pub devices: Vec<smithay::reexports::input::Device>,
}

pub fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<Oxide> = EventLoop::try_new()?;
    let display: Display<Oxide> = Display::new()?;
    let dh = display.handle();

    let (session, notifier) = LibSeatSession::new().map_err(|e| format!("cannot open session (seatd/logind): {e}"))?;
    let seat_name = session.seat();

    let gpu_path = std::env::var("OXIDE_DRM_DEVICE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| primary_gpu(&seat_name).ok().flatten())
        .ok_or("no GPU found")?;
    let node = DrmNode::from_path(&gpu_path)?;
    info!("using GPU {}", gpu_path.display());

    let fd = session
        .clone()
        .open(&gpu_path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
        .map_err(|e| format!("cannot open {}: {e}", gpu_path.display()))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let egl_device = EGLDevice::device_for_display(&egl)?;
    let render_node = egl_device.try_get_render_node().ok().flatten().unwrap_or(node);
    let context = EGLContext::new_with_priority(&egl, ContextPriority::High)?;
    let renderer = unsafe { GlesRenderer::new(context)? };

    let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), Some(render_node).into());
    let render_formats: FormatSet = renderer.egl_context().dmabuf_render_formats().iter().copied().collect();
    let manager = DrmOutputManager::new(drm, allocator, exporter, Some(gbm), FORMATS.iter().copied(), render_formats);

    let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput.udev_assign_seat(&seat_name).map_err(|_| "libinput seat assignment failed")?;

    let mut state = Oxide::new(display, event_loop.handle(), event_loop.get_signal(), config);

    // dmabuf
    let formats = renderer.dmabuf_formats();
    match DmabufFeedbackBuilder::new(render_node.dev_id(), formats.clone()).build() {
        Ok(fb) => {
            state.dmabuf_state.create_global_with_default_feedback::<Oxide>(&dh, &fb);
        }
        Err(_) => {
            state.dmabuf_state.create_global::<Oxide>(&dh, formats);
        }
    }
    state.shm_state.update_formats(renderer.shm_formats());

    state.drm = Some(DrmData {
        session,
        node,
        renderer,
        manager,
        scanner: DrmScanner::new(),
        outputs: HashMap::new(),
        libinput: libinput.clone(),
        devices: vec![],
    });

    // libinput events
    let backend = LibinputInputBackend::new(libinput);
    event_loop.handle().insert_source(backend, |mut event, _, state| {
        if let InputEvent::DeviceAdded { device } = &mut event {
            if device.has_capability(DeviceCapability::Keyboard) {
                if let Some(led) = state.seat.get_keyboard().map(|k| k.led_state()) {
                    device.led_update(led.into());
                }
            }
            info!(name = device.name(), "input device added");
            if let Some(d) = state.drm.as_mut() {
                d.devices.push(device.clone());
            }
            state.configure_input_device(device);
        } else if let InputEvent::DeviceRemoved { device } = &event {
            if let Some(d) = state.drm.as_mut() {
                d.devices.retain(|x| x != device);
            }
        }
        state.process_input_event(event);
    })?;

    // vblank
    event_loop.handle().insert_source(drm_notifier, move |event, _, state| match event {
        DrmEvent::VBlank(crtc) => state.drm_frame_done(crtc),
        DrmEvent::Error(e) => error!("drm error: {e}"),
    })?;

    // session (VT switching)
    event_loop.handle().insert_source(notifier, |event, _, state| {
        let Some(d) = state.drm.as_mut() else { return };
        match event {
            SessionEvent::PauseSession => {
                info!("session paused");
                d.libinput.suspend();
                d.manager.pause();
            }
            SessionEvent::ActivateSession => {
                info!("session resumed");
                if d.libinput.resume().is_err() {
                    error!("failed to resume libinput");
                }
                if let Err(e) = d.manager.activate(false) {
                    error!("failed to reactivate drm: {e}");
                }
                // a flip queued before the pause will never complete
                for o in d.outputs.values_mut() {
                    o.in_flight = false;
                }
                state.queue_redraw(true);
            }
        }
    })?;

    // hotplug
    let udev = UdevBackend::new(&seat_name)?;
    event_loop.handle().insert_source(udev, move |event, _, state| match event {
        UdevEvent::Changed { device_id } if device_id == node.dev_id() => state.drm_scan_connectors(),
        UdevEvent::Added { path, .. } => debug!("ignoring additional gpu {}", path.display()),
        _ => {}
    })?;

    crate::ipc::init(&event_loop.handle());
    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    state.drm_scan_connectors();
    state.startup();

    event_loop.run(None::<Duration>, &mut state, |state| {
        state.after_dispatch();
        if state.dirty {
            state.dirty = false;
            state.queue_redraw(false);
        }
    })?;
    state.stop_shell();
    Ok(())
}

fn pick_mode(
    modes: &[smithay::reexports::drm::control::Mode],
    cfg: Option<&oxide_config::Monitor>,
) -> Option<usize> {
    let c = cfg?;
    modes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.size() == (c.width as u16, c.height as u16))
        .min_by_key(|(_, m)| match c.refresh {
            Some(want) if want > 0.0 => ((m.vrefresh() as f64 - want).abs() * 1000.0) as i64,
            _ => 0,
        })
        .map(|(i, _)| i)
}

impl Oxide {
    /// Re-apply the configured mode of every output (position and scale are handled in `apply_monitors`).
    pub fn drm_apply_modes(&mut self) {
        let Some(mut d) = self.drm.take() else { return };
        for o in d.outputs.values_mut() {
            let cfg = self.config.monitors.iter().find(|m| m.name == o.output.name());
            let Some(idx) = pick_mode(&o.modes, cfg) else { continue };
            let mode = o.modes[idx];
            if o.output.current_mode() == Some(WlMode::from(mode)) {
                continue;
            }
            match o.drm_output.use_mode(mode, &mut d.renderer, &DrmOutputRenderElements::<_, OxideElement>::default()) {
                Ok(()) => o.output.change_current_state(Some(WlMode::from(mode)), None, None, None),
                Err(e) => warn!("cannot switch {} mode: {e:?}", o.output.name()),
            }
        }
        self.drm = Some(d);
    }

    pub fn drm_scan_connectors(&mut self) {
        let Some(mut d) = self.drm.take() else { return };
        let events = match d.scanner.scan_connectors(d.manager.device()) {
            Ok(e) => e,
            Err(e) => {
                warn!("connector scan failed: {e}");
                self.drm = Some(d);
                return;
            }
        };
        for ev in events {
            match ev {
                DrmScanEvent::Connected { connector, crtc: Some(crtc) } => self.connector_connected(&mut d, connector, crtc),
                DrmScanEvent::Disconnected { crtc: Some(crtc), .. } => {
                    if let Some(o) = d.outputs.remove(&crtc) {
                        info!("output {} disconnected", o.output.name());
                        self.remove_monitor(&o.output);
                    }
                }
                _ => {}
            }
        }
        self.drm = Some(d);
        self.queue_redraw(true);
    }

    fn connector_connected(&mut self, d: &mut DrmData, connector: connector::Info, crtc: crtc::Handle) {
        let name = format!("{}-{}", connector.interface().as_str(), connector.interface_id());
        info!(%name, "connector connected");
        let mcfg = self.config.monitors.iter().find(|m| m.name == name).cloned();

        let modes = connector.modes();
        if modes.is_empty() {
            return;
        }
        let preferred = modes.iter().position(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).unwrap_or(0);
        let mode_idx = pick_mode(&modes, mcfg.as_ref()).unwrap_or(preferred);
        let drm_mode = modes[mode_idx];
        let wl_mode = WlMode::from(drm_mode);

        let (pw, ph) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            name.clone(),
            PhysicalProperties {
                size: (pw as i32, ph as i32).into(),
                subpixel: connector.subpixel().into(),
                make: "Unknown".into(),
                model: "Unknown".into(),
            },
        );
        let _global = output.create_global::<Oxide>(&self.display_handle);
        // keep the global alive for the lifetime of the output
        output.user_data().insert_if_missing(|| _global);

        let scale = mcfg.as_ref().map_or(1.0, |c| c.scale);
        output.set_preferred(WlMode::from(modes[preferred]));
        output.change_current_state(Some(wl_mode), Some(Transform::Normal), Some(OutScale::Fractional(scale)), None);

        let planes = d.manager.device().planes(&crtc).ok();
        let drm_output = match d.manager.initialize_output::<_, OxideElement>(
            crtc,
            drm_mode,
            &[connector.handle()],
            &output,
            planes,
            &mut d.renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(o) => o,
            Err(e) => {
                warn!("cannot initialize output {name}: {e}");
                return;
            }
        };

        info!(%name, mode = ?(drm_mode.size(), drm_mode.vrefresh()), scale, "output initialized");
        self.add_monitor(output.clone(), (0, 0));
        self.apply_monitors();
        d.outputs.insert(crtc, OutData { output, modes: modes.to_vec(), drm_output, in_flight: false, redraw: true });
    }

    /// Render every output that needs it. `force` ignores the dirty state.
    pub fn queue_redraw(&mut self, force: bool) {
        let _ = force;
        let Some(d) = self.drm.as_mut() else { return };
        let mut ready = vec![];
        for (crtc, o) in d.outputs.iter_mut() {
            o.redraw = true;
            if !o.in_flight {
                ready.push(*crtc);
            }
        }
        for crtc in ready {
            self.render_crtc(crtc);
        }
    }

    pub fn drm_frame_done(&mut self, crtc: crtc::Handle) {
        let Some(d) = self.drm.as_mut() else { return };
        let Some(o) = d.outputs.get_mut(&crtc) else { return };
        if let Err(e) = o.drm_output.frame_submitted() {
            warn!("frame_submitted: {e:?}");
        }
        o.in_flight = false;
        if o.redraw {
            self.render_crtc(crtc);
        }
    }

    fn render_crtc(&mut self, crtc: crtc::Handle) {
        let Some(mut d) = self.drm.take() else { return };
        if let Some(o) = d.outputs.get_mut(&crtc) {
            o.redraw = false;
            let output = o.output.clone();
            let elements = self.build_elements(&mut d.renderer, &output);
            match o.drm_output.render_frame(&mut d.renderer, &elements, clear(), FrameFlags::DEFAULT) {
                Ok(res) => {
                    if !res.is_empty {
                        match o.drm_output.queue_frame(()) {
                            Ok(()) => o.in_flight = true,
                            Err(e) => warn!("queue_frame failed: {e:?}"),
                        }
                    }
                }
                Err(e) => {
                    warn!("render_frame failed: {e:?}");
                    // retry in a moment (e.g. device inactive during VT switch)
                    o.redraw = true;
                    let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(100)), move |_, _, s| {
                        s.queue_redraw(true);
                        TimeoutAction::Drop
                    });
                }
            }
            self.send_frames(&output);
        }
        self.drm = Some(d);
    }

    /// Apply libinput settings (tap, scroll, acceleration) to one device.
    pub fn configure_input_device(&self, device: &mut smithay::reexports::input::Device) {
        let c = &self.config.input;
        use smithay::reexports::input::AccelProfile;
        if device.config_tap_finger_count() > 0 {
            let _ = device.config_tap_set_enabled(c.tap_to_click);
        }
        if device.config_scroll_has_natural_scroll() {
            // a touchpad and a mouse are set apart: natural scrolling feels right on one and wrong on the other
            let touchpad = device.config_tap_finger_count() > 0;
            let _ = device.config_scroll_set_natural_scroll_enabled(if touchpad { c.natural_scroll } else { c.mouse_natural_scroll });
        }
        if device.config_dwt_is_available() {
            let _ = device.config_dwt_set_enabled(c.disable_while_typing);
        }
        if device.config_accel_is_available() {
            let profile = if c.accel_profile == "flat" { AccelProfile::Flat } else { AccelProfile::Adaptive };
            let _ = device.config_accel_set_profile(profile);
            let _ = device.config_accel_set_speed(c.sensitivity.clamp(-1.0, 1.0));
        }
    }

    pub fn configure_all_inputs(&mut self) {
        let Some(d) = self.drm.as_ref() else { return };
        let mut devices = d.devices.clone();
        for dev in &mut devices {
            self.configure_input_device(dev);
        }
    }

    pub fn change_vt(&mut self, vt: i32) {
        if let Some(d) = self.drm.as_mut() {
            if let Err(e) = d.session.change_vt(vt) {
                error!("vt switch failed: {e}");
            }
        }
    }

    pub fn drm_renderer(&mut self) -> Option<&mut GlesRenderer> {
        self.drm.as_mut().map(|d| &mut d.renderer)
    }

    #[allow(dead_code)]
    fn _node(&self) -> Option<DrmNode> {
        self.drm.as_ref().map(|d| d.node)
    }
}
