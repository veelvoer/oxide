//! The zoom-out overview (Super+Tab): the screen zooms out to a map of every workspace with its live windows.
//! Drag a window onto another workspace to move it there, click a workspace or a window to go to it, Esc or
//! Super+Tab to zoom back in.
use std::time::Duration;

use smithay::{
    backend::renderer::{
        element::{
            solid::SolidColorRenderElement,
            surface::WaylandSurfaceRenderElement,
            utils::{Relocate, RelocateRenderElement, RescaleRenderElement},
            AsRenderElements, Element, Id, Kind,
        },
        gles::{element::PixelShaderElement, GlesRenderer},
        utils::CommitCounter,
    },
    utils::{Logical, Physical, Point, Rectangle, Scale},
};

use crate::{render::OxideElement, shaders::ClippedSurface, state::Oxide};

const DURATION: f64 = 0.32;

pub struct Overview {
    pub mon: usize,
    /// when it started opening or closing (clock time)
    pub start: Duration,
    pub closing: bool,
    pub hover: Option<u32>,
    /// window being dragged: (index, grab offset inside the thumbnail as a fraction, pointer)
    pub drag: Option<(usize, (f64, f64), Point<f64, Logical>)>,
    pub press: Option<Point<f64, Logical>>,
    /// ids for the cell backgrounds and outlines (stable ids keep damage tracking cheap)
    pub ids: Vec<Id>,
}

fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn lerp_rect(a: Rectangle<f64, Logical>, b: Rectangle<f64, Logical>, t: f64) -> Rectangle<f64, Logical> {
    let l = |x: f64, y: f64| x + (y - x) * t;
    Rectangle::new((l(a.loc.x, b.loc.x), l(a.loc.y, b.loc.y)).into(), (l(a.size.w, b.size.w), l(a.size.h, b.size.h)).into())
}

impl Oxide {
    pub fn toggle_overview(&mut self) {
        let now: Duration = self.clock.now().into();
        match &mut self.overview {
            Some(o) if !o.closing => {
                // zoom back in from wherever the opening got to
                let p = self.overview_progress();
                let o = self.overview.as_mut().unwrap();
                o.closing = true;
                o.start = now.saturating_sub(Duration::from_secs_f64((1.0 - p) * DURATION));
                o.drag = None;
            }
            _ => {
                self.overview = Some(Overview { mon: self.focused_mon, start: now, closing: false, hover: None, drag: None, press: None, ids: vec![] });
                // windows of other workspaces are not drawn normally; make sure their buffers stay fresh enough
            }
        }
        self.dirty = true;
    }

    /// 0 = normal view, 1 = fully zoomed out.
    pub fn overview_progress(&self) -> f64 {
        let Some(o) = &self.overview else { return 0.0 };
        let now: Duration = self.clock.now().into();
        let t = now.saturating_sub(o.start).as_secs_f64() / DURATION;
        if o.closing { 1.0 - ease(t) } else { ease(t) }
    }

    pub fn overview_active_on(&self, mi: usize) -> bool {
        self.overview.as_ref().is_some_and(|o| o.mon == mi)
    }

    /// Workspaces shown in the overview: those with windows or on screen, plus a fresh empty one.
    fn overview_workspaces(&self, mi: usize) -> Vec<u32> {
        let cur = self.monitors.get(mi).map_or(1, |m| m.ws);
        let mut ids: Vec<u32> = self.wins.iter().map(|w| w.ws).chain(std::iter::once(cur)).filter(|&w| w < oxide_config::SPECIAL_WS).collect();
        ids.sort_unstable();
        ids.dedup();
        let next = (1..oxide_config::SPECIAL_WS).find(|i| !ids.contains(i)).unwrap_or(99);
        ids.push(next);
        ids
    }

