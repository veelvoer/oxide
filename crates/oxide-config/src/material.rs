//! Material 3 style color schemes, for the "Material Expressive" style.
//!
//! A seed color (the wallpaper's, or the accent) becomes tonal palettes: the same hue at every tone (lightness)
//! from 0 to 100, as colorful as the screen can show at that tone. Material's tone is CIELAB lightness, so the math
//! here is CIELAB LCh. The roles (primary, containers, surface levels, outline) follow Material 3's scheme, with the
//! livelier chroma of its expressive variant.

/// sRGB 0..255 to linear 0..1
fn lin(c: f64) -> f64 {
    let c = c / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// linear to sRGB 0..255 (unclamped, so callers can tell when a color is out of gamut)
fn delin(c: f64) -> f64 {
    let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    v * 255.0
}

const WHITE: [f64; 3] = [0.95047, 1.0, 1.08883];

fn f(t: f64) -> f64 {
    if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 }
}

fn finv(t: f64) -> f64 {
    let t3 = t * t * t;
    if t3 > 216.0 / 24389.0 { t3 } else { (116.0 * t - 16.0) / (24389.0 / 27.0) }
}

/// sRGB to CIELAB LCh: (lightness 0..100, chroma, hue in degrees)
pub fn lch(rgb: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (lin(rgb[0] as f64), lin(rgb[1] as f64), lin(rgb[2] as f64));
    let x = 0.4124 * r + 0.3576 * g + 0.1805 * b;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = 0.0193 * r + 0.1192 * g + 0.9505 * b;
    let (fx, fy, fz) = (f(x / WHITE[0]), f(y / WHITE[1]), f(z / WHITE[2]));
    let l = 116.0 * fy - 16.0;
    let (a, bb) = (500.0 * (fx - fy), 200.0 * (fy - fz));
    let h = bb.atan2(a).to_degrees().rem_euclid(360.0);
    (l, (a * a + bb * bb).sqrt(), h)
}

/// CIELAB LCh to sRGB, None when the screen cannot show it.
fn rgb(l: f64, c: f64, h: f64) -> Option<[u8; 3]> {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let (x, y, z) = (finv(fx) * WHITE[0], finv(fy) * WHITE[1], finv(fz) * WHITE[2]);
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let bl = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let out = [delin(r), delin(g), delin(bl)];
    out.iter().all(|v| (-0.5..=255.5).contains(v)).then(|| out.map(|v| v.round().clamp(0.0, 255.0) as u8))
}

/// The color of `hue` at `tone`, with as much of `chroma` as the screen can show.
pub fn tone(hue: f64, chroma: f64, tone: f64) -> [u8; 3] {
    // the ends are exact black and white
    if tone <= 0.0 {
        return [0, 0, 0];
    }
    if tone >= 100.0 {
        return [255, 255, 255];
    }
    if let Some(c) = rgb(tone, chroma, hue) {
        return c;
    }
    let (mut lo, mut hi) = (0.0, chroma);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if rgb(tone, mid, hue).is_some() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    rgb(tone, lo, hue).unwrap_or_else(|| {
        let v = (tone / 100.0 * 255.0).round() as u8;
        [v, v, v]
    })
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn parse(h: &str) -> Option<[u8; 3]> {
    let h = h.strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([v(0)?, v(2)?, v(4)?])
}

/// One hue and chroma; any tone.
#[derive(Clone, Copy, Debug)]
pub struct Tonal {
    pub hue: f64,
    pub chroma: f64,
}

impl Tonal {
    pub fn at(&self, t: f64) -> String {
        hex(tone(self.hue, self.chroma, t))
    }
}

/// The roles of a scheme, as "#rrggbb".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scheme {
    pub primary: String,
    pub on_primary: String,
    pub primary_container: String,
    pub on_primary_container: String,
    pub secondary_container: String,
    pub on_secondary_container: String,
    pub tertiary: String,
    pub tertiary_container: String,
    pub on_tertiary_container: String,
    pub surface: String,
    pub surface_low: String,
    pub surface_container: String,
    pub surface_high: String,
    pub surface_highest: String,
    pub on_surface: String,
    pub on_surface_variant: String,
    pub outline: String,
    pub outline_variant: String,
    pub error: String,
    pub good: String,
    pub warn: String,
}

