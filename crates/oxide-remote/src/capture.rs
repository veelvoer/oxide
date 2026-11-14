//! Screen capture with the wlr-screencopy protocol (Oxide, Sway, Hyprland and other wlroots compositors support it),
//! shrunk and JPEG-encoded for the phone.
use std::os::fd::AsFd;

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool},
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::screencopy::v1::client::{zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1};

#[derive(Default)]
struct St {
    format: Option<(wl_shm::Format, u32, u32, u32)>,
    buffer_done: bool,
    y_invert: bool,
    result: Option<bool>,
    outputs: Vec<(wl_output::WlOutput, String)>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for St {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_output::WlOutput, ()> for St {
    fn event(st: &mut Self, o: &wl_output::WlOutput, ev: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name } = ev {
            if let Some(e) = st.outputs.iter_mut().find(|(x, _)| x == o) {
                e.1 = name;
            }
        }
    }
}
impl Dispatch<wl_shm::WlShm, ()> for St {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm_pool::WlShmPool, ()> for St {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_buffer::WlBuffer, ()> for St {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for St {
    fn event(_: &mut Self, _: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _: zwlr_screencopy_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for St {
    fn event(st: &mut Self, _: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ev: zwlr_screencopy_frame_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        use zwlr_screencopy_frame_v1::Event as E;
        match ev {
            E::Buffer { format: WEnum::Value(f), width, height, stride } => {
                if st.format.is_none() && matches!(f, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888 | wl_shm::Format::Xbgr8888 | wl_shm::Format::Abgr8888) {
                    st.format = Some((f, width, height, stride));
                }
            }
            E::Flags { flags: WEnum::Value(fl) } => st.y_invert = fl.contains(zwlr_screencopy_frame_v1::Flags::YInvert),
            E::BufferDone => st.buffer_done = true,
            E::Ready { .. } => st.result = Some(true),
            E::Failed => st.result = Some(false),
            _ => {}
        }
    }
}

/// A raw frame: RGB, width, height.
pub struct Image {
    pub rgb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub full: (u32, u32),
}

pub struct Capturer {
    queue: EventQueue<St>,
    st: St,
    shm: wl_shm::WlShm,
    manager: zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    manager_version: u32,
    /// the shared memory the compositor copies into (kept while the size stays the same)
    mem: Option<(rustix::fd::OwnedFd, *mut u8, usize)>,
}

// the mapping is only touched from the thread that owns the Capturer
unsafe impl Send for Capturer {}

impl Drop for Capturer {
    fn drop(&mut self) {
        if let Some((_, ptr, len)) = self.mem.take() {
            unsafe {
                let _ = rustix::mm::munmap(ptr.cast(), len);
            }
        }
    }
}

impl Capturer {
    pub fn new() -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland display: {e}"))?;
        let (globals, mut queue) = registry_queue_init::<St>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).map_err(|_| "no wl_shm".to_string())?;
        let manager: zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1 =
            globals.bind(&qh, 1..=3, ()).map_err(|_| "the compositor does not offer screen capture (wlr-screencopy)".to_string())?;
        let manager_version = wayland_client::Proxy::version(&manager);
        let mut st = St::default();
        for g in globals.contents().clone_list() {
            if g.interface == "wl_output" {
                let o: wl_output::WlOutput = globals.registry().bind(g.name, g.version.min(4), &qh, ());
                st.outputs.push((o, String::new()));
            }
        }
        queue.roundtrip(&mut st).map_err(|e| e.to_string())?;
        Ok(Self { queue, st, shm, manager, manager_version, mem: None })
    }

    pub fn outputs(&self) -> Vec<String> {
        self.st.outputs.iter().map(|o| o.1.clone()).collect()
    }

