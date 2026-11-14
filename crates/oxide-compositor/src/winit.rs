//! Nested backend: runs the compositor inside a window of another compositor.
use std::time::Duration;

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer, ImportDma, ImportMemWl},
        winit::{self, WinitEvent},
    },
    desktop::layer_map_for_output,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::Display,
    },
    utils::Transform,
    wayland::dmabuf::DmabufFeedbackBuilder,
};
use oxide_config::Config;
use tracing::{error, warn};

use crate::state::{Oxide, WinitData};

use crate::render::{clear, OxideElement};
use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};

smithay::render_elements! {
    /// An output's elements moved to that output's place in the one big test window.
    pub ShiftedElement<=GlesRenderer>;
    Inner = RelocateRenderElement<OxideElement>,
}


pub fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<Oxide> = EventLoop::try_new()?;
    let display: Display<Oxide> = Display::new()?;
    let dh = display.handle();

    // Test options: OXIDE_SIZE=3840x1080 sets the window size, OXIDE_OUTPUTS=3 splits it into that many monitors
    let n_outputs: i32 = std::env::var("OXIDE_OUTPUTS").ok().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 4);
    let mut attrs = smithay::reexports::winit::window::Window::default_attributes().with_title("Oxide");
    if let Some((w, h)) = std::env::var("OXIDE_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse::<f64>().ok(), b.parse::<f64>().ok()))).and_then(|(a, b)| a.zip(b)) {
        attrs = attrs.with_inner_size(smithay::reexports::winit::dpi::LogicalSize::new(w, h));
    }
    let (mut backend, winit_source) =
        winit::init_from_attributes::<GlesRenderer>(attrs).map_err(|e| format!("winit init: {e}"))?;
    let size = backend.window_size();
    // like real monitors: outputs without a flip, composited (and flipped) once. OXIDE_NORMAL=1 does that for one monitor too.
    let multi = n_outputs > 1 || std::env::var_os("OXIDE_NORMAL").is_some();
    let each = smithay::utils::Size::<i32, smithay::utils::Physical>::from((size.w / n_outputs, size.h));
    let mode = Mode { size: if multi { each } else { size }, refresh: 60_000 };
    let mut outputs: Vec<Output> = vec![];
    for i in 0..n_outputs {
        let name = if i == 0 { "winit".to_string() } else { format!("winit-{}", i + 1) };
        let o = Output::new(name, PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "Oxide".into(), model: "Winit".into() });
        let _global = o.create_global::<Oxide>(&dh);
        // a single window keeps the old (flipped) arrangement; several monitors are flipped once, when composited
        let transform = if multi { Transform::Normal } else { Transform::Flipped180 };
        o.change_current_state(Some(mode), Some(transform), None, Some((i * mode.size.w, 0).into()));
        o.set_preferred(mode);
        outputs.push(o);
    }
    let output = outputs[0].clone();
    // dmabuf: required by Mesa's Wayland EGL platform
    let mut state = Oxide::new(display, event_loop.handle(), event_loop.get_signal(), config);
    let formats = backend.renderer().dmabuf_formats();
    let render_node = smithay::backend::egl::EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|d| d.try_get_render_node());
    match render_node {
        Ok(Some(node)) => {
            let feedback = DmabufFeedbackBuilder::new(node.dev_id(), formats.clone()).build()?;
            state.dmabuf_state.create_global_with_default_feedback::<Oxide>(&dh, &feedback);
        }
        _ => {
            warn!("no render node, advertising dmabuf v3");
            state.dmabuf_state.create_global::<Oxide>(&dh, formats);
        }
    }
    state.shm_state.update_formats(backend.renderer().shm_formats());

    for (i, o) in outputs.iter().enumerate() {
        state.add_monitor(o.clone(), (i as i32 * mode.size.w, 0));
    }
    let damage_tracker = if multi {
        OutputDamageTracker::new(size, 1.0, Transform::Flipped180)
    } else {
        OutputDamageTracker::from_output(&output)
    };
    state.winit = Some(WinitData { backend, damage_tracker });
    state.winit_multi = multi;

    crate::ipc::init(&event_loop.handle());
    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    state.startup();

    event_loop.handle().insert_source(winit_source, move |event, _, state| match event {
        WinitEvent::Resized { size, .. } => {
            let each = smithay::utils::Size::<i32, smithay::utils::Physical>::from((size.w / n_outputs, size.h));
            for (i, o) in outputs.iter().enumerate() {
                let mode = Mode { size: if multi { each } else { size }, refresh: 60_000 };
                o.change_current_state(Some(mode), None, None, Some((i as i32 * mode.size.w, 0).into()));
                o.set_preferred(mode);
            }
            if multi {
                if let Some(w) = state.winit.as_mut() {
                    w.damage_tracker = OutputDamageTracker::new(size, 1.0, Transform::Flipped180);
                }
            }
            state.apply_monitors();
            state.arrange();
        }
        WinitEvent::Input(ev) => state.process_input_event(ev),
        WinitEvent::Redraw => state.render_winit(),
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        WinitEvent::Focus(_) => {}
    })?;

    event_loop.run(None::<Duration>, &mut state, |state| {
        state.after_dispatch();
        if state.dirty {
            state.dirty = false;
            if let Some(w) = &state.winit {
                w.backend.window().request_redraw();
            }
        }
    })?;
    state.stop_shell();
    Ok(())
}

