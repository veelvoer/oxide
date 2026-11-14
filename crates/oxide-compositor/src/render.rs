//! Backend-independent construction of the element list for one output.
use std::{sync::Mutex, time::Duration};

use oxide_config::parse_color;
use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement,
            solid::SolidColorRenderElement,
            texture::TextureRenderElement,
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
            utils::{Relocate, RelocateRenderElement, RescaleRenderElement},
            AsRenderElements, Element, Id, Kind,
        },
        gles::{element::PixelShaderElement, GlesRenderer, GlesTexture},
        Renderer, Texture,
    },
    desktop::{layer_map_for_output, Window},
    input::pointer::{CursorImageAttributes, CursorImageStatus},
    output::Output,
    render_elements,
    utils::{IsAlive, Logical, Physical, Point, Rectangle, Scale, Transform},
    wayland::{compositor::with_states, shell::wlr_layer::Layer},
};

use crate::{
    anim::{popin_scale, progress},
    shaders::{uniform_2f, uniform_4f, uniform_f, ClippedSurface, ClippedTexture},
    state::Oxide,
};

static CLEAR_RGB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x0011111a);

/// Background where nothing is drawn (before the shell's wallpaper is up, or when it restarts): the theme's color.
pub fn clear() -> [f32; 4] {
    let v = CLEAR_RGB.load(std::sync::atomic::Ordering::Relaxed);
    [((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0, 1.0]
}

pub fn set_clear(hex: &str) {
    if let Some(c) = oxide_config::parse_color(hex) {
        let b = |x: f32| (x * 255.0).round() as u32;
        CLEAR_RGB.store((b(c[0]) << 16) | (b(c[1]) << 8) | b(c[2]), std::sync::atomic::Ordering::Relaxed);
    }
}

render_elements! {
    pub OxideElement<=GlesRenderer>;
    Solid = SolidColorRenderElement,
    Cursor = MemoryRenderBufferRenderElement<GlesRenderer>,
    Surface = WaylandSurfaceRenderElement<GlesRenderer>,
    Clipped = ClippedSurface,
    Rescaled = RelocateRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
    RescaledClip = RelocateRenderElement<RescaleRenderElement<ClippedSurface>>,
    ClippedTex = ClippedTexture,
    Pixel = PixelShaderElement,
}

type Surf = WaylandSurfaceRenderElement<GlesRenderer>;

/// Blur is drawn under windows that are translucent (or that a rule asks for).
fn wants_blur(base_alpha: f32, rule: Option<bool>) -> bool {
    match rule {
        Some(b) => b,
        None => base_alpha < 0.999,
    }
}

fn premul(c: [f32; 4], a: f32) -> [f32; 4] {
    let a = c[3] * a;
    [c[0] * a, c[1] * a, c[2] * a, a]
}

impl Oxide {
    /// Elements for `output`, topmost first: cursor, upper layers, windows, lower layers.
    pub fn build_elements(&mut self, renderer: &mut GlesRenderer, output: &Output) -> Vec<OxideElement> {
        // animations advance to the moment of this frame, so a frame never repeats the last one (the timer alone beats against the screen)
        if self.animations_running() {
            self.tick_animations();
        }
        if !self.shaders_tried {
            self.shaders_tried = true;
            self.shaders = crate::shaders::Shaders::compile(renderer);
        }
        let mut out: Vec<OxideElement> = vec![];
        let Some(geo) = self.space.output_geometry(output) else { return out };
        let scale = Scale::from(output.current_scale().fractional_scale());

        if self.sampling_bg {
            let layer_map = layer_map_for_output(output);
            return layer_map
                .layers()
                .rev()
                .filter(|l| l.layer() == Layer::Background)
                .filter_map(|l| layer_map.layer_geometry(l).map(|g| (g.loc, l)))
                .flat_map(|(loc, l)| AsRenderElements::<GlesRenderer>::render_elements::<Surf>(l, renderer, loc.to_physical_precise_round(scale), scale, 1.0))
                .map(OxideElement::from)
                .collect();
        }

        self.cursor_elements(renderer, output, geo, scale, &mut out);

        if self.locked() {
            // only the lock surface (or black) is visible while locked
            if let Some(ls) = self.lock_surface_for(output) {
                let els: Vec<Surf> =
                    render_elements_from_surface_tree(renderer, ls.wl_surface(), (0, 0), scale, 1.0, Kind::Unspecified);
                out.extend(els.into_iter().map(OxideElement::from));
            }
            return out;
        }

        let layer_map = layer_map_for_output(output);
        let fullscreen = self.fullscreen_on(output);
        let layer_els = |renderer: &mut GlesRenderer, upper: bool| -> Vec<OxideElement> {
            layer_map
                .layers()
                .rev()
                .filter(|l| matches!(l.layer(), Layer::Overlay | Layer::Top) == upper)
                // the bar lives on the top layer: it steps aside for a fullscreen window (popups stay)
                .filter(|l| !(fullscreen.is_some() && l.layer() == Layer::Top))
                .filter_map(|l| layer_map.layer_geometry(l).map(|g| (g.loc, l)))
                .flat_map(|(loc, l)| {
                    AsRenderElements::<GlesRenderer>::render_elements::<Surf>(
                        l,
                        renderer,
                        loc.to_physical_precise_round(scale),
                        scale,
                        1.0,
                    )
                })
                .map(OxideElement::from)
                .collect()
        };
        out.extend(layer_els(renderer, true));
        drop(layer_map);

        // X11 menus and tooltips sit above ordinary windows
        for w in self.or_windows.iter().rev() {
            let Some(x) = w.x11_surface() else { continue };
            let loc = x.geometry().loc - w.geometry().loc - geo.loc;
            let els: Vec<Surf> = w.render_elements(renderer, loc.to_physical_precise_round(scale), scale, 1.0);
            out.extend(els.into_iter().map(OxideElement::from));
        }

        let layer_map = layer_map_for_output(output);
        let lower: Vec<OxideElement> = layer_map
            .layers()
            .rev()
            .filter(|l| matches!(l.layer(), Layer::Background | Layer::Bottom))
            .filter_map(|l| layer_map.layer_geometry(l).map(|g| (g.loc, l)))
            .flat_map(|(loc, l)| {
                AsRenderElements::<GlesRenderer>::render_elements::<Surf>(
                    l,
                    renderer,
                    loc.to_physical_precise_round(scale),
                    scale,
                    1.0,
                )
            })
            .map(OxideElement::from)
            .collect();
        let list = self.render_list(output);
        self.closing_elements(renderer, output, geo, scale, &mut out);
        let blur_tex = self.update_blur(renderer, output, &lower);
        for (wi, dx) in list.into_iter().rev() {
            self.window_elements(renderer, wi, dx, geo, scale, blur_tex.as_ref(), &mut out);
        }
        let _ = fullscreen;

        out.extend(lower);
        out
    }

    /// Refresh the blurred background of `output` if any visible window wants blur.
    fn update_blur(&mut self, renderer: &mut GlesRenderer, output: &Output, lower: &[OxideElement]) -> Option<GlesTexture> {
        let name = output.name();
        let dec = self.config.decoration.clone();
        let bsh = self.shaders.as_ref()?.blur.clone();
        let need = dec.blur.enabled
            && bsh.is_some()
            && self.wins.iter().any(|w| {
                let shown = self.monitors.iter().any(|m| &m.output == output && m.ws == w.ws);
                let base = if self.focus.as_ref() == Some(&w.window) { dec.active_opacity } else { dec.inactive_opacity };
                shown && wants_blur(base * w.opacity.unwrap_or(1.0), w.blur)
            });
        if !need {
            self.blurs.remove(&name);
            return None;
        }
        let bsh = bsh?;
        let size = output.current_mode()?.size;
        let scale = output.current_scale().fractional_scale();
        let passes = dec.blur.passes.clamp(1, 6);
        let stale = self.blurs.get(&name).map_or(true, |b| b.size != size || b.passes != passes);
        if stale {
            let b = crate::blur::Blur::new(renderer, size, passes, scale)?;
            self.blurs.insert(name.clone(), b);
        }
        let offset = (dec.blur.size as f32 / 2.0).max(1.0);
        let blur = self.blurs.get_mut(&name)?;
        if blur.update(renderer, lower, &bsh.down, &bsh.up, offset, clear()) {
            for w in &mut self.wins {
                w.blur_id = Id::new();
            }
        }
        Some(blur.texture())
    }

    pub fn fullscreen_on(&self, output: &Output) -> Option<usize> {
        let m = self.monitors.iter().find(|m| &m.output == output)?;
        self.wins.iter().position(|w| w.fullscreen && w.ws == m.ws && w.visible)
    }

    /// Windows to draw on `output`, bottom to top, with their horizontal slide offset in logical px.
    fn render_list(&self, output: &Output) -> Vec<(usize, i32)> {
        let Some(mi) = self.monitors.iter().position(|m| &m.output == output) else { return vec![] };
        let width = self.space.output_geometry(output).map_or(0, |g| g.size.w);
        let slide = self.ws_anim.as_ref().filter(|a| a.mon == mi).map(|a| {
            let now: Duration = self.clock.now().into();
            let anims = &self.config.animations;
            let (p, _) = progress(anims, &anims.workspaces, now.saturating_sub(a.start).as_secs_f64());
            (a.old_ws, a.dir, p)
        });
        let mut list = vec![];
        if let Some((old_ws, dir, e)) = slide {
            let off = (-(dir as f64) * width as f64 * e) as i32; // e may overshoot with springs
            for (i, w) in self.wins.iter().enumerate() {
                if w.ws == old_ws && !w.visible && w.cur.w > 0.0 {
                    list.push((i, off));
                }
            }
        }
        let new_off = slide.map_or(0, |(_, dir, e)| (dir as f64 * width as f64 * (1.0 - e)) as i32);
        if slide.is_none() {
            if let Some(i) = self.fullscreen_on(output) {
                return vec![(i, 0)];
            }
        }
        for w in self.space.elements_for_output(output) {
            if let Some(i) = self.win_idx(w) {
                list.push((i, new_off));
            }
        }
        list
    }

    fn cursor_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        output: &Output,
        geo: Rectangle<i32, Logical>,
        scale: Scale<f64>,
        out: &mut Vec<OxideElement>,
    ) {
        let pos = self.pointer.current_location();
        if self.hide_cursor || !geo.to_f64().contains(pos) {
            return;
        }
        if let CursorImageStatus::Surface(s) = &self.cursor_status {
            if !s.alive() {
                self.cursor_status = CursorImageStatus::default_named();
            }
        }
        let rel = pos - geo.loc.to_f64();
        match &self.cursor_status {
            CursorImageStatus::Hidden => {}
            CursorImageStatus::Named(icon) => {
                let now: Duration = self.clock.now().into();
                let s = output.current_scale().integer_scale().max(1) as u32;
                let frame = self.cursor.get(icon.name(), s, now);
                let hot = Point::from((frame.hotspot.0 as f64 / s as f64, frame.hotspot.1 as f64 / s as f64));
                let loc: Point<f64, Physical> = (rel - hot).to_physical(scale);
                if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(
                    renderer, loc, &frame.buffer, None, None, None, Kind::Cursor,
                ) {
                    out.push(e.into());
                }
            }
            CursorImageStatus::Surface(surface) => {
                let hotspot = with_states(surface, |s| {
                    s.data_map
                        .get::<Mutex<CursorImageAttributes>>()
                        .map(|a| a.lock().unwrap().hotspot)
                        .unwrap_or_default()
                });
                let loc = (rel - hotspot.to_f64()).to_physical(scale).to_i32_round();
                let els: Vec<Surf> = render_elements_from_surface_tree(renderer, surface, loc, scale, 1.0, Kind::Cursor);
                out.extend(els.into_iter().map(OxideElement::from));
            }
        }
    }

    /// Border gradient uniforms for the given mix (0 = inactive color, 1 = active gradient).
    fn border_uniforms(&self, mix: f32, fade: f32, radius: i32, b: i32) -> Vec<smithay::backend::renderer::gles::Uniform<'static>> {
        let g = &self.config.general;
        let fix = |c: [f32; 4]| premul(c, fade);
        let cols: Vec<[f32; 4]> =
            g.active_border.iter().filter_map(|s| parse_color(s)).take(4).collect();
        let cols = if cols.is_empty() { vec![[1.0; 4]] } else { cols };
        let last = *cols.last().unwrap();
        let col = |i: usize| fix(*cols.get(i).unwrap_or(&last));
        vec![
            uniform_f("radius", radius as f32),
            uniform_f("border", b as f32),
            uniform_f("mixv", mix),
            uniform_f("angle", g.border_angle.to_radians()),
            uniform_f("ncols", cols.len() as f32),
            uniform_4f("c0", col(0)),
            uniform_4f("c1", col(1)),
            uniform_4f("c2", col(2)),
            uniform_4f("c3", col(3)),
            uniform_4f("inactive", fix(parse_color(&g.inactive_border).unwrap_or([0.3, 0.3, 0.4, 0.7]))),
        ]
    }

    /// Everything belonging to one window, topmost first: dim, surfaces, border, blur, shadow.
    fn window_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        wi: usize,
        dx: i32,
        ogeo: Rectangle<i32, Logical>,
        scale: Scale<f64>,
        blur_tex: Option<&GlesTexture>,
        out: &mut Vec<OxideElement>,
    ) {
        let cfg = self.config.clone();
        let dec = &cfg.decoration;
        let focused = self.focus.as_ref() == Some(&self.wins[wi].window);
        let border_uniforms = {
            let w = &self.wins[wi];
            self.border_uniforms(w.border_mix, w.fade, if w.fullscreen { 0 } else { dec.rounding.max(0) }, if w.fullscreen { 0 } else { w.border })
        };
        let w = &mut self.wins[wi];
        let fullscreen = w.fullscreen;

        // `cur` is where the window is drawn now; `target` is where the client believes it is
        let outer = w.cur.rounded();
        let target = w.rect;
        let b = if fullscreen { 0 } else { w.border };
        let outer_loc: Point<i32, Logical> = (outer.loc.x + dx - ogeo.loc.x, outer.loc.y - ogeo.loc.y).into();
        let inner_loc: Point<i32, Logical> = (outer_loc.x + b, outer_loc.y + b).into();
        let inner_size: smithay::utils::Size<i32, Logical> = ((outer.size.w - 2 * b).max(1), (outer.size.h - 2 * b).max(1)).into();
        let radius = if fullscreen { 0 } else { dec.rounding.max(0) };

        let base = if focused { dec.active_opacity } else { dec.inactive_opacity };
        let alpha = (w.fade * base * w.opacity.unwrap_or(1.0)).clamp(0.0, 1.0);

        // dim overlay for unfocused windows
        if !focused && dec.dim_inactive > 0.0 && !fullscreen {
            w.dim.update((inner_size.w, inner_size.h), [0.0, 0.0, 0.0, dec.dim_inactive]);
            let loc: Point<i32, Physical> = inner_loc.to_physical_precise_round(scale);
            out.push(SolidColorRenderElement::from_buffer(&w.dim, loc, scale, w.fade, Kind::Unspecified).into());
        }

        // surfaces: rendered at their final place, then moved/stretched to the animated rect
        let window: Window = w.window.clone();
        let geo_off = window.geometry().loc;
        let t_inner_loc: Point<i32, Logical> = (target.loc.x + b - ogeo.loc.x, target.loc.y + b - ogeo.loc.y).into();
        let t_inner_size = ((target.size.w - 2 * b).max(1) as f64, (target.size.h - 2 * b).max(1) as f64);
        let render_loc: Point<i32, Physical> = (t_inner_loc - geo_off).to_physical_precise_round(scale);
        let origin: Point<i32, Physical> = t_inner_loc.to_physical_precise_round(scale);
        let shift: Point<i32, Physical> = (inner_loc - t_inner_loc).to_physical_precise_round(scale);
        let (sx, sy) = (inner_size.w as f64 / t_inner_size.0, inner_size.h as f64 / t_inner_size.1);
        let transformed = shift.x != 0 || shift.y != 0 || (sx - 1.0).abs() > 1e-4 || (sy - 1.0).abs() > 1e-4;
        let els: Vec<Surf> = window.render_elements(renderer, render_loc, scale, alpha);
        let root_id = crate::wm::surface_of(&window).map(|s| Id::from_wayland_resource(&s));
        let wg = window.geometry();
        let geo = (wg.loc.x as f32, wg.loc.y as f32, wg.size.w as f32, wg.size.h as f32);
        let inner_radius = (radius - b).max(0) as f32;
        for e in els {
            let clip = match (&self.shaders, root_id.as_ref()) {
                (Some(sh), Some(rid)) if radius > 0 && e.id() == rid => Some(sh.clip.clone()),
                _ => None,
            };
            let wrap = |e| RelocateRenderElement::from_element(RescaleRenderElement::from_element(e, origin, Scale::from((sx, sy))), shift, Relocate::Relative);
            match (clip, transformed) {
                (Some(p), false) => out.push(ClippedSurface::new(e, p, inner_radius, geo).into()),
                (Some(p), true) => out.push(wrap(ClippedSurface::new(e, p, inner_radius, geo)).into()),
                (None, false) => out.push(e.into()),
                (None, true) => out.push(RelocateRenderElement::from_element(RescaleRenderElement::from_element(e, origin, Scale::from((sx, sy))), shift, Relocate::Relative).into()),
            }
        }

        if fullscreen {
            return;
        }

        // border (gradient, fades between inactive and active color)
        if b > 0 {
            match &self.shaders {
                Some(sh) => {
                    let area = Rectangle::new(outer_loc, outer.size);
                    let key = (area, border_uniforms.iter().map(|u| format!("{:?}", u.value)).collect::<Vec<_>>());
                    match &mut w.border_el {
                        Some((el, k)) => {
                            el.resize(area, None);
                            if *k != key {
                                el.update_uniforms(border_uniforms);
                                *k = key;
                            }
                        }
                        None => {
                            let el = PixelShaderElement::new(sh.border.clone(), area, None, 1.0, border_uniforms, Kind::Unspecified);
                            w.border_el = Some((el, key));
                        }
                    }
                    if let Some((el, _)) = &w.border_el {
                        out.push(el.clone().into());
                    }
                }
                None => {
                    // shader failed to compile: plain four-strip border
                    let color = parse_color(if focused { cfg.general.active_border.first().map_or("#ffffff", |s| s.as_str()) } else { &cfg.general.inactive_border }).unwrap_or([1.0; 4]);
                    let (rw, rh) = (outer.size.w, outer.size.h);
                    let sizes = [(rw, b), (rw, b), (b, (rh - 2 * b).max(0)), (b, (rh - 2 * b).max(0))];
                    let locs = [
                        (outer_loc.x, outer_loc.y),
                        (outer_loc.x, outer_loc.y + rh - b),
                        (outer_loc.x, outer_loc.y + b),
                        (outer_loc.x + rw - b, outer_loc.y + b),
                    ];
                    for ((buf, size), (x, y)) in w.borders.iter_mut().zip(sizes).zip(locs) {
                        buf.update(size, color);
                        let loc: Point<i32, Physical> = Point::<i32, Logical>::from((x, y)).to_physical_precise_round(scale);
                        out.push(SolidColorRenderElement::from_buffer(buf, loc, scale, w.fade, Kind::Unspecified).into());
                    }
                }
            }
        }

        // blurred background (xray): sits under the window, over the shadow
        if let (Some(tex), Some(bsh)) = (blur_tex, self.shaders.as_ref().and_then(|s| s.blur.clone())) {
            let base_alpha = if focused { dec.active_opacity } else { dec.inactive_opacity } * w.opacity.unwrap_or(1.0);
            if wants_blur(base_alpha, w.blur) {
                let phys: Rectangle<i32, Physical> =
                    Rectangle::<i32, Logical>::new(inner_loc, inner_size).to_physical_precise_round(scale);
                let ts = tex.size();
                let full = ogeo.size.to_f64().to_physical(scale);
                let (rx, ry) = (ts.w as f64 / full.w, ts.h as f64 / full.h);
                let src_px = Rectangle::<f64, Logical>::new(
                    (phys.loc.x as f64 * rx, phys.loc.y as f64 * ry).into(),
                    (phys.size.w as f64 * rx, phys.size.h as f64 * ry).into(),
                );
                let uv_origin = ((src_px.loc.x / ts.w as f64) as f32, (src_px.loc.y / ts.h as f64) as f32);
                let uv_size = ((src_px.size.w / ts.w as f64) as f32, (src_px.size.h / ts.h as f64) as f32);
                let el = TextureRenderElement::from_static_texture(
                    w.blur_id.clone(),
                    renderer.context_id(),
                    phys.loc.to_f64(),
                    tex.clone(),
                    1,
                    Transform::Normal,
                    Some(w.fade),
                    Some(src_px),
                    Some(inner_size),
                    None,
                    Kind::Unspecified,
                );
                out.push(ClippedTexture::new(el, bsh.clip_sub.clone(), inner_radius, uv_origin, uv_size).into());
            }
        }

        // shadow
        if dec.shadow.enabled {
            if let Some(sh) = &self.shaders {
                let range = dec.shadow.range as i32;
                let area = Rectangle::new(
                    (outer_loc.x - range, outer_loc.y - range).into(),
                    (outer.size.w + 2 * range, outer.size.h + 2 * range).into(),
                );
                let col = premul(parse_color(&dec.shadow.color).unwrap_or([0.0, 0.0, 0.0, 0.5]), w.fade);
                let uniforms = vec![
                    uniform_2f("win_size", outer.size.w as f32, outer.size.h as f32),
                    uniform_f("radius", radius as f32),
                    uniform_f("range", range as f32),
                    uniform_4f("color", col),
                ];
                let key = (area, uniforms.iter().map(|u| format!("{:?}", u.value)).collect::<Vec<_>>());
                match &mut w.shadow_el {
                    Some((el, k)) => {
                        el.resize(area, None);
                        if *k != key {
                            el.update_uniforms(uniforms);
                            *k = key;
                        }
                    }
                    None => {
                        let el = PixelShaderElement::new(sh.shadow.clone(), area, None, 1.0, uniforms, Kind::Unspecified);
                        w.shadow_el = Some((el, key));
                    }
                }
                if let Some((el, _)) = &w.shadow_el {
                    out.push(el.clone().into());
                }
            }
        }
    }

    /// Windows that were just closed: fade and shrink their last frame.
    fn closing_elements(&mut self, renderer: &mut GlesRenderer, output: &Output, ogeo: Rectangle<i32, Logical>, scale: Scale<f64>, out: &mut Vec<OxideElement>) {
        let Some(sh) = self.shaders.clone() else { return };
        let Some(ws) = self.monitors.iter().find(|m| &m.output == output).map(|m| m.ws) else { return };
        let now: Duration = self.clock.now().into();
        let anims = self.config.animations.clone();
        let dec = self.config.decoration.clone();
        for c in &self.closing {
            if c.ws != ws {
                continue;
            }
            let e = now.saturating_sub(c.start).as_secs_f64();
            let (p, _) = progress(&anims, &anims.windows_out, e);
            let (f, _) = progress(&anims, &anims.fade_out, e);
            let fade = (1.0 - f).clamp(0.0, 1.0) as f32;
            let sc = popin_scale(&anims.windows_out.style).map_or(1.0, |s0| 1.0 - (1.0 - s0) * p);
            let outer = c.rect.scaled(sc).rounded();
            let b = c.border;
            let outer_loc: Point<i32, Logical> = (outer.loc.x - ogeo.loc.x, outer.loc.y - ogeo.loc.y).into();
            let inner_loc: Point<i32, Logical> = (outer_loc.x + b, outer_loc.y + b).into();
            let inner_size: smithay::utils::Size<i32, Logical> = ((outer.size.w - 2 * b).max(1), (outer.size.h - 2 * b).max(1)).into();
            let inner_radius = (c.radius - b).max(0) as f32;

            let phys_loc: Point<i32, Physical> = inner_loc.to_physical_precise_round(scale);
            let el = TextureRenderElement::from_static_texture(
                c.id.clone(),
                renderer.context_id(),
                phys_loc.to_f64(),
                c.texture.clone(),
                1,
                Transform::Normal,
                Some(fade * dec.inactive_opacity.max(0.3)),
                None,
                Some(inner_size),
                None,
                Kind::Unspecified,
            );
            if let Some(bsh) = &sh.blur {
                out.push(ClippedTexture::new(el, bsh.clip_sub.clone(), inner_radius, (0.0, 0.0), (1.0, 1.0)).into());
            }
            if b > 0 {
                let area = Rectangle::new(outer_loc, outer.size);
                let u = self.border_uniforms(c.border_mix, fade, c.radius, b);
                out.push(PixelShaderElement::new(sh.border.clone(), area, None, 1.0, u, Kind::Unspecified).into());
            }
            if dec.shadow.enabled {
                let range = dec.shadow.range as i32;
                let area = Rectangle::new(
                    (outer_loc.x - range, outer_loc.y - range).into(),
                    (outer.size.w + 2 * range, outer.size.h + 2 * range).into(),
                );
                let col = premul(parse_color(&dec.shadow.color).unwrap_or([0.0, 0.0, 0.0, 0.5]), fade);
                let u = vec![
                    uniform_2f("win_size", outer.size.w as f32, outer.size.h as f32),
                    uniform_f("radius", c.radius as f32),
                    uniform_f("range", range as f32),
                    uniform_4f("color", col),
                ];
                out.push(PixelShaderElement::new(sh.shadow.clone(), area, None, 1.0, u, Kind::Unspecified).into());
            }
        }
    }
}