    /// The grid cell of each workspace, in output-local logical coordinates.
    pub fn overview_cells(&self, mi: usize) -> Vec<(u32, Rectangle<f64, Logical>)> {
        let Some(geo) = self.monitors.get(mi).and_then(|m| self.space.output_geometry(&m.output)) else { return vec![] };
        let ids = self.overview_workspaces(mi);
        let n = ids.len().max(1);
        let cols = ((n as f64).sqrt().ceil() as usize).max(2).min(n.max(2));
        let rows = n.div_ceil(cols);
        let (w, h) = (geo.size.w as f64, geo.size.h as f64);
        let (mx, my, gap) = (w * 0.06, h * 0.09, 28.0);
        let cw = ((w - 2.0 * mx - gap * (cols as f64 - 1.0)) / cols as f64).min((h - 2.0 * my - gap * (rows as f64 - 1.0)) / rows as f64 * w / h);
        let ch = cw * h / w;
        let total_w = cols as f64 * cw + (cols as f64 - 1.0) * gap;
        let total_h = rows as f64 * ch + (rows as f64 - 1.0) * gap;
        let (x0, y0) = ((w - total_w) / 2.0, (h - total_h) / 2.0);
        ids.into_iter()
            .enumerate()
            .map(|(k, id)| {
                let (r, c) = (k / cols, k % cols);
                (id, Rectangle::new((x0 + c as f64 * (cw + gap), y0 + r as f64 * (ch + gap)).into(), (cw, ch).into()))
            })
            .collect()
    }

    /// Where workspace `ws` is drawn at progress `p` (the current one zooms out of the full screen).
    fn overview_cell_at(&self, mi: usize, ws: u32, cell: Rectangle<f64, Logical>, p: f64) -> Rectangle<f64, Logical> {
        let Some(geo) = self.monitors.get(mi).and_then(|m| self.space.output_geometry(&m.output)) else { return cell };
        if self.monitors[mi].ws == ws {
            lerp_rect(Rectangle::new((0.0, 0.0).into(), geo.size.to_f64()), cell, p)
        } else {
            cell
        }
    }

    /// The window under `pos` (output-local) in the overview: (window index, its thumbnail rect).
    fn overview_window_at(&self, mi: usize, pos: Point<f64, Logical>) -> Option<(usize, Rectangle<f64, Logical>)> {
        let geo = self.space.output_geometry(&self.monitors.get(mi)?.output)?;
        let k = |cell: Rectangle<f64, Logical>| cell.size.w / geo.size.w as f64;
        for (ws, cell) in self.overview_cells(mi) {
            let s = k(cell);
            // topmost first: focused, floating, then the rest
            let mut members: Vec<usize> = (0..self.wins.len()).filter(|&i| self.wins[i].ws == ws && self.wins[i].rect.size.w > 0).collect();
            members.sort_by_key(|&i| (!self.wins[i].floating, self.focus.as_ref() != Some(&self.wins[i].window)));
            for i in members {
                let r = self.wins[i].rect;
                let t = Rectangle::new(
                    (cell.loc.x + (r.loc.x - geo.loc.x) as f64 * s, cell.loc.y + (r.loc.y - geo.loc.y) as f64 * s).into(),
                    (r.size.w as f64 * s, r.size.h as f64 * s).into(),
                );
                if t.contains(pos) {
                    return Some((i, t));
                }
            }
        }
        None
    }

    fn overview_cell_under(&self, mi: usize, pos: Point<f64, Logical>) -> Option<u32> {
        self.overview_cells(mi).into_iter().find(|(_, c)| c.contains(pos)).map(|(ws, _)| ws)
    }

