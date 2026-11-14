//! Custom GLES shaders: rounded borders, shadows, rounded surface clipping.
use smithay::{
    backend::renderer::{
        element::{
            surface::WaylandSurfaceRenderElement, texture::TextureRenderElement, Element, Id, Kind, RenderElement,
            UnderlyingStorage,
        },
        gles::{
            GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName,
            UniformType,
        },
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{Buffer, Physical, Point, Rectangle, Scale, Transform},
};
use tracing::warn;

const SD: &str = r#"
float sd_round_rect(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}
"#;

const PRECISION: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
"#;

fn border_src() -> String {
    format!(
        "{PRECISION}
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;
uniform float radius;
uniform float border;
uniform float mixv;
uniform float angle;
uniform float ncols;
uniform vec4 c0;
uniform vec4 c1;
uniform vec4 c2;
uniform vec4 c3;
uniform vec4 inactive;
{SD}
vec4 gradient(float t) {{
    if (ncols < 1.5) return c0;
    float s = t * (ncols - 1.0);
    vec4 a = c0;
    vec4 b = c1;
    float f = s;
    if (s >= 1.0) {{ a = c1; b = c2; f = s - 1.0; }}
    if (s >= 2.0) {{ a = c2; b = c3; f = s - 2.0; }}
    return mix(a, b, clamp(f, 0.0, 1.0));
}}
void main() {{
    vec2 p = v_coords * size - size * 0.5;
    float d_out = sd_round_rect(p, size * 0.5, radius);
    float d_in = sd_round_rect(p, size * 0.5 - border, max(radius - border, 0.0));
    float a = clamp(0.5 - d_out, 0.0, 1.0) * (1.0 - clamp(0.5 - d_in, 0.0, 1.0));
    vec2 dir = vec2(cos(angle), -sin(angle));
    float ext = max(abs(size.x * dir.x) + abs(size.y * dir.y), 1.0);
    float t = clamp(dot(p, dir) / ext + 0.5, 0.0, 1.0);
    vec4 col = mix(inactive, gradient(t), mixv);
    gl_FragColor = col * (a * alpha);
}}"
    )
}

fn shadow_src() -> String {
    format!(
        "{PRECISION}
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;
uniform vec2 win_size;
uniform float radius;
uniform float range;
uniform vec4 color;
{SD}
void main() {{
    vec2 p = v_coords * size - size * 0.5;
    float d = sd_round_rect(p, win_size * 0.5, radius);
    float a = 1.0 - smoothstep(0.0, range, d);
    a = a * a;
    if (d < 0.0) a = 0.0;
    gl_FragColor = color * (a * alpha);
}}"
    )
}

fn clip_src() -> String {
    format!(
        "//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
{PRECISION}
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
uniform vec2 size;
uniform vec4 geo;
uniform float radius;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
{SD}
void main() {{
    vec4 color = texture2D(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0) * alpha;
#else
    color = color * alpha;
#endif
    // round the window's own rectangle: apps with client-side shadow margins draw outside it
    vec2 p = v_coords * size - (geo.xy + geo.zw * 0.5);
    float d = sd_round_rect(p, geo.zw * 0.5, radius);
    color *= clamp(0.5 - d, 0.0, 1.0);
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}}"
    )
}

fn tex_header() -> String {
    format!(
        "//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
{PRECISION}
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
"
    )
}

fn blur_down_src() -> String {
    format!(
        "{}
uniform vec2 halfpixel;
uniform float offset;
void main() {{
    vec2 uv = v_coords;
    vec4 sum = texture2D(tex, uv) * 4.0;
    sum += texture2D(tex, uv - halfpixel * offset);
    sum += texture2D(tex, uv + halfpixel * offset);
    sum += texture2D(tex, uv + vec2(halfpixel.x, -halfpixel.y) * offset);
    sum += texture2D(tex, uv - vec2(halfpixel.x, -halfpixel.y) * offset);
    gl_FragColor = sum / 8.0 * alpha;
}}",
        tex_header()
    )
}

fn blur_up_src() -> String {
    format!(
        "{}
uniform vec2 halfpixel;
uniform float offset;
void main() {{
    vec2 uv = v_coords;
    vec4 sum = texture2D(tex, uv + vec2(-halfpixel.x * 2.0, 0.0) * offset);
    sum += texture2D(tex, uv + vec2(-halfpixel.x, halfpixel.y) * offset) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, halfpixel.y * 2.0) * offset);
    sum += texture2D(tex, uv + vec2(halfpixel.x, halfpixel.y) * offset) * 2.0;
    sum += texture2D(tex, uv + vec2(halfpixel.x * 2.0, 0.0) * offset);
    sum += texture2D(tex, uv + vec2(halfpixel.x, -halfpixel.y) * offset) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, -halfpixel.y * 2.0) * offset);
    sum += texture2D(tex, uv + vec2(-halfpixel.x, -halfpixel.y) * offset) * 2.0;
    gl_FragColor = sum / 12.0 * alpha;
}}",
        tex_header()
    )
}

/// Rounded-rect mask for a texture drawn through a sub-region (`uv_origin`/`uv_size` in texture space).
fn clip_sub_src() -> String {
    format!(
        "{}
uniform vec2 size;
uniform float radius;
uniform vec2 uv_origin;
uniform vec2 uv_size;
{SD}
void main() {{
    vec4 color = texture2D(tex, v_coords) * alpha;
    vec2 local = (v_coords - uv_origin) / uv_size;
    vec2 p = local * size - size * 0.5;
    float d = sd_round_rect(p, size * 0.5, radius);
    gl_FragColor = color * clamp(0.5 - d, 0.0, 1.0);
}}",
        tex_header()
    )
}

