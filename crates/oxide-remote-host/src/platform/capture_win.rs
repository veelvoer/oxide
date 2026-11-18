//! Screen capture on Windows. DXGI desktop duplication first: the graphics driver hands over each new picture of the
//! desktop (nothing happens while the screen does not change, and nothing is copied that is not needed). If that
//! is not possible (some remote sessions, old drivers) the screen is copied with GDI instead.
use std::time::Duration;

use windows::{
    core::Interface,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Dxgi::{Common::*, *},
            Gdi::*,
        },
    },
};

use super::{shrink_bgra, Image};

struct Dxgi {
    ctx: ID3D11DeviceContext,
    device: ID3D11Device,
    dup: IDXGIOutputDuplication,
    staging: Option<ID3D11Texture2D>,
    w: u32,
    h: u32,
}

impl Dxgi {
    /// The duplication of the primary screen, on whichever graphics adapter drives it.
    fn new() -> Result<Self, String> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| format!("DXGI: {e}"))?;
            let mut i = 0;
            let mut last = String::from("no screen found");
            while let Ok(adapter) = factory.EnumAdapters1(i) {
                i += 1;
                let mut o = 0;
                while let Ok(output) = adapter.EnumOutputs(o) {
                    o += 1;
                    let Ok(desc) = output.GetDesc() else { continue };
                    if !desc.AttachedToDesktop.as_bool() {
                        continue;
                    }
                    // the primary screen sits at the origin
                    let r = desc.DesktopCoordinates;
                    if r.left != 0 || r.top != 0 {
                        continue;
                    }
                    let mut device = None;
                    let mut ctx = None;
                    if let Err(e) = D3D11CreateDevice(
                        &adapter,
                        D3D_DRIVER_TYPE_UNKNOWN,
                        HMODULE::default(),
                        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                        Some(&[D3D_FEATURE_LEVEL_11_0]),
                        D3D11_SDK_VERSION,
                        Some(&mut device),
                        None,
                        Some(&mut ctx),
                    ) {
                        last = format!("Direct3D: {e}");
                        continue;
                    }
                    let (Some(device), Some(ctx)) = (device, ctx) else { continue };
                    let out1: IDXGIOutput1 = match output.cast() {
                        Ok(o) => o,
                        Err(e) => {
                            last = format!("DXGI: {e}");
                            continue;
                        }
                    };
                    match out1.DuplicateOutput(&device) {
                        Ok(dup) => {
                            let d = dup.GetDesc();
                            return Ok(Self { ctx, device, dup, staging: None, w: d.ModeDesc.Width, h: d.ModeDesc.Height });
                        }
                        Err(e) => last = format!("desktop duplication: {e}"),
                    }
                }
            }
            Err(last)
        }
    }

    /// The next picture, None when nothing changed within `wait_ms`.
    fn grab(&mut self, max_w: u32) -> Result<Option<Image>, String> {
        unsafe {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut res: Option<IDXGIResource> = None;
            match self.dup.AcquireNextFrame(16, &mut info, &mut res) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
                Err(e) => return Err(format!("screen capture lost: {e}")),
            }
            let Some(res) = res else {
                let _ = self.dup.ReleaseFrame();
                return Ok(None);
            };
            let out = (|| -> Result<Image, String> {
                let tex: ID3D11Texture2D = res.cast().map_err(|e| e.to_string())?;
                if self.staging.is_none() {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    tex.GetDesc(&mut desc);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.BindFlags = 0;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    desc.MiscFlags = 0;
                    desc.MipLevels = 1;
                    desc.ArraySize = 1;
                    desc.SampleDesc = DXGI_SAMPLE_DESC { Count: 1, Quality: 0 };
                    let mut st = None;
                    self.device.CreateTexture2D(&desc, None, Some(&mut st)).map_err(|e| e.to_string())?;
                    self.staging = st;
                    self.w = desc.Width;
                    self.h = desc.Height;
                }
                let staging = self.staging.as_ref().ok_or("no staging texture")?;
                self.ctx.CopyResource(staging, &tex);
                let mut m = D3D11_MAPPED_SUBRESOURCE::default();
                self.ctx.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut m)).map_err(|e| e.to_string())?;
                let bytes = std::slice::from_raw_parts(m.pData as *const u8, m.RowPitch as usize * self.h as usize);
                let img = shrink_bgra(bytes, self.w as usize, self.h as usize, m.RowPitch as usize, max_w as usize);
                self.ctx.Unmap(staging, 0);
                Ok(img)
            })();
            let _ = self.dup.ReleaseFrame();
            out.map(Some)
        }
    }
}

/// GDI: slower (the whole screen is copied every time) but works nearly everywhere.
struct Gdi {
    w: i32,
    h: i32,
    buf: Vec<u8>,
}

impl Gdi {
    fn new() -> Self {
        let (w, h) = super::screen_size();
        Self { w: w as i32, h: h as i32, buf: vec![0; (w * h * 4) as usize] }
    }

    fn grab(&mut self, max_w: u32) -> Result<Option<Image>, String> {
        unsafe {
            let screen = GetDC(None);
            if screen.is_invalid() {
                return Err("cannot read the screen".into());
            }
            let dc = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, self.w, self.h);
            let old = SelectObject(dc, bmp.into());
            let ok = BitBlt(dc, 0, 0, self.w, self.h, Some(screen), 0, 0, SRCCOPY | CAPTUREBLT).is_ok();
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = self.w;
            bi.bmiHeader.biHeight = -self.h; // top-down
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB.0;
            let lines = GetDIBits(dc, bmp, 0, self.h as u32, Some(self.buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            SelectObject(dc, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            if !ok || lines == 0 {
                return Err("the screen could not be copied (a locked screen?)".into());
            }
            Ok(Some(shrink_bgra(&self.buf, self.w as usize, self.h as usize, self.w as usize * 4, max_w as usize)))
        }
    }
}

enum Kind {
    Dxgi(Dxgi),
    Gdi(Gdi),
}

pub struct Capturer {
    kind: Kind,
}

impl Capturer {
    pub fn new() -> Result<Self, String> {
        match Dxgi::new() {
            Ok(d) => Ok(Self { kind: Kind::Dxgi(d) }),
            Err(e) => {
                eprintln!("oxide-remote-host: DXGI capture unavailable ({e}), using GDI");
                Ok(Self { kind: Kind::Gdi(Gdi::new()) })
            }
        }
    }

    pub fn grab(&mut self, max_w: u32) -> Result<Option<Image>, String> {
        let r = match &mut self.kind {
            Kind::Dxgi(d) => d.grab(max_w),
            Kind::Gdi(g) => {
                std::thread::sleep(Duration::from_millis(1));
                g.grab(max_w)
            }
        };
        r.map(|img| {
            img.map(|mut img| {
                if let Some((cx, cy)) = super::cursor_pos() {
                    let (sx, sy) = (img.full.0.max(1) as f32, img.full.1.max(1) as f32);
                    let (x, y) = ((cx as f32 * img.w as f32 / sx) as i32, (cy as f32 * img.h as f32 / sy) as i32);
                    super::draw_cursor(&mut img, x, y);
                }
                img
            })
        })
    }
}
