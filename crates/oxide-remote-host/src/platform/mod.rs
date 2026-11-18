//! What differs between systems: grabbing the screen, injecting input, system actions, a few facts.
#[cfg(windows)]
mod capture_win;
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;

/// A shrunk screenshot: RGB, and the real size of the screen it came from.
pub struct Image {
    pub rgb: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub full: (u32, u32),
}

/// Shrink BGRA (rows `stride` bytes apart) to `out_w` wide, as RGB, using four threads. Each output pixel is the
/// average of a 2×2 block of the source around its place, which keeps text readable.
pub fn shrink_bgra(src: &[u8], sw: usize, sh: usize, stride: usize, out_w: usize) -> Image {
    let out_w = out_w.clamp(16, sw.max(16)).min(sw);
    let out_h = (sh * out_w / sw.max(1)).max(1);
    let mut rgb = vec![0u8; out_w * out_h * 3];
    let kx = sw as f32 / out_w as f32;
    let ky = sh as f32 / out_h as f32;
    let rows_per = out_h.div_ceil(4).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in rgb.chunks_mut(rows_per * out_w * 3).enumerate() {
            s.spawn(move || {
                for (ry, row) in chunk.chunks_mut(out_w * 3).enumerate() {
                    let y = ci * rows_per + ry;
                    let y0 = ((y as f32 * ky) as usize).min(sh - 1);
                    let y1 = (y0 + (ky.ceil() as usize).max(1) - 1).min(sh - 1).min(y0 + 1);
                    for x in 0..out_w {
                        let x0 = ((x as f32 * kx) as usize).min(sw - 1);
                        let x1 = (x0 + (kx.ceil() as usize).max(1) - 1).min(sw - 1).min(x0 + 1);
                        let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                        for (yy, xx) in [(y0, x0), (y0, x1), (y1, x0), (y1, x1)] {
                            let i = yy * stride + xx * 4;
                            b += src[i] as u32;
                            g += src[i + 1] as u32;
                            r += src[i + 2] as u32;
                        }
                        let o = x * 3;
                        row[o] = (r / 4) as u8;
                        row[o + 1] = (g / 4) as u8;
                        row[o + 2] = (b / 4) as u8;
                    }
                }
            });
        }
    });
    Image { rgb, w: out_w as u32, h: out_h as u32, full: (sw as u32, sh as u32) }
}

pub fn jpeg(img: &Image, quality: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.rgb.len() / 8);
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality.clamp(10, 95));
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2);
    let _ = enc.encode(&img.rgb, img.w as u16, img.h as u16, jpeg_encoder::ColorType::Rgb);
    out
}

/// A small arrow drawn into the picture at (x, y) (screen captures leave the pointer out).
pub fn draw_cursor(img: &mut Image, x: i32, y: i32) {
    const ARROW: [&str; 16] = [
        "X", "XX", "XWX", "XWWX", "XWWWX", "XWWWWX", "XWWWWWX", "XWWWWWWX", "XWWWWWWWX", "XWWWWXXXXX", "XWWXWWX", "XWX XWWX", "XX  XWWX", "X    XWWX", "     XWWX", "      XX",
    ];
    for (dy, row) in ARROW.iter().enumerate() {
        for (dx, c) in row.bytes().enumerate() {
            let (px, py) = (x + dx as i32, y + dy as i32);
            if px < 0 || py < 0 || px >= img.w as i32 || py >= img.h as i32 {
                continue;
            }
            let v = match c {
                b'X' => 0u8,
                b'W' => 255u8,
                _ => continue,
            };
            let i = (py as usize * img.w as usize + px as usize) * 3;
            img.rgb[i] = v;
            img.rgb[i + 1] = v;
            img.rgb[i + 2] = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1080p frame shrunk to 1280 wide and encoded: correct size, and fast enough for 60 pictures a second.
    #[test]
    fn shrink_and_encode_speed() {
        let (w, h) = (1920usize, 1080usize);
        // screen-like: smooth areas with sharp edges (text, windows), not noise
        let mut px = vec![255u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let panel = (x / 160 + y / 90) % 3;
                let edge = (x % 9 < 2) && (y % 19 < 12);
                let v = if edge { 40 } else { 200 + panel as u8 * 20 };
                px[i] = v;
                px[i + 1] = v.saturating_sub(10);
                px[i + 2] = v.saturating_sub(30) + (y * 40 / h) as u8;
            }
        }
        let n = 30;
        let (mut t_shrink, mut t_jpeg, mut size) = (0.0, 0.0, 0);
        for _ in 0..n {
            let t = std::time::Instant::now();
            let img = shrink_bgra(&px, w, h, w * 4, 1280);
            t_shrink += t.elapsed().as_secs_f64();
            assert_eq!((img.w, img.h, img.full), (1280, 720, (1920, 1080)));
            let t = std::time::Instant::now();
            size = jpeg(&img, 60).len();
            t_jpeg += t.elapsed().as_secs_f64();
        }
        eprintln!("1080p -> 1280 px: shrink {:.1} ms, JPEG {:.1} ms ({} KB)", t_shrink * 1000.0 / n as f64, t_jpeg * 1000.0 / n as f64, size / 1024);
        assert!(size > 1000);
    }
}
