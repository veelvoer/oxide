//! wlr-screencopy (shm only): lets `grim` and friends take screenshots.
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            gles::{GlesRenderer, GlesTexture},
            Bind, ExportMem, Offscreen, TextureMapping,
        },
    },
    output::Output,
    reexports::wayland_server::{
        protocol::wl_shm,
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
    },
    utils::{Buffer as BufferCoord, Logical, Physical, Rectangle, Size, Transform},
    wayland::shm::with_buffer_contents_mut,
};
use tracing::warn;
use wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1::{self, Flags, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};

use crate::{render::clear, state::Oxide};

pub struct FrameData {
    output: Output,
    /// Capture region in physical output pixels.
    region: Rectangle<i32, Physical>,
    overlay_cursor: bool,
    used: AtomicBool,
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<Oxide, ZwlrScreencopyManagerV1, _>(3, ());
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for Oxide {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrScreencopyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Oxide {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let (frame, overlay_cursor, wl_output, region) = match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput { frame, overlay_cursor, output } => {
                (frame, overlay_cursor != 0, output, None)
            }
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion { frame, overlay_cursor, output, x, y, width, height } => {
                (frame, overlay_cursor != 0, output, Some(Rectangle::<i32, Logical>::new((x, y).into(), (width, height).into())))
            }
            _ => return,
        };
        let Some(output) = Output::from_resource(&wl_output) else {
            let f = data_init.init(
                frame,
                FrameData { output: Output::new("none".into(), smithay::output::PhysicalProperties {
                    size: (0, 0).into(), subpixel: smithay::output::Subpixel::Unknown, make: String::new(), model: String::new(),
                }), region: Rectangle::default(), overlay_cursor: false, used: AtomicBool::new(true) },
            );
            f.failed();
            return;
        };
        let Some(mode) = output.current_mode() else { return };
        let scale = output.current_scale().fractional_scale();
        let full = Rectangle::<i32, Physical>::from_size(mode.size);
        let region = region
            .map(|r| r.to_f64().to_physical(scale).to_i32_round::<i32>())
            .and_then(|r| r.intersection(full))
            .unwrap_or(full);
        let f = data_init.init(frame, FrameData { output, region, overlay_cursor, used: AtomicBool::new(false) });
        f.buffer(wl_shm::Format::Xrgb8888, region.size.w as u32, region.size.h as u32, region.size.w as u32 * 4);
        if f.version() >= 3 {
            f.buffer_done();
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, FrameData> for Oxide {
    fn request(
        state: &mut Self,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let buffer = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => buffer,
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => buffer,
            _ => return,
        };
        if data.used.swap(true, Ordering::SeqCst) {
            frame.post_error(zwlr_screencopy_frame_v1::Error::AlreadyUsed, "frame already used");
            return;
        }
        let size = data.region.size;
        let ok = with_buffer_contents_mut(&buffer, |_, len, d| {
            d.width == size.w
                && d.height == size.h
                && d.stride == size.w * 4
                && matches!(d.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888)
                && len >= (d.stride * d.height) as usize
        })
        .unwrap_or(false);
        if !ok {
            frame.post_error(zwlr_screencopy_frame_v1::Error::InvalidBuffer, "buffer does not match the advertised format");
            return;
        }
        let Some(pixels) = state.capture(&data.output, data.region, data.overlay_cursor) else {
            frame.failed();
            return;
        };
        let _ = with_buffer_contents_mut(&buffer, |ptr, _, _| unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr, pixels.len());
        });
        frame.flags(Flags::empty());
        let t: Duration = state.clock.now().into();
        let secs = t.as_secs();
        frame.ready((secs >> 32) as u32, secs as u32, t.subsec_nanos());
    }
}

impl Oxide {
    /// Render `output` offscreen and return the BGRX pixels of `region`, top row first.
    pub fn capture(&mut self, output: &Output, region: Rectangle<i32, Physical>, overlay_cursor: bool) -> Option<Vec<u8>> {
        let mode = output.current_mode()?;
        let output = output.clone();
        self.hide_cursor = !overlay_cursor;
        let res = self.with_renderer(|state, renderer| render_to_bytes(state, renderer, &output, mode.size, region));
        self.hide_cursor = false;
        res.flatten()
    }
}

fn render_to_bytes(
    state: &mut Oxide,
    renderer: &mut GlesRenderer,
    output: &Output,
    full: Size<i32, Physical>,
    region: Rectangle<i32, Physical>,
) -> Option<Vec<u8>> {
    let buf_size: Size<i32, BufferCoord> = (full.w, full.h).into();
    let mut tex: GlesTexture = Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Argb8888, buf_size)
        .map_err(|e| warn!("screencopy: offscreen buffer: {e}"))
        .ok()?;
    let mut fb = renderer.bind(&mut tex).map_err(|e| warn!("screencopy: bind: {e}")).ok()?;
    let mut tracker = OutputDamageTracker::new(full, output.current_scale().fractional_scale(), Transform::Normal);
    let elements = state.build_elements(renderer, output);
    tracker
        .render_output(renderer, &mut fb, 0, &elements, clear())
        .map_err(|e| warn!("screencopy: render: {e:?}"))
        .ok()?;
    // Read the whole frame and crop here: the winit backend reports the row order correctly, the DRM backend
    // hands back GL's bottom-up rows without saying so (screenshots came out upside down).
    let all = Rectangle::<i32, BufferCoord>::new((0, 0).into(), (full.w, full.h).into());
    let mapping = renderer.copy_framebuffer(&fb, all, Fourcc::Argb8888).map_err(|e| warn!("screencopy: copy: {e}")).ok()?;
    // The picture comes out upright for a normally oriented output (real monitors, the multi-monitor test window).
    // Only the single nested window advertises Flipped180 (its surface is y-inverted), and its pictures are the
    // other way up.
    let bottom_up = output.current_transform() == Transform::Flipped180;
    let bytes = renderer.map_texture(&mapping).map_err(|e| warn!("screencopy: map: {e}")).ok()?;
    let stride = full.w as usize * 4;
    let rows: Vec<&[u8]> = bytes.chunks(stride).collect();
    let mut out = Vec::with_capacity(region.size.w as usize * region.size.h as usize * 4);
    for y in region.loc.y..region.loc.y + region.size.h {
        let src = if bottom_up { full.h - 1 - y } else { y };
        let row = rows.get(src as usize)?;
        let x0 = region.loc.x as usize * 4;
        out.extend_from_slice(row.get(x0..x0 + region.size.w as usize * 4)?);
    }
    Some(out)
}