impl Oxide {
    pub fn render_winit(&mut self) {
        let t_start = std::time::Instant::now();
        let outputs: Vec<Output> = self.monitors.iter().map(|m| m.output.clone()).collect();
        let multi_render = self.winit_multi;
        let Some(first) = outputs.first().cloned() else { return };
        let Some(mut wd) = self.winit.take() else { return };
        wd.backend.window().set_cursor_visible(false);
        let age = wd.backend.buffer_age().unwrap_or(0);
        let WinitData { backend, damage_tracker } = &mut wd;
        let res = backend.bind().and_then(|(renderer, mut fb)| {
            let render_err = |e: smithay::backend::renderer::damage::Error<smithay::backend::renderer::gles::GlesError>| match e {
                smithay::backend::renderer::damage::Error::Rendering(e) => e.into(),
                _ => unreachable!(),
            };
            if !multi_render {
                let elements = self.build_elements(renderer, &first);
                tracing::debug!(n = elements.len(), "frame elements");
                damage_tracker.render_output(renderer, &mut fb, age, &elements, clear()).map(|r| r.damage.cloned()).map_err(render_err)
            } else {
                // several test monitors in one window: every monitor's picture is moved to its own place
                let mut all: Vec<ShiftedElement> = vec![];
                for o in &outputs {
                    let Some(geo) = self.space.output_geometry(o) else { continue };
                    let scale = smithay::utils::Scale::from(o.current_scale().fractional_scale());
                    let off: smithay::utils::Point<i32, smithay::utils::Physical> = geo.loc.to_physical_precise_round(scale);
                    all.extend(self.build_elements(renderer, o).into_iter().map(|e| ShiftedElement::from(RelocateRenderElement::from_element(e, off, Relocate::Relative))));
                }
                damage_tracker.render_output(renderer, &mut fb, age, &all, clear()).map(|r| r.damage.cloned()).map_err(render_err)
            }
        });
        match res {
            Ok(Some(damage)) => {
                if let Err(e) = backend.submit(Some(&damage)) {
                    warn!("submit failed: {e}");
                }
            }
            Ok(None) => {}
            Err(e) => error!("render error: {e}"),
        }
        self.winit = Some(wd);
        let t_render = t_start.elapsed();
        for o in &outputs {
            self.send_frames(o);
        }
        if t_start.elapsed() > Duration::from_millis(12) {
            tracing::info!(render_ms = t_render.as_millis() as u64, total_ms = t_start.elapsed().as_millis() as u64, "slow frame");
        }
        self.record_frame(&outputs);
        if self.recorder.is_some() {
            // keep rendering while recording, so every animation step is caught
            self.dirty = true;
        }
    }

    /// Test aid: write this frame of every monitor to the recorder's directory.
    fn record_frame(&mut self, outputs: &[Output]) {
        let Some((dir, left, n)) = self.recorder.clone() else { return };
        if left == 0 {
            self.recorder = None;
            return;
        }
        let t: std::time::Duration = self.clock.now().into();
        for o in outputs {
            let Some(mode) = o.current_mode() else { continue };
            let region = smithay::utils::Rectangle::<i32, smithay::utils::Physical>::new((0, 0).into(), mode.size);
            if let Some(px) = self.capture(o, region, true) {
                // half size keeps long recordings small (OXIDE_RECORD_FULL=1 keeps every pixel)
                let (w, h) = (mode.size.w as usize, mode.size.h as usize);
                let (px, ow, oh) = if std::env::var_os("OXIDE_RECORD_FULL").is_some() {
                    (px, w, h)
                } else {
                    let mut out = Vec::with_capacity(px.len() / 4);
                    for y in (0..h).step_by(2) {
                        for x in (0..w).step_by(2) {
                            let i = (y * w + x) * 4;
                            out.extend_from_slice(&px[i..i + 4]);
                        }
                    }
                    (out, w.div_ceil(2), h.div_ceil(2))
                };
                let name = format!("{dir}/f{n:04}_t{}_{}_{}x{}.raw", t.as_millis(), o.name(), ow, oh);
                let _ = std::fs::write(name, px);
            }
        }
        self.recorder = Some((dir, left - 1, n + 1));
    }

    pub fn send_frames(&self, output: &Output) {
        let now = self.clock.now();
        for w in self.space.elements() {
            w.send_frame(output, now, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
        for layer in layer_map_for_output(output).layers() {
            layer.send_frame(output, now, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
        // the lock screen needs frame callbacks too, or it never redraws (dots while typing, errors)
        if let Some(ls) = self.lock_surface_for(output) {
            smithay::desktop::utils::send_frames_surface_tree(ls.wl_surface(), output, now, Some(Duration::ZERO), |_, _| Some(output.clone()));
        }
    }
}
