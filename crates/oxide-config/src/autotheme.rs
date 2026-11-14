//! A palette taken from a picture: the theme that follows your wallpaper.
//!
//! The picture is shrunk, its pixels are grouped into a few dominant colors, and a whole palette is built from
//! them: backgrounds and text in the picture's main hue, an accent from its most vivid color, the semantic colors
//! (good, warn, bad) and the terminal colors from the standard preset so they stay readable.
use crate::theme::Palette;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What was learned from the picture; stored in `auto-theme.json` next to the config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Seed {
    /// The main hue of the picture (0..360) and how colorful it is overall (0..1).
    pub hue: f32,
    pub tint: f32,
    /// The accent color ("#rrggbb"), and a second vivid color for gradients.
    pub accent: String,
    pub second: String,
}

impl Default for Seed {
    fn default() -> Self {
        Self { hue: 25.0, tint: 0.12, accent: "#d4572e".into(), second: "#e08a63".into() }
    }
}

pub fn seed_path() -> PathBuf {
    crate::Config::path().with_file_name("auto-theme.json")
}

pub fn load_seed() -> Option<Seed> {
    serde_json::from_str(&std::fs::read_to_string(seed_path()).ok()?).ok()
}

pub fn save_seed(s: &Seed) -> std::io::Result<()> {
    std::fs::write(seed_path(), serde_json::to_string_pretty(s).map_err(std::io::Error::other)?)
}

/// Up to ~4000 pixels of a picture file (jpg or png).
pub fn samples_from_file(path: &Path) -> Option<Vec<[u8; 3]>> {
    let img = image::open(path).ok()?.thumbnail(80, 80).to_rgb8();
    Some(img.pixels().map(|p| p.0).collect())
}

fn rgb_to_hsl(c: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (c[0] / 255.0, c[1] / 255.0, c[2] / 255.0);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 } * 60.0;
    (h, s.clamp(0.0, 1.0), l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

fn hex(c: [f32; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8)
}

fn hsl_hex(h: f32, s: f32, l: f32) -> String {
    hex(hsl_to_rgb(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0)))
}