    /// Pointer moved (global position). Returns true when the overview took the event.
    pub fn overview_motion(&mut self, global: Point<f64, Logical>) -> bool {
        let Some(mi) = self.overview.as_ref().filter(|o| !o.closing).map(|o| o.mon) else { return false };
        let Some(geo) = self.space.output_geometry(&self.monitors[mi].output) else { return false };
        let pos = global - geo.loc.to_f64();
        let hover = self.overview_cell_under(mi, pos);
        let press = self.overview.as_ref().and_then(|o| o.press);
        // start dragging once the pointer moved a little with the button down
        if let Some(p0) = press {
            if self.overview.as_ref().is_some_and(|o| o.drag.is_none()) && ((pos.x - p0.x).powi(2) + (pos.y - p0.y).powi(2)).sqrt() > 8.0 {
                if let Some((i, t)) = self.overview_window_at(mi, p0) {
                    let grab = ((p0.x - t.loc.x) / t.size.w, (p0.y - t.loc.y) / t.size.h);
                    if let Some(o) = self.overview.as_mut() {
                        o.drag = Some((i, grab, pos));
                    }
                }
            }
        }
        if let Some(o) = self.overview.as_mut() {
            o.hover = hover;
            if let Some(d) = o.drag.as_mut() {
                d.2 = pos;
            }
        }
        self.dirty = true;
        true
    }

    /// Left button in the overview. Returns true when the overview took the event.
    pub fn overview_button(&mut self, pressed: bool) -> bool {
        let Some(mi) = self.overview.as_ref().filter(|o| !o.closing).map(|o| o.mon) else { return false };
        let Some(geo) = self.space.output_geometry(&self.monitors[mi].output) else { return false };
        let pos = self.pointer.current_location() - geo.loc.to_f64();
        if pressed {
            if let Some(o) = self.overview.as_mut() {
                o.press = Some(pos);
            }
            return true;
        }
        let (drag, _press) = match self.overview.as_mut() {
            Some(o) => (o.drag.take(), o.press.take()),
            None => (None, None),
        };
        let target = self.overview_cell_under(mi, pos);
        match (drag, target) {
            // dropped on another workspace: the window moves there (and the map shows it at once)
            (Some((i, _, _)), Some(ws)) => {
                if i < self.wins.len() && self.wins[i].ws != ws {
                    self.wins[i].ws = ws;
                    self.wins[i].fullscreen = false;
                    self.arrange();
                }
            }
            (Some(_), None) => {}
            // a click: go there (and focus the window that was clicked)
            (None, Some(ws)) => {
                let clicked = self.overview_window_at(mi, pos).map(|(i, _)| self.wins[i].window.clone());
                self.switch_workspace(ws);
                if let Some(w) = clicked {
                    self.focus_window(Some(w));
                }
                self.toggle_overview();
            }
            (None, None) => self.toggle_overview(),
        }
        self.dirty = true;
        true
    }

