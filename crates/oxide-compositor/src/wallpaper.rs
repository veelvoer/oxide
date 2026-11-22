//! Oxide's own wallpaper engine. Options live in ~/.config/oxide/wallpaper.json (Settings > Theme > Background):
//!   { "mode": "photo" | "theme" | "image" | "color" | "slideshow",
//!     "photo": "dolomites",  "path": "/home/me/x.jpg",  "color": "#1b1917",  "fit": "fill" | "fit" | "center",
//!     "folder": "/home/me/Pictures/walls",  "interval": 600,  "shuffle": false }
//! A picture is decoded on a worker thread, scaled once to the size of the output it is for and drawn as one
//! element under everything else; a change cross-fades, and only the picture on show (and the one fading out) is kept.
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
    time::{Duration, Instant, SystemTime},
};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{memory::MemoryRenderBuffer, Kind},
            gles::GlesRenderer,
        },
    },
    utils::Transform,
};

use crate::render::OxideElement;

const FADE: Duration = Duration::from_millis(450);

#[derive(Clone, Default, PartialEq)]
struct Opts {
    mode: String,
    photo: String,
    path: String,
    color: String,
    fit: String,
    folder: String,
    interval: u64,
    shuffle: bool,
}

impl Opts {
    fn load() -> (Opts, Option<SystemTime>) {
        let p = file_path();
        let stamp = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
        let v: serde_json::Value = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let mut o = Opts {
            mode: s("mode"),
            photo: s("photo"),
            path: s("path"),
            color: s("color"),
            fit: s("fit"),
            folder: s("folder"),
            interval: v.get("interval").and_then(|x| x.as_u64()).unwrap_or(600).clamp(5, 86400),
            shuffle: v.get("shuffle").and_then(|x| x.as_bool()).unwrap_or(false),
        };
        // the older "scene" modes became photos
        if o.mode.is_empty() || o.mode.starts_with("scene") {
            o.mode = "photo".into();
        }
        if o.photo.is_empty() {
            o.photo = "dolomites".into();
        }
        (o, stamp)
    }
}

fn file_path() -> std::path::PathBuf {
    oxide_config::Config::path().with_file_name("wallpaper.json")
}

/// What to show on one output: a file, or a flat/gradient picture made from a color.
#[derive(Clone, PartialEq)]
enum Want {
    Nothing,
    File(String),
    Flat([u8; 3]),
    Gradient([u8; 3]),
}

struct Job {
    out: String,
    want: Want,
    size: (u32, u32),
    fit: String,
    bg: [u8; 3],
}

struct Done {
    out: String,
    want: Want,
    size: (u32, u32),
    /// pixel size of `bgra` (flat and gradient pictures are tiny; the renderer stretches them)
    dims: (u32, u32),
    bgra: Option<Vec<u8>>,
}

struct Pic {
    buf: MemoryRenderBuffer,
}

#[derive(Default)]
struct Slot {
    shown: Option<Pic>,
    old: Option<Pic>,
    fade: Option<Instant>,
    /// what the worker was last asked for (or the picture on show)
    asked: Option<(Want, (u32, u32))>,
}

pub struct Wallpaper {
    opts: Opts,
    stamp: Option<SystemTime>,
    checked: Instant,
    slots: HashMap<String, Slot>,
    jobs: Sender<Job>,
    done: Receiver<Done>,
    files: Vec<String>,
    index: usize,
    next_slide: Instant,
}

