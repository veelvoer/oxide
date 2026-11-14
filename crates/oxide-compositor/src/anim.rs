//! Animation engine: bezier and spring curves, shared by windows, borders and workspaces.
pub use oxide_config::anim::{popin_scale, progress};
use smithay::utils::{Logical, Rectangle};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectF {
    pub fn from_rect(r: Rectangle<i32, Logical>) -> Self {
        Self { x: r.loc.x as f64, y: r.loc.y as f64, w: r.size.w as f64, h: r.size.h as f64 }
    }

    pub fn lerp(self, to: Self, t: f64) -> Self {
        let l = |a: f64, b: f64| a + (b - a) * t;
        Self { x: l(self.x, to.x), y: l(self.y, to.y), w: l(self.w, to.w), h: l(self.h, to.h) }
    }

    /// Scale about the center.
    pub fn scaled(self, f: f64) -> Self {
        Self { x: self.x + self.w * (1.0 - f) / 2.0, y: self.y + self.h * (1.0 - f) / 2.0, w: self.w * f, h: self.h * f }
    }

    pub fn rounded(self) -> Rectangle<i32, Logical> {
        Rectangle::new(
            (self.x.round() as i32, self.y.round() as i32).into(),
            (self.w.round().max(1.0) as i32, self.h.round().max(1.0) as i32).into(),
        )
    }
}

/// An animation between two values, started at `start` (monotonic time).
#[derive(Clone, Copy, Debug)]
pub struct Tween<T> {
    pub from: T,
    pub to: T,
    pub start: std::time::Duration,
}