    /// Capture `output` (or the first) and shrink it to at most `max_w` pixels wide.
    pub fn grab(&mut self, output: Option<&str>, max_w: u32) -> Result<Image, String> {
        let qh = self.queue.handle();
        let out = self
            .st
            .outputs
            .iter()
            .find(|o| output.is_some_and(|n| n == o.1))
            .or_else(|| self.st.outputs.first())
            .map(|o| o.0.clone())
            .ok_or("no screen")?;
        self.st.format = None;
        self.st.buffer_done = false;
        self.st.result = None;
        let frame = self.manager.capture_output(1, &out, &qh, ());
        // wait for the buffer description
        while self.st.format.is_none() || (self.manager_version >= 3 && !self.st.buffer_done) {
            if self.st.result == Some(false) {
                frame.destroy();
                return Err("capture failed".into());
            }
            self.queue.blocking_dispatch(&mut self.st).map_err(|e| e.to_string())?;
            if self.manager_version < 3 && self.st.format.is_some() {
                break;
            }
        }
        let (format, w, h, stride) = self.st.format.ok_or("no buffer format")?;
        let size = (stride * h) as usize;
        if self.mem.as_ref().map(|m| m.2) != Some(size) {
            if let Some((_, ptr, len)) = self.mem.take() {
                unsafe {
                    let _ = rustix::mm::munmap(ptr.cast(), len);
                }
            }
            let fd = rustix::fs::memfd_create("oxide-remote", rustix::fs::MemfdFlags::CLOEXEC).map_err(|e| e.to_string())?;
            rustix::fs::ftruncate(&fd, size as u64).map_err(|e| e.to_string())?;
            let ptr = unsafe {
                rustix::mm::mmap(std::ptr::null_mut(), size, rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE, rustix::mm::MapFlags::SHARED, &fd, 0)
                    .map_err(|e| e.to_string())?
            };
            self.mem = Some((fd, ptr.cast(), size));
        }
        let (fd, ptr, _) = self.mem.as_ref().unwrap();
        let pool = self.shm.create_pool(fd.as_fd(), size as i32, &qh, ());
        let buffer = pool.create_buffer(0, w as i32, h as i32, stride as i32, format, &qh, ());
        frame.copy(&buffer);
        while self.st.result.is_none() {
            self.queue.blocking_dispatch(&mut self.st).map_err(|e| e.to_string())?;
        }
        let ok = self.st.result == Some(true);
        frame.destroy();
        buffer.destroy();
        pool.destroy();
        if !ok {
            return Err("capture failed".into());
        }
        let src = unsafe { std::slice::from_raw_parts(*ptr, size) };
        let bgr = matches!(format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888);
        let t = std::time::Instant::now();
        let img = shrink(src, w, h, stride, bgr, self.st.y_invert, max_w);
        if std::env::var_os("OXIDE_REMOTE_DEBUG").is_some() {
            eprintln!("  shrink {} ms", t.elapsed().as_millis());
        }
        Ok(img)
    }
}

/// Scale down with a 2x2 average per sample (sharp enough for text), to RGB. Integer-only and branch-free in the
/// inner loop: about 5 ms for a 1080p screen.
fn shrink(src: &[u8], w: u32, h: u32, stride: u32, bgr: bool, flip: bool, max_w: u32) -> Image {
    let tw = w.min(max_w.max(160)) as usize;
    let th = ((h as u64 * tw as u64) / w.max(1) as u64).max(1) as usize;
    let (w, h, stride) = (w as usize, h as usize, stride as usize);
    let mut rgb = vec![0u8; tw * th * 3];
    // source columns for each target column (two samples a quarter step apart)
    let xs: Vec<(usize, usize)> = (0..tw)
        .map(|tx| {
            let x0 = ((tx * 4 + 1) * w) / (tw * 4);
            let x1 = (((tx * 4 + 3) * w) / (tw * 4)).min(w - 1);
            (x0 * 4, x1 * 4)
        })
        .collect();
    let (ri, bi) = if bgr { (2, 0) } else { (0, 2) };
    for (ty, out) in rgb.chunks_exact_mut(tw * 3).enumerate() {
        let mut y0 = ((ty * 4 + 1) * h) / (th * 4);
        let mut y1 = (((ty * 4 + 3) * h) / (th * 4)).min(h - 1);
        if flip {
            y0 = h - 1 - y0;
            y1 = h - 1 - y1;
        }
        let r0 = &src[y0 * stride..y0 * stride + w * 4];
        let r1 = &src[y1 * stride..y1 * stride + w * 4];
        for (o, &(a, b)) in out.chunks_exact_mut(3).zip(xs.iter()) {
            let px = |k: usize| (r0[a + k] as u16 + r0[b + k] as u16 + r1[a + k] as u16 + r1[b + k] as u16) >> 2;
            o[0] = px(ri) as u8;
            o[1] = px(1) as u8;
            o[2] = px(bi) as u8;
        }
    }
    Image { rgb, width: tw as u32, height: th as u32, full: (w as u32, h as u32) }
}

pub fn jpeg(img: &Image, quality: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.rgb.len() / 8);
    let enc = jpeg_encoder::Encoder::new(&mut out, quality.clamp(10, 95));
    let _ = enc.encode(&img.rgb, img.width as u16, img.height as u16, jpeg_encoder::ColorType::Rgb);
    out
}