impl Wallpaper {
    pub fn new() -> Wallpaper {
        let (jobs, rx) = channel::<Job>();
        let (tx, done) = channel::<Done>();
        std::thread::Builder::new()
            .name("oxide-wallpaper".into())
            .spawn(move || {
                while let Ok(mut j) = rx.recv() {
                    // only the newest request per output matters
                    while let Ok(n) = rx.try_recv() {
                        if n.out == j.out {
                            j = n;
                        } else {
                            let _ = tx.send(render(j));
                            j = n;
                        }
                    }
                    if tx.send(render(j)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        let (opts, stamp) = Opts::load();
        let mut w = Wallpaper { opts, stamp, checked: Instant::now(), slots: HashMap::new(), jobs, done, files: vec![], index: 0, next_slide: Instant::now() };
        w.scan();
        w
    }

    fn scan(&mut self) {
        self.files.clear();
        if self.opts.mode != "slideshow" {
            return;
        }
        if let Ok(rd) = std::fs::read_dir(&self.opts.folder) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png")) {
                    self.files.push(p.display().to_string());
                }
            }
        }
        self.files.sort();
        tracing::info!("wallpaper slideshow: {} pictures in {}", self.files.len(), self.opts.folder);
        self.index = 0;
        self.next_slide = Instant::now() + Duration::from_secs(self.opts.interval);
    }

    fn current_want(&self, bg: [u8; 3]) -> Want {
        match self.opts.mode.as_str() {
            "color" => oxide_config::parse_color(&self.opts.color).map_or(Want::Gradient(bg), |c| Want::Flat([(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8])),
            "image" if !self.opts.path.is_empty() => Want::File(self.opts.path.clone()),
            "slideshow" => self.files.get(self.index).cloned().map_or(Want::Gradient(bg), Want::File),
            "photo" => {
                for d in oxide_config::wallpaper_dirs() {
                    for ext in ["jpg", "jpeg", "png"] {
                        let p = d.join(format!("{}.{ext}", self.opts.photo));
                        if p.exists() {
                            return Want::File(p.display().to_string());
                        }
                    }
                }
                Want::Gradient(bg)
            }
            _ => Want::Gradient(bg),
        }
    }

    /// Called a few times a second: look for a changed file, advance the slideshow, collect finished pictures.
    /// Returns true when a frame is needed.
    pub fn poll(&mut self) -> bool {
        let mut redraw = false;
        if self.checked.elapsed() > Duration::from_millis(700) {
            self.checked = Instant::now();
            let stamp = std::fs::metadata(file_path()).and_then(|m| m.modified()).ok();
            if stamp != self.stamp {
                let (o, s) = Opts::load();
                self.stamp = s;
                if o != self.opts {
                    self.opts = o;
                    self.scan();
                }
                redraw = true;
            }
        }
        if self.opts.mode == "slideshow" && !self.files.is_empty() && Instant::now() >= self.next_slide {
            self.next_slide = Instant::now() + Duration::from_secs(self.opts.interval);
            self.index = if self.opts.shuffle && self.files.len() > 1 { (self.index + 1 + (nanos() as usize) % (self.files.len() - 1)) % self.files.len() } else { (self.index + 1) % self.files.len() };
            redraw = true;
        }
        while let Ok(d) = self.done.try_recv() {
            let Some(slot) = self.slots.get_mut(&d.out) else { continue };
            if slot.asked.as_ref().map(|a| (&a.0, a.1)) != Some((&d.want, d.size)) {
                continue; // stale
            }
            let new = d.bgra.map(|px| Pic { buf: MemoryRenderBuffer::from_slice(&px, Fourcc::Argb8888, (d.dims.0 as i32, d.dims.1 as i32), 1, Transform::Normal, None) });
            slot.old = slot.shown.take();
            slot.shown = new;
            slot.fade = Some(Instant::now());
            redraw = true;
        }
        redraw
    }

    pub fn fading(&self) -> bool {
        self.slots.values().any(|s| s.fade.is_some())
    }

    /// The wallpaper of `out` (physical `size`, logical `logical`), lowest element last in `out`. `bg` is the theme's color.
    pub fn elements(&mut self, renderer: &mut GlesRenderer, name: &str, size: (u32, u32), logical: (i32, i32), bg: [u8; 3], out: &mut Vec<OxideElement>) {
        let want = self.current_want(bg);
        let fit = self.opts.fit.clone();
        let slot = self.slots.entry(name.to_string()).or_default();
        let ask = (want.clone(), size);
        if slot.asked.as_ref() != Some(&ask) {
            slot.asked = Some(ask);
            let _ = self.jobs.send(Job { out: name.to_string(), want, size, fit, bg });
        }
        let t = match slot.fade {
            Some(at) => {
                let t = (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0);
                if t >= 1.0 {
                    slot.fade = None;
                    slot.old = None;
                }
                // ease in-out
                t * t * (3.0 - 2.0 * t)
            }
            None => 1.0,
        };
        let mut add = |pic: &Pic, alpha: f32| {
            if let Ok(e) = smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                (0.0, 0.0),
                &pic.buf,
                Some(alpha),
                None,
                Some(logical.into()),
                Kind::Unspecified,
            ) {
                out.push(e.into());
            }
        };
        match (&slot.shown, &slot.old) {
            (Some(s), Some(o)) => {
                add(s, t);
                add(o, 1.0);
            }
            (Some(s), None) => add(s, t),
            (None, Some(o)) => add(o, 1.0 - t),
            (None, None) => {}
        }
    }
}

fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos())
}

