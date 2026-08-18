//! Ironbow heatmap from an MLX90640 frame — same LUT and mount correction
//! as `py-source/sensor_head/hardware/thermal.py`.

use crate::thermal::{clamp_celsius, THERMAL_COLS, THERMAL_PIXELS, THERMAL_ROWS};

/// Default JPEG upscale from the Python config (`thermal_upscale_size`).
pub const HEATMAP_WIDTH: u32 = 320;
pub const HEATMAP_HEIGHT: u32 = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

fn lut() -> &'static [(u8, u8, u8); 256] {
    use std::sync::OnceLock;
    static LUT: OnceLock<[(u8, u8, u8); 256]> = OnceLock::new();
    LUT.get_or_init(build_lut)
}

fn build_lut() -> [(u8, u8, u8); 256] {
    // Anchors copied from the Python original.
    let anchors: [(usize, (i32, i32, i32)); 9] = [
        (0, (0, 0, 0)),
        (32, (0, 0, 128)),
        (64, (0, 0, 255)),
        (96, (128, 0, 255)),
        (128, (255, 0, 128)),
        (160, (255, 0, 0)),
        (192, (255, 128, 0)),
        (224, (255, 255, 0)),
        (255, (255, 255, 255)),
    ];
    let mut lut = [(0u8, 0u8, 0u8); 256];
    for w in anchors.windows(2) {
        let (idx0, (r0, g0, b0)) = w[0];
        let (idx1, (r1, g1, b1)) = w[1];
        let span = (idx1 - idx0) as f32;
        for j in 0..=(idx1 - idx0) {
            let t = j as f32 / span;
            let idx = idx0 + j;
            if idx < 256 {
                lut[idx] = (
                    (r0 as f32 + (r1 - r0) as f32 * t) as u8,
                    (g0 as f32 + (g1 - g0) as f32 * t) as u8,
                    (b0 as f32 + (b1 - b0) as f32 * t) as u8,
                );
            }
        }
    }
    lut
}

/// LUT entry for a 0–255 index. Exposed for tests.
pub fn ironbow_index(index: u8) -> (u8, u8, u8) {
    lut()[index as usize]
}

/// 32×24 RGB, auto-ranged min→max, no mount correction.
pub fn colorize_frame(frame: &[f32]) -> Option<RgbImage> {
    if frame.len() < THERMAL_PIXELS {
        return None;
    }
    let slice = &frame[..THERMAL_PIXELS];
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for &t in slice {
        let t = clamp_celsius(t);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let range = if hi > lo { hi - lo } else { 1.0 };
    let lut = lut();
    let mut rgb = vec![0u8; THERMAL_PIXELS * 3];
    for (i, &t) in slice.iter().enumerate() {
        let n = (((clamp_celsius(t) - lo) / range) * 255.0).round() as i32;
        let n = n.clamp(0, 255) as usize;
        let (r, g, b) = lut[n];
        rgb[i * 3] = r;
        rgb[i * 3 + 1] = g;
        rgb[i * 3 + 2] = b;
    }
    Some(RgbImage {
        width: THERMAL_COLS as u32,
        height: THERMAL_ROWS as u32,
        rgb,
    })
}

/// 90° clockwise. `(x, y)` in a `w×h` image → `(h-1-y, x)` in an `h×w` image.
fn rotate_90_cw(src: &RgbImage) -> RgbImage {
    let (w, h) = (src.width as usize, src.height as usize);
    let mut rgb = vec![0u8; src.rgb.len()];
    for y in 0..h {
        for x in 0..w {
            let si = (y * w + x) * 3;
            let nx = h - 1 - y;
            let ny = x;
            let di = (ny * h + nx) * 3;
            rgb[di..di + 3].copy_from_slice(&src.rgb[si..si + 3]);
        }
    }
    RgbImage {
        width: src.height,
        height: src.width,
        rgb,
    }
}

fn flip_horizontal(src: &RgbImage) -> RgbImage {
    let (w, h) = (src.width as usize, src.height as usize);
    let mut rgb = vec![0u8; src.rgb.len()];
    for y in 0..h {
        for x in 0..w {
            let si = (y * w + x) * 3;
            let di = (y * w + (w - 1 - x)) * 3;
            rgb[di..di + 3].copy_from_slice(&src.rgb[si..si + 3]);
        }
    }
    RgbImage {
        width: src.width,
        height: src.height,
        rgb,
    }
}

fn nearest_resize(src: &RgbImage, width: u32, height: u32) -> RgbImage {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (dw, dh) = (width as usize, height as usize);
    let mut rgb = vec![0u8; dw * dh * 3];
    for y in 0..dh {
        let sy = y * sh / dh;
        for x in 0..dw {
            let sx = x * sw / dw;
            let si = (sy * sw + sx) * 3;
            let di = (y * dw + x) * 3;
            rgb[di..di + 3].copy_from_slice(&src.rgb[si..si + 3]);
        }
    }
    RgbImage { width, height, rgb }
}

/// Full Python heatmap pipeline: colorize → 90° CW → flip L-R → nearest upscale.
pub fn render_heatmap(frame: &[f32], width: u32, height: u32) -> Option<RgbImage> {
    let img = colorize_frame(frame)?;
    let img = rotate_90_cw(&img);
    let img = flip_horizontal(&img);
    Some(nearest_resize(&img, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_ends_are_black_and_white() {
        assert_eq!(ironbow_index(0), (0, 0, 0));
        assert_eq!(ironbow_index(255), (255, 255, 255));
    }

    #[test]
    fn heatmap_is_320x240_from_32x24() {
        let frame = vec![20.0; THERMAL_PIXELS];
        let img = render_heatmap(&frame, HEATMAP_WIDTH, HEATMAP_HEIGHT).unwrap();
        assert_eq!(img.width, 320);
        assert_eq!(img.height, 240);
        assert_eq!(img.rgb.len(), 320 * 240 * 3);
    }

    #[test]
    fn short_frame_is_none() {
        assert!(colorize_frame(&[1.0, 2.0]).is_none());
    }
}
