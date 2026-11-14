//! Dual-kawase background blur (xray: only layer-shell background/bottom surfaces are blurred).
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::RenderElement,
            gles::{GlesRenderer, GlesTexProgram, GlesTexture, Uniform},
            Bind, Frame, Offscreen, Renderer, Texture,
        },
    },
    utils::{Buffer as BufferCoord, Physical, Rectangle, Size, Transform},
};
use tracing::warn;

use crate::render::OxideElement;

pub struct Blur {
    pub size: Size<i32, Physical>,
    pub passes: u32,
    tracker: OutputDamageTracker,
    bg: GlesTexture,
    chain: Vec<GlesTexture>,
    fresh: bool,
}

fn make_tex(r: &mut GlesRenderer, w: i32, h: i32) -> Option<GlesTexture> {
    let size: Size<i32, BufferCoord> = (w.max(1), h.max(1)).into();
    Offscreen::<GlesTexture>::create_buffer(r, Fourcc::Abgr8888, size)
        .map_err(|e| warn!("blur: cannot create texture: {e}"))
        .ok()
}

fn pass(
    r: &mut GlesRenderer,
    src: &GlesTexture,
    dst: &mut GlesTexture,
    prog: &GlesTexProgram,
    offset: f32,
) -> Option<()> {
    let ds = dst.size();
    let ss = src.size();
    let size: Size<i32, Physical> = (ds.w, ds.h).into();
    let mut fb = r.bind(dst).ok()?;
    let mut frame = r.render(&mut fb, size, Transform::Normal).ok()?;
    let dst_rect = Rectangle::from_size(size);
    let src_rect = Rectangle::<f64, BufferCoord>::from_size((ss.w as f64, ss.h as f64).into());
    frame
        .render_texture_from_to(
            src,
            src_rect,
            dst_rect,
            &[dst_rect],
            &[],
            Transform::Normal,
            1.0,
            Some(prog),
            &[
                Uniform::new("halfpixel", (0.5 / ss.w as f32, 0.5 / ss.h as f32)),
                Uniform::new("offset", offset),
            ],
        )
        .ok()?;
    // same GL context: later passes are ordered after this one without waiting
    let _sync = frame.finish().ok()?;
    Some(())
}

impl Blur {
    pub fn new(r: &mut GlesRenderer, size: Size<i32, Physical>, passes: u32, scale: f64) -> Option<Self> {
        let passes = passes.clamp(1, 6);
        let bg = make_tex(r, size.w, size.h)?;
        let mut chain = vec![];
        let (mut w, mut h) = (size.w, size.h);
        for _ in 0..passes {
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            chain.push(make_tex(r, w, h)?);
        }
        Some(Self { size, passes, tracker: OutputDamageTracker::new(size, scale, Transform::Normal), bg, chain, fresh: true })
    }

    /// Re-render the background and re-blur when it changed. Returns true when the result is new.
    pub fn update(
        &mut self,
        r: &mut GlesRenderer,
        elements: &[OxideElement],
        down: &GlesTexProgram,
        up: &GlesTexProgram,
        offset: f32,
        clear: [f32; 4],
    ) -> bool
    where
        OxideElement: RenderElement<GlesRenderer>,
    {
        let age = if self.fresh { 0 } else { 1 };
        let changed = {
            let Ok(mut fb) = r.bind(&mut self.bg) else { return false };
            match self.tracker.render_output(r, &mut fb, age, elements, clear) {
                Ok(res) => res.damage.is_some(),
                Err(_) => {
                    warn!("blur: background render failed");
                    return false;
                }
            }
        };
        if !changed && !self.fresh {
            return false;
        }
        self.fresh = false;
        // down
        let mut src = self.bg.clone();
        for i in 0..self.chain.len() {
            let mut dst = self.chain[i].clone();
            if pass(r, &src, &mut dst, down, offset).is_none() {
                return false;
            }
            src = dst;
        }
        // up (chain[0] ends up holding the final, half-resolution result)
        for i in (0..self.chain.len().saturating_sub(1)).rev() {
            let src = self.chain[i + 1].clone();
            let mut dst = self.chain[i].clone();
            if pass(r, &src, &mut dst, up, offset).is_none() {
                return false;
            }
        }
        true
    }

    pub fn texture(&self) -> GlesTexture {
        self.chain[0].clone()
    }
}