#[derive(Clone)]
pub struct BlurShaders {
    pub down: GlesTexProgram,
    pub up: GlesTexProgram,
    pub clip_sub: GlesTexProgram,
}

#[derive(Clone)]
pub struct Shaders {
    pub border: GlesPixelProgram,
    pub shadow: GlesPixelProgram,
    pub clip: GlesTexProgram,
    pub blur: Option<BlurShaders>,
}

impl Shaders {
    pub fn compile(r: &mut GlesRenderer) -> Option<Self> {
        let f = |n: &'static str| UniformName::new(n, UniformType::_1f);
        let v2 = |n: &'static str| UniformName::new(n, UniformType::_2f);
        let v4 = |n: &'static str| UniformName::new(n, UniformType::_4f);
        let border = r
            .compile_custom_pixel_shader(
                border_src(),
                &[f("radius"), f("border"), f("mixv"), f("angle"), f("ncols"), v4("c0"), v4("c1"), v4("c2"), v4("c3"), v4("inactive")],
            )
            .map_err(|e| warn!("border shader: {e}"))
            .ok()?;
        let shadow = r
            .compile_custom_pixel_shader(shadow_src(), &[v2("win_size"), f("radius"), f("range"), v4("color")])
            .map_err(|e| warn!("shadow shader: {e}"))
            .ok()?;
        let clip = r
            .compile_custom_texture_shader(clip_src(), &[v2("size"), v4("geo"), f("radius")])
            .map_err(|e| warn!("clip shader: {e}"))
            .ok()?;
        let blur = (|| {
            let v2 = |n: &'static str| UniformName::new(n, UniformType::_2f);
            let f = |n: &'static str| UniformName::new(n, UniformType::_1f);
            let down = r.compile_custom_texture_shader(blur_down_src(), &[v2("halfpixel"), f("offset")]).ok()?;
            let up = r.compile_custom_texture_shader(blur_up_src(), &[v2("halfpixel"), f("offset")]).ok()?;
            let clip_sub = r
                .compile_custom_texture_shader(clip_sub_src(), &[v2("size"), f("radius"), v2("uv_origin"), v2("uv_size")])
                .ok()?;
            Some(BlurShaders { down, up, clip_sub })
        })();
        if blur.is_none() {
            warn!("blur shaders failed to compile; blur disabled");
        }
        Some(Self { border, shadow, clip, blur })
    }
}

pub fn uniform_f(name: &'static str, v: f32) -> Uniform<'static> {
    Uniform::new(name, v)
}
pub fn uniform_2f(name: &'static str, a: f32, b: f32) -> Uniform<'static> {
    Uniform::new(name, (a, b))
}
pub fn uniform_4f(name: &'static str, c: [f32; 4]) -> Uniform<'static> {
    Uniform::new(name, (c[0], c[1], c[2], c[3]))
}

/// A surface element drawn through the rounded-corner shader.
#[derive(Debug)]
pub struct ClippedSurface {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    /// radius in logical pixels
    radius: f32,
    /// the window's visible rectangle inside the surface, logical pixels (x, y, w, h)
    geo: (f32, f32, f32, f32),
}

impl ClippedSurface {
    pub fn new(inner: WaylandSurfaceRenderElement<GlesRenderer>, program: GlesTexProgram, radius: f32, geo: (f32, f32, f32, f32)) -> Self {
        Self { inner, program, radius, geo }
    }
}

impl Element for ClippedSurface {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        self.inner.location(scale)
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        // corners are transparent
        OpaqueRegions::default()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for ClippedSurface {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let scale = if self.inner.geometry(Scale::from(1.0)).size.w > 0 {
            dst.size.w as f32 / self.inner.geometry(Scale::from(1.0)).size.w as f32
        } else {
            1.0
        };
        frame.override_default_tex_program(
            self.program.clone(),
            vec![
                uniform_2f("size", dst.size.w as f32, dst.size.h as f32),
                uniform_4f("geo", [self.geo.0 * scale, self.geo.1 * scale, self.geo.2 * scale, self.geo.3 * scale]),
                uniform_f("radius", self.radius * scale),
            ],
        );
        let res = RenderElement::<GlesRenderer>::draw(&self.inner, frame, src, dst, damage, opaque_regions);
        frame.clear_tex_program_override();
        res
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        // never scan out directly: the corners need the shader
        None
    }
}

/// A texture sub-region drawn through the rounded-corner shader (used for blurred backgrounds).
#[derive(Debug)]
pub struct ClippedTexture {
    inner: TextureRenderElement<GlesTexture>,
    program: GlesTexProgram,
    /// radius in logical pixels
    radius: f32,
    uv_origin: (f32, f32),
    uv_size: (f32, f32),
}

impl ClippedTexture {
    pub fn new(
        inner: TextureRenderElement<GlesTexture>,
        program: GlesTexProgram,
        radius: f32,
        uv_origin: (f32, f32),
        uv_size: (f32, f32),
    ) -> Self {
        Self { inner, program, radius, uv_origin, uv_size }
    }
}

impl Element for ClippedTexture {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        self.inner.location(scale)
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for ClippedTexture {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let logical_w = self.inner.geometry(Scale::from(1.0)).size.w.max(1) as f32;
        let scale = dst.size.w as f32 / logical_w;
        frame.override_default_tex_program(
            self.program.clone(),
            vec![
                uniform_2f("size", dst.size.w as f32, dst.size.h as f32),
                uniform_f("radius", self.radius * scale),
                uniform_2f("uv_origin", self.uv_origin.0, self.uv_origin.1),
                uniform_2f("uv_size", self.uv_size.0, self.uv_size.1),
            ],
        );
        let res = RenderElement::<GlesRenderer>::draw(&self.inner, frame, src, dst, damage, opaque_regions);
        frame.clear_tex_program_override();
        res
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}