    /// Everything drawn on output `mi` while the overview is open (topmost first); `lower` is the wallpaper.
    pub fn overview_elements(&mut self, renderer: &mut GlesRenderer, mi: usize, scale: Scale<f64>, out: &mut Vec<OxideElement>) {
        let p = self.overview_progress();
        let now_closing_done = self.overview.as_ref().is_some_and(|o| o.closing) && p <= 0.0;
        if now_closing_done {
            self.overview = None;
            self.dirty = true;
            return;
        }
        if p < 1.0 {
            self.dirty = true; // keep animating
        }
        let Some(geo) = self.space.output_geometry(&self.monitors[mi].output) else { return };
        let accent = oxide_config::parse_color(&oxide_config::theme::palette(&self.config.theme).accent).unwrap_or([0.5, 0.6, 0.9, 1.0]);
        let cells = self.overview_cells(mi);
        let (hover, drag) = self.overview.as_ref().map_or((None, None), |o| (o.hover, o.drag));
        let cur = self.monitors[mi].ws;
        // stable ids: 3 per cell (outline top/bottom/left/right share one) + backdrop
        let need = cells.len() * 5 + 1;
        if let Some(o) = self.overview.as_mut() {
            while o.ids.len() < need {
                o.ids.push(Id::new());
            }
        }
        let ids = self.overview.as_ref().map(|o| o.ids.clone()).unwrap_or_default();
        let to_phys = |r: Rectangle<f64, Logical>| -> Rectangle<i32, Physical> { r.to_physical_precise_round(scale) };
        let radius = self.config.decoration.rounding.max(0) as f32;

        // the dragged window, on top of everything, following the pointer
        if let Some((i, grab, pos)) = drag {
            let r = self.wins[i].rect;
            let s = cells.first().map_or(0.2, |(_, c)| c.size.w / geo.size.w as f64);
            let size = (r.size.w as f64 * s, r.size.h as f64 * s);
            let t = Rectangle::new((pos.x - grab.0 * size.0, pos.y - grab.1 * size.1).into(), size.into());
            self.overview_window(renderer, i, t, scale, 0.92, radius * s as f32, out);
        }

        for (k, (ws, cell)) in cells.iter().enumerate() {
            let rect = self.overview_cell_at(mi, *ws, *cell, p);
            let s = rect.size.w / geo.size.w as f64;
            let alpha = if *ws == cur { 1.0 } else { p as f32 };
            // the windows of this workspace, scaled into the cell
            let mut members: Vec<usize> = (0..self.wins.len())
                .filter(|&i| self.wins[i].ws == *ws && self.wins[i].rect.size.w > 0 && drag.map(|d| d.0) != Some(i))
                .collect();
            members.sort_by_key(|&i| (!self.wins[i].floating, self.focus.as_ref() != Some(&self.wins[i].window)));
            for i in members {
                let r = self.wins[i].rect;
                let t = Rectangle::new(
                    (rect.loc.x + (r.loc.x - geo.loc.x) as f64 * s, rect.loc.y + (r.loc.y - geo.loc.y) as f64 * s).into(),
                    (r.size.w as f64 * s, r.size.h as f64 * s).into(),
                );
                self.overview_window(renderer, i, t, scale, alpha, radius * s as f32, out);
            }
            // Material Expressive: soft rounded tiles, drawn by a shader, with a rounded outline
            let rounded = self.config.theme.style == "expressive" && self.shaders.is_some();
            if rounded {
                let sh = self.shaders.as_ref().unwrap();
                let rr = (radius * 1.2 * s as f32 * 2.2).clamp(10.0, 34.0) * 1.0;
                let scale_f = scale.x as f32;
                let lr = Rectangle::<i32, Logical>::new((rect.loc.x.round() as i32, rect.loc.y.round() as i32).into(), (rect.size.w.round() as i32, rect.size.h.round() as i32).into());
                let pr = rr * scale_f;
                if (hover == Some(*ws) || *ws == cur) && p > 0.05 {
                    let c = if *ws == cur { accent } else { [1.0, 1.0, 1.0, 1.0] };
                    let a = p as f32 * if *ws == cur { 1.0 } else { 0.45 };
                    let g = 4;
                    let area = Rectangle::<i32, Logical>::new((lr.loc.x - g, lr.loc.y - g).into(), (lr.size.w + 2 * g, lr.size.h + 2 * g).into());
                    out.push(
                        PixelShaderElement::new(
                            sh.fill.clone(),
                            area,
                            None,
                            1.0,
                            vec![
                                crate::shaders::uniform_f("radius", pr + g as f32 * scale_f),
                                crate::shaders::uniform_f("border", 2.5 * scale_f),
                                crate::shaders::uniform_4f("color", [c[0] * a, c[1] * a, c[2] * a, a]),
                            ],
                            Kind::Unspecified,
                        )
                        .into(),
                    );
                }
                let a = 0.45 * p as f32;
                out.push(
                    PixelShaderElement::new(
                        sh.fill.clone(),
                        lr,
                        None,
                        1.0,
                        vec![crate::shaders::uniform_f("radius", pr), crate::shaders::uniform_f("border", 0.0), crate::shaders::uniform_4f("color", [0.0, 0.0, 0.0, a])],
                        Kind::Unspecified,
                    )
                    .into(),
                );
                continue;
            }
            // outline for the workspace under the pointer; the current one gets the accent
            if (hover == Some(*ws) || *ws == cur) && p > 0.05 {
                let c = if *ws == cur { accent } else { [1.0, 1.0, 1.0, 1.0] };
                let a = p as f32 * if *ws == cur { 1.0 } else { 0.45 };
                let col = [c[0] * a, c[1] * a, c[2] * a, a];
                let (x, y, w, h, t) = (rect.loc.x - 3.0, rect.loc.y - 3.0, rect.size.w + 6.0, rect.size.h + 6.0, 2.0);
                let edges = [(x, y, w, t), (x, y + h - t, w, t), (x, y, t, h), (x + w - t, y, t, h)];
                for (e, (ex, ey, ew, eh)) in edges.into_iter().enumerate() {
                    let r = to_phys(Rectangle::new((ex, ey).into(), (ew, eh).into()));
                    out.push(SolidColorRenderElement::new(ids[k * 5 + e].clone(), r, CommitCounter::default(), col, Kind::Unspecified).into());
                }
            }
            // the cell itself: a dark glass panel the windows sit on
            let a = 0.45 * p as f32;
            out.push(SolidColorRenderElement::new(ids[k * 5 + 4].clone(), to_phys(rect), CommitCounter::default(), [0.0, 0.0, 0.0, a], Kind::Unspecified).into());
        }
        // dim the wallpaper a little
        let a = 0.35 * p as f32;
        out.push(SolidColorRenderElement::new(ids[need - 1].clone(), to_phys(Rectangle::new((0.0, 0.0).into(), geo.size.to_f64())), CommitCounter::default(), [0.0, 0.0, 0.0, a], Kind::Unspecified).into());
    }

