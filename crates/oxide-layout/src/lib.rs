//! Pure tiling geometry: no Wayland dependencies, fully unit-testable.
use oxide_config::{Dir, LayoutKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x as f64 && py >= self.y as f64 && px < self.right() as f64 && py < self.bottom() as f64
    }
}

/// Dwindle: every window takes `ratio` of the longer side of the remaining area.
pub fn dwindle(area: Rect, n: usize, ratio: f32) -> Vec<Rect> {
    let ratio = ratio.clamp(0.1, 0.9);
    let mut out = Vec::with_capacity(n);
    let mut rest = area;
    for i in 0..n {
        if i == n - 1 {
            out.push(rest);
            break;
        }
        if rest.w >= rest.h {
            let w = (rest.w as f32 * ratio) as i32;
            out.push(Rect::new(rest.x, rest.y, w, rest.h));
            rest = Rect::new(rest.x + w, rest.y, rest.w - w, rest.h);
        } else {
            let h = (rest.h as f32 * ratio) as i32;
            out.push(Rect::new(rest.x, rest.y, rest.w, h));
            rest = Rect::new(rest.x, rest.y + h, rest.w, rest.h - h);
        }
    }
    out
}

/// Master/stack: window 0 is master on the left, the rest stack on the right.
pub fn master(area: Rect, n: usize, factor: f32) -> Vec<Rect> {
    match n {
        0 => vec![],
        1 => vec![area],
        _ => {
            let mw = (area.w as f32 * factor.clamp(0.1, 0.9)) as i32;
            let mut out = vec![Rect::new(area.x, area.y, mw, area.h)];
            let stack = n - 1;
            let sh = area.h / stack as i32;
            for i in 0..stack {
                let y = area.y + sh * i as i32;
                let h = if i == stack - 1 { area.bottom() - y } else { sh };
                out.push(Rect::new(area.x + mw, y, area.w - mw, h));
            }
            out
        }
    }
}

pub fn compute(kind: LayoutKind, area: Rect, n: usize, split_ratio: f32, master_factor: f32) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    match kind {
        LayoutKind::Dwindle => dwindle(area, n, split_ratio),
        LayoutKind::Master => master(area, n, master_factor),
    }
}

/// Inset tiles: `gaps_out` against the area edge, `gaps_in` between tiles.
pub fn apply_gaps(rects: &[Rect], area: Rect, gaps_in: i32, gaps_out: i32) -> Vec<Rect> {
    let half = gaps_in / 2;
    rects
        .iter()
        .map(|r| {
            let l = if r.x == area.x { gaps_out } else { half };
            let t = if r.y == area.y { gaps_out } else { half };
            let rr = if r.right() == area.right() { gaps_out } else { half };
            let b = if r.bottom() == area.bottom() { gaps_out } else { half };
            Rect::new(r.x + l, r.y + t, (r.w - l - rr).max(1), (r.h - t - b).max(1))
        })
        .collect()
}

/// Nearest tile in direction `dir` from tile `cur`.
pub fn neighbor(rects: &[Rect], cur: usize, dir: Dir) -> Option<usize> {
    let (cx, cy) = rects.get(cur)?.center();
    rects
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != cur)
        .filter_map(|(i, r)| {
            let (x, y) = r.center();
            let (along, across) = match dir {
                Dir::Left => (cx - x, (cy - y).abs()),
                Dir::Right => (x - cx, (cy - y).abs()),
                Dir::Up => (cy - y, (cx - x).abs()),
                Dir::Down => (y - cy, (cx - x).abs()),
            };
            (along > 0).then_some((i, along + across * 2))
        })
        .min_by_key(|(_, score)| *score)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: Rect = Rect { x: 0, y: 0, w: 1000, h: 600 };

    fn area_sum(r: &[Rect]) -> i64 {
        r.iter().map(|r| r.w as i64 * r.h as i64).sum()
    }

    #[test]
    fn dwindle_covers_area() {
        for n in 1..8 {
            let r = dwindle(A, n, 0.5);
            assert_eq!(r.len(), n);
            assert_eq!(area_sum(&r), 600_000, "n={n}");
        }
    }
    #[test]
    fn master_covers_area() {
        for n in 1..8 {
            assert_eq!(area_sum(&master(A, n, 0.55)), 600_000, "n={n}");
        }
    }
    #[test]
    fn gaps_shrink() {
        let r = apply_gaps(&[A], A, 5, 10);
        assert_eq!(r[0], Rect::new(10, 10, 980, 580));
    }
    #[test]
    fn nav() {
        let r = master(A, 3, 0.5);
        assert_eq!(neighbor(&r, 0, Dir::Right), Some(1));
        assert_eq!(neighbor(&r, 1, Dir::Down), Some(2));
        assert_eq!(neighbor(&r, 2, Dir::Left), Some(0));
        assert_eq!(neighbor(&r, 0, Dir::Left), None);
    }
}