/// Decode and scale one picture to the output's size, as BGRA (little-endian ARGB).
fn render(j: Job) -> Done {
    let (w, h) = j.size;
    let bgra = match &j.want {
        Want::Nothing => None,
        Want::Flat(c) => Some(vec![c[2], c[1], c[0], 255]).map(|p| (p, 1, 1)),
        Want::Gradient(c) => {
            let dark = |v: u8| (v as f32 / 1.25) as u8;
            let mut px = Vec::with_capacity(64 * 4);
            for y in 0..64 {
                let t = y as f32 / 63.0;
                let m = |v: u8| (v as f32 + (dark(v) as f32 - v as f32) * t) as u8;
                px.extend([m(c[2]), m(c[1]), m(c[0]), 255]);
            }
            Some((px, 1, 64))
        }
        Want::File(p) => load(p, w, h, &j.fit, j.bg).map(|px| (px, w, h)),
    };
    match bgra {
        Some((px, pw, ph)) => Done { out: j.out, want: j.want, size: j.size, dims: (pw, ph), bgra: Some(px) },
        None => Done { out: j.out, want: j.want, size: j.size, dims: j.size, bgra: None },
    }
}

fn load(path: &str, w: u32, h: u32, fit: &str, bg: [u8; 3]) -> Option<Vec<u8>> {
    use image::imageops::{self, FilterType};
    let img = image::open(path).ok()?.to_rgba8();
    let (iw, ih) = img.dimensions();
    if iw == 0 || ih == 0 || w == 0 || h == 0 {
        return None;
    }
    let mut canvas = image::RgbaImage::from_pixel(w, h, image::Rgba([bg[0], bg[1], bg[2], 255]));
    match fit {
        "fit" => {
            let k = (w as f32 / iw as f32).min(h as f32 / ih as f32);
            let (nw, nh) = (((iw as f32 * k).round() as u32).max(1), ((ih as f32 * k).round() as u32).max(1));
            let r = imageops::resize(&img, nw, nh, FilterType::CatmullRom);
            imageops::overlay(&mut canvas, &r, ((w - nw) / 2) as i64, ((h - nh) / 2) as i64);
        }
        "center" => {
            imageops::overlay(&mut canvas, &img, (w as i64 - iw as i64) / 2, (h as i64 - ih as i64) / 2);
        }
        _ => {
            let k = (w as f32 / iw as f32).max(h as f32 / ih as f32);
            let (nw, nh) = (((iw as f32 * k).ceil() as u32).max(w), ((ih as f32 * k).ceil() as u32).max(h));
            let r = imageops::resize(&img, nw, nh, FilterType::CatmullRom);
            let c = imageops::crop_imm(&r, (nw - w) / 2, (nh - h) / 2, w, h).to_image();
            canvas = c;
        }
    }
    let mut px = canvas.into_raw();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Some(px)
}