    /// One window drawn into `target` (output-local logical rect of its outer frame).
    #[allow(clippy::too_many_arguments)]
    fn overview_window(&mut self, renderer: &mut GlesRenderer, i: usize, target: Rectangle<f64, Logical>, scale: Scale<f64>, alpha: f32, radius: f32, out: &mut Vec<OxideElement>) {
        let w = &self.wins[i];
        let window = w.window.clone();
        let b = w.border.max(0) as f64;
        let r = w.rect;
        let inner = ((r.size.w as f64 - 2.0 * b).max(1.0), (r.size.h as f64 - 2.0 * b).max(1.0));
        let k = target.size.w / r.size.w.max(1) as f64;
        let geo_off = window.geometry().loc;
        // render at the origin, then scale and move into place
        let render_loc: Point<i32, Physical> = (Point::<i32, Logical>::from((0, 0)) - geo_off).to_physical_precise_round(scale);
        let dest: Point<i32, Physical> = Point::<f64, Logical>::from((target.loc.x + b * k, target.loc.y + b * k)).to_physical_precise_round(scale);
        let els: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = window.render_elements(renderer, render_loc, scale, alpha);
        let root_id = crate::wm::surface_of(&window).map(|s| Id::from_wayland_resource(&s));
        let wg = window.geometry();
        let geo = (wg.loc.x as f32, wg.loc.y as f32, wg.size.w as f32, wg.size.h as f32);
        let _ = inner;
        for e in els {
            let clip = match (&self.shaders, root_id.as_ref()) {
                (Some(sh), Some(rid)) if radius > 0.5 && e.id() == rid => Some(sh.clip.clone()),
                _ => None,
            };
            match clip {
                Some(pr) => out.push(
                    RelocateRenderElement::from_element(
                        RescaleRenderElement::from_element(ClippedSurface::new(e, pr, radius / k as f32, geo), (0, 0).into(), Scale::from(k)),
                        dest,
                        Relocate::Relative,
                    )
                    .into(),
                ),
                None => out.push(RelocateRenderElement::from_element(RescaleRenderElement::from_element(e, (0, 0).into(), Scale::from(k)), dest, Relocate::Relative).into()),
            }
        }
    }
}