fn parse(h: &str) -> Option<[f32; 3]> {
    let h = h.strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok().map(|b| b as f32);
    Some([v(0)?, v(2)?, v(4)?])
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Group pixels into `k` clusters: (center, share of the pixels).
fn kmeans(px: &[[f32; 3]], k: usize) -> Vec<([f32; 3], f32)> {
    if px.is_empty() {
        return vec![];
    }
    // start from evenly spread picks over the brightness order, so very different areas start apart
    let mut order: Vec<usize> = (0..px.len()).collect();
    order.sort_by(|&a, &b| (px[a][0] + px[a][1] + px[a][2]).total_cmp(&(px[b][0] + px[b][1] + px[b][2])));
    let mut centers: Vec<[f32; 3]> = (0..k).map(|i| px[order[(order.len() - 1) * (2 * i + 1) / (2 * k)]]).collect();
    let mut counts = vec![0usize; k];
    for _ in 0..10 {
        let mut sum = vec![[0.0f32; 3]; k];
        counts = vec![0; k];
        for p in px {
            let i = (0..k).min_by(|&a, &b| dist(*p, centers[a]).total_cmp(&dist(*p, centers[b]))).unwrap_or(0);
            counts[i] += 1;
            for c in 0..3 {
                sum[i][c] += p[c];
            }
        }
        for i in 0..k {
            if counts[i] > 0 {
                centers[i] = [sum[i][0] / counts[i] as f32, sum[i][1] / counts[i] as f32, sum[i][2] / counts[i] as f32];
            }
        }
    }
    let n = px.len() as f32;
    centers.into_iter().zip(counts).filter(|(_, c)| *c > 0).map(|(c, n_i)| (c, n_i as f32 / n)).collect()
}

/// Learn the seed from pixels.
pub fn seed_from_samples(samples: &[[u8; 3]]) -> Seed {
    let px: Vec<[f32; 3]> = samples.iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
    let clusters = kmeans(&px, 6);
    if clusters.is_empty() {
        return Seed::default();
    }
    // the picture's overall tone: the average, weighted by how much of the picture each color covers
    let (mut sx, mut sy, mut ssat) = (0.0f32, 0.0f32, 0.0f32);
    for (c, share) in &clusters {
        let (h, s, _) = rgb_to_hsl(*c);
        sx += h.to_radians().cos() * s * share;
        sy += h.to_radians().sin() * s * share;
        ssat += s * share;
    }
    let hue = if sx.abs() + sy.abs() < 1e-4 { 25.0 } else { sy.atan2(sx).to_degrees().rem_euclid(360.0) };
    let tint = ssat.clamp(0.0, 1.0);
    // vivid colors that are neither near black nor near white make good accents
    let mut vivid: Vec<(f32, [f32; 3])> = clusters
        .iter()
        .map(|(c, share)| {
            let (_, s, l) = rgb_to_hsl(*c);
            let fit = 1.0 - ((l - 0.55).abs() / 0.55).min(1.0);
            (s * fit * (0.35 + share.sqrt()), *c)
        })
        .collect();
    vivid.sort_by(|a, b| b.0.total_cmp(&a.0));
    let pick = |i: usize| vivid.get(i).map(|v| v.1);
    let first = pick(0).filter(|_| vivid[0].0 > 0.06);
    let (ah, as_, _) = first.map(rgb_to_hsl).unwrap_or((hue, 0.5, 0.5));
    let accent = (ah, as_.clamp(0.45, 0.9));
    // the second color: the next vivid one whose hue differs, else a neighbor hue
    let second_h = vivid
        .iter()
        .skip(1)
        .map(|v| rgb_to_hsl(v.1))
        .find(|(h, s, _)| *s > 0.2 && ((h - ah).abs().min(360.0 - (h - ah).abs())) > 25.0)
        .map(|v| v.0)
        .unwrap_or(ah + 35.0);
    Seed { hue, tint, accent: hsl_hex(accent.0, accent.1, 0.62), second: hsl_hex(second_h, accent.1 * 0.85, 0.66) }
}

/// A seed from one flat color (the "Color" background).
pub fn seed_from_color(c: &str) -> Option<Seed> {
    let rgb = parse(c)?;
    let (h, s, _) = rgb_to_hsl(rgb);
    Some(Seed { hue: h, tint: s, accent: hsl_hex(h, s.clamp(0.45, 0.9), 0.62), second: hsl_hex(h + 35.0, (s * 0.85).clamp(0.3, 0.8), 0.66) })
}

/// The whole palette for a seed. `base` (a preset of the same mode) supplies the semantic and terminal colors.
pub fn palette_from_seed(seed: &Seed, light: bool, base: &Palette) -> Palette {
    let h = seed.hue;
    // how tinted the surfaces are: a gray picture gives neutral grays, a colorful one a clearly tinted theme
    let s = (0.10 + seed.tint * 0.55).clamp(0.08, 0.45);
    let mut p = base.clone();
    p.name = "auto".into();
    p.light = light;
    if light {
        p.bg = hsl_hex(h, s * 0.9, 0.945);
        p.surface = hsl_hex(h, s * 0.8, 0.975);
        p.raised = hsl_hex(h, s, 0.905);
        p.line = hsl_hex(h, s, 0.82);
        p.text = hsl_hex(h, s * 0.9, 0.13);
        p.muted = hsl_hex(h, s * 0.6, 0.40);
    } else {
        p.bg = hsl_hex(h, s, 0.085);
        p.surface = hsl_hex(h, s, 0.125);
        p.raised = hsl_hex(h, s * 0.95, 0.17);
        p.line = hsl_hex(h, s * 0.9, 0.23);
        p.text = hsl_hex(h, s * 0.5, 0.92);
        p.muted = hsl_hex(h, s * 0.45, 0.64);
    }
    let acc = parse(&seed.accent).unwrap_or([212.0, 87.0, 46.0]);
    let (ah, as_, _) = rgb_to_hsl(acc);
    // on light backgrounds the accent has to be darker to stay readable
    let accent = if light { hsl_hex(ah, as_.max(0.55), 0.42) } else { hsl_hex(ah, as_, 0.64) };
    p.accent = accent.clone();
    p.accent_soft = if light { hsl_hex(ah, as_, 0.55) } else { hsl_hex(ah, as_ * 0.9, 0.74) };
    let ac = parse(&accent).unwrap_or(acc);
    p.ink = if relative_luminance(ac) > 0.3 { "#161310".into() } else { "#fbf7f2".into() };
    let second = parse(&seed.second).map(|c| {
        let (sh, ss, _) = rgb_to_hsl(c);
        hsl_hex(sh, ss, if light { 0.48 } else { 0.68 })
    });
    p.border = vec![accent, second.unwrap_or_else(|| p.accent_soft.clone())];
    p.border_inactive = format!("{}aa", p.line);
    p.blue = hsl_hex(215.0, 0.6, if light { 0.45 } else { 0.68 });
    p
}

fn relative_luminance(c: [f32; 3]) -> f32 {
    let f = |v: f32| {
        let v = v / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(c: [u8; 3]) -> Vec<[u8; 3]> {
        vec![c; 400]
    }

    #[test]
    fn blue_picture_gives_blue_theme() {
        let mut px = flat([30, 60, 140]);
        px.extend(flat([20, 40, 100]));
        px.extend(flat([200, 210, 240]));
        let seed = seed_from_samples(&px);
        assert!((200.0..250.0).contains(&seed.hue), "hue {}", seed.hue);
        let base = crate::theme::presets()[0].clone();
        let dark = palette_from_seed(&seed, false, &base);
        let light = palette_from_seed(&seed, true, &crate::theme::light_presets()[0]);
        // dark bg is dark, light bg is light, and text contrasts with both
        let l = |h: &str| relative_luminance(parse(h).unwrap());
        assert!(l(&dark.bg) < 0.05 && l(&dark.text) > 0.5);
        assert!(l(&light.bg) > 0.7 && l(&light.text) < 0.1);
        assert!(!dark.light && light.light);
    }

    #[test]
    fn gray_picture_stays_neutral_and_never_panics() {
        let seed = seed_from_samples(&flat([128, 128, 128]));
        assert!(seed.tint < 0.05);
        let _ = seed_from_samples(&[]);
        assert!(seed_from_color("#336699").is_some());
    }
}
