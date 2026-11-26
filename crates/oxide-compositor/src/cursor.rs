//! XCursor theme loading with per-name caching.
use std::{collections::HashMap, io::Read, time::Duration};

use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};
use tracing::warn;
use xcursor::{
    parser::{parse_xcursor, Image},
    CursorTheme,
};

static FALLBACK: &[u8] = include_bytes!("../resources/cursor.rgba");

pub struct CursorLoader {
    theme: CursorTheme,
    size: u32,
    icons: HashMap<String, Vec<Image>>,
    buffers: Vec<((String, u32, u32), MemoryRenderBuffer)>,
}

pub struct CursorFrame {
    pub buffer: MemoryRenderBuffer,
    pub hotspot: (i32, i32),
}

impl CursorLoader {
    pub fn new(theme: &str, size: u32) -> Self {
        Self { theme: CursorTheme::load(theme), size, icons: HashMap::new(), buffers: Vec::new() }
    }

    pub fn reload(&mut self, theme: &str, size: u32) {
        *self = Self::new(theme, size);
    }

    fn load(theme: &CursorTheme, name: &str) -> Option<Vec<Image>> {
        let path = theme.load_icon(name)?;
        let mut data = Vec::new();
        std::fs::File::open(path).ok()?.read_to_end(&mut data).ok()?;
        parse_xcursor(&data)
    }

    fn images(&mut self, name: &str) -> &Vec<Image> {
        if !self.icons.contains_key(name) {
            let imgs = Self::load(&self.theme, name)
                .or_else(|| Self::load(&self.theme, "default"))
                .unwrap_or_else(|| {
                    warn!("no xcursor theme found, using built-in cursor");
                    vec![Image {
                        size: 32,
                        width: 64,
                        height: 64,
                        xhot: 1,
                        yhot: 1,
                        delay: 1,
                        pixels_rgba: FALLBACK.to_vec(),
                        pixels_argb: vec![],
                    }]
                });
            self.icons.insert(name.to_string(), imgs);
        }
        &self.icons[name]
    }

    /// The first frame of an icon as plain RGBA pixels: (pixels, width, height, hotspot x, hotspot y). For X11's root cursor.
    pub fn raw(&mut self, name: &str) -> (Vec<u8>, u16, u16, u16, u16) {
        let size = self.size;
        self.images(name);
        let img = self.icons[name].iter().min_by_key(|i| (size as i32 - i.size as i32).abs()).unwrap();
        (img.pixels_rgba.clone(), img.width as u16, img.height as u16, img.xhot as u16, img.yhot as u16)
    }

    /// Cursor image for a CSS-style icon name at the given time.
    pub fn get(&mut self, name: &str, scale: u32, time: Duration) -> CursorFrame {
        let size = self.size * scale;
        self.images(name);
        let imgs = &self.icons[name];
        let nearest = imgs.iter().min_by_key(|i| (size as i32 - i.size as i32).abs()).unwrap();
        let (w, h) = (nearest.width, nearest.height);
        let frames: Vec<(usize, &Image)> =
            imgs.iter().filter(|i| i.width == w && i.height == h).enumerate().collect();
        let total: u32 = frames.iter().map(|(_, i)| i.delay).sum();
        let mut ms = if total == 0 { 0 } else { time.as_millis() as u32 % total };
        let mut pick = frames[0];
        for f in &frames {
            if ms < f.1.delay {
                pick = *f;
                break;
            }
            ms -= f.1.delay;
        }
        let (idx, img) = pick;
        let hotspot = (img.xhot as i32, img.yhot as i32);
        let key = (name.to_string(), size, idx as u32);
        if let Some((_, b)) = self.buffers.iter().find(|(k, _)| *k == key) {
            return CursorFrame { buffer: b.clone(), hotspot };
        }
        let buffer = MemoryRenderBuffer::from_slice(
            &img.pixels_rgba,
            Fourcc::Argb8888,
            (img.width as i32, img.height as i32),
            scale as i32,
            Transform::Normal,
            None,
        );
        self.buffers.push((key, buffer.clone()));
        CursorFrame { buffer, hotspot }
    }
}