/// A Material Expressive scheme from a seed color.
pub fn scheme(seed: [u8; 3], light: bool) -> Scheme {
    let (_, c, h) = lch(seed);
    // grey seeds still get a hint of color; vivid seeds are kept lively
    let primary = Tonal { hue: h, chroma: c.max(36.0).min(72.0) };
    let secondary = Tonal { hue: h, chroma: (c * 0.35).clamp(12.0, 24.0) };
    // tertiary: a neighbouring hue for accents that should stand apart
    let tertiary = Tonal { hue: (h + 60.0).rem_euclid(360.0), chroma: (c * 0.6).clamp(24.0, 40.0) };
    let neutral = Tonal { hue: h, chroma: 5.0 };
    let variant = Tonal { hue: h, chroma: 9.0 };
    let error = Tonal { hue: 25.0, chroma: 70.0 };
    let green = Tonal { hue: 145.0, chroma: 42.0 };
    let amber = Tonal { hue: 75.0, chroma: 60.0 };
    if light {
        Scheme {
            primary: primary.at(40.0),
            on_primary: primary.at(100.0),
            primary_container: primary.at(90.0),
            on_primary_container: primary.at(10.0),
            secondary_container: secondary.at(90.0),
            on_secondary_container: secondary.at(10.0),
            tertiary: tertiary.at(40.0),
            tertiary_container: tertiary.at(90.0),
            on_tertiary_container: tertiary.at(10.0),
            surface: neutral.at(98.0),
            surface_low: neutral.at(96.0),
            surface_container: neutral.at(94.0),
            surface_high: neutral.at(92.0),
            surface_highest: neutral.at(90.0),
            on_surface: neutral.at(10.0),
            on_surface_variant: variant.at(30.0),
            outline: variant.at(50.0),
            outline_variant: variant.at(80.0),
            error: error.at(40.0),
            good: green.at(40.0),
            warn: amber.at(45.0),
        }
    } else {
        Scheme {
            primary: primary.at(80.0),
            on_primary: primary.at(20.0),
            primary_container: primary.at(30.0),
            on_primary_container: primary.at(90.0),
            secondary_container: secondary.at(30.0),
            on_secondary_container: secondary.at(90.0),
            tertiary: tertiary.at(80.0),
            tertiary_container: tertiary.at(30.0),
            on_tertiary_container: tertiary.at(90.0),
            surface: neutral.at(6.0),
            surface_low: neutral.at(10.0),
            surface_container: neutral.at(12.0),
            surface_high: neutral.at(17.0),
            surface_highest: neutral.at(22.0),
            on_surface: neutral.at(90.0),
            on_surface_variant: variant.at(80.0),
            outline: variant.at(60.0),
            outline_variant: variant.at(30.0),
            error: error.at(80.0),
            good: green.at(80.0),
            warn: amber.at(80.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tones() {
        for c in [[212u8, 87, 46], [30, 120, 220], [128, 128, 128], [255, 255, 255], [0, 0, 0]] {
            let (l, ch, h) = lch(c);
            let back = rgb(l, ch, h).unwrap();
            for i in 0..3 {
                assert!((back[i] as i32 - c[i] as i32).abs() <= 1, "{c:?} -> {back:?}");
            }
        }
        // tone is lightness: 0 is black, 100 white, and every tone exists for every hue
        assert_eq!(tone(30.0, 50.0, 0.0), [0, 0, 0]);
        assert_eq!(tone(30.0, 50.0, 100.0), [255, 255, 255]);
        for h in (0..360).step_by(30) {
            for t in (5..100).step_by(10) {
                let (l, _, _) = lch(tone(h as f64, 200.0, t as f64));
                assert!((l - t as f64).abs() < 1.5, "hue {h} tone {t} gave {l}");
            }
        }
    }

    #[test]
    fn scheme_contrast() {
        for light in [false, true] {
            let s = scheme([212, 87, 46], light);
            let l = |x: &str| lch(parse(x).unwrap()).0;
            // text stands well apart from what it is on
            assert!((l(&s.on_surface) - l(&s.surface)).abs() > 70.0);
            assert!((l(&s.on_primary) - l(&s.primary)).abs() > 45.0);
            assert!((l(&s.on_primary_container) - l(&s.primary_container)).abs() > 55.0);
        }
    }
}
