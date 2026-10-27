//! Cover art decoding, the soft backdrop derived from it and its accent color.
//!
//! Runs on a background thread, so it only hands plain pixel buffers back to
//! the UI thread.

use creamui_image::ImageData;
use creamui_theme::Color;
use std::sync::Arc;

const BACKDROP_SIDE: u32 = 36;
const BLUR_RADIUS: i32 = 3;
const BLUR_PASSES: usize = 3;
const ACCENT_SAMPLES: usize = 4096;
const HUE_BUCKETS: usize = 36;

pub struct Artwork {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    backdrop: (u32, u32, Vec<u8>),
    accent: Option<Color>,
    encoded: Arc<[u8]>,
}

pub struct ArtworkImages {
    pub cover: Option<ImageData>,
    pub backdrop: Option<ImageData>,
    pub accent: Option<Color>,
    /// The original encoded image, for platform media controls.
    pub encoded: Arc<[u8]>,
}

impl Artwork {
    pub fn into_images(self) -> ArtworkImages {
        let (bw, bh, backdrop) = self.backdrop;
        ArtworkImages {
            cover: ImageData::from_premultiplied_rgba(self.width, self.height, self.pixels).ok(),
            backdrop: ImageData::from_premultiplied_rgba(bw, bh, backdrop).ok(),
            accent: self.accent,
            encoded: self.encoded,
        }
    }
}

pub fn decode(bytes: &[u8]) -> Option<Artwork> {
    let image = ImageData::from_bytes(bytes).ok()?;
    let (width, height) = (image.width(), image.height());
    let pixels = image.pixels().to_vec();
    let mut backdrop = blur(downscale(&pixels, width, height));
    for pixel in backdrop.2.chunks_exact_mut(4) {
        for channel in &mut pixel[..3] {
            *channel = (*channel as f32 * 0.42) as u8;
        }
        pixel[3] = 255;
    }
    let accent = dominant_color(&pixels);
    Some(Artwork {
        width,
        height,
        pixels,
        backdrop,
        accent,
        encoded: bytes.into(),
    })
}

/// The most prominent vivid hue in premultiplied RGBA pixels, tuned to work
/// as an accent color. Grayscale or very dark images have none.
pub fn dominant_color(pixels: &[u8]) -> Option<Color> {
    let count = pixels.len() / 4;
    let step = (count / ACCENT_SAMPLES).max(1);
    let mut weights = [0f32; HUE_BUCKETS];
    let mut sums = [[0f32; 3]; HUE_BUCKETS];
    let mut samples = 0usize;
    for pixel in pixels.chunks_exact(4).step_by(step) {
        if pixel[3] < 128 {
            continue;
        }
        samples += 1;
        let rgb = [pixel[0], pixel[1], pixel[2]].map(|c| c as f32 / 255.0);
        let (hue, saturation, value) = hsv(rgb);
        if value < 0.18 || saturation < 0.22 {
            continue;
        }
        let bucket = ((hue / 360.0 * HUE_BUCKETS as f32) as usize).min(HUE_BUCKETS - 1);
        let weight = saturation * value;
        weights[bucket] += weight;
        for c in 0..3 {
            sums[bucket][c] += rgb[c] * weight;
        }
    }
    let (best, weight) = weights
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    if samples == 0 || *weight < samples as f32 * 0.01 {
        return None;
    }
    let rgb = sums[best].map(|c| c / weight);
    let (hue, saturation, lightness) = hsl(rgb);
    Some(from_hsl(
        hue,
        saturation.clamp(0.45, 0.85),
        lightness.clamp(0.45, 0.62),
    ))
}

fn hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let saturation = if max > 0.0 { delta / max } else { 0.0 };
    (hue(r, g, b, max, delta), saturation, max)
}

fn hsl([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let lightness = (max + min) / 2.0;
    let saturation = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * lightness - 1.0).abs())
    };
    (hue(r, g, b, max, delta), saturation, lightness)
}

fn hue(r: f32, g: f32, b: f32, max: f32, delta: f32) -> f32 {
    if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    }
}

fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation.min(1.0);
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    let channel = |c: f32| ((c + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgb(channel(r), channel(g), channel(b))
}

/// Area-averages premultiplied pixels into a small square thumbnail,
/// center-cropping so the backdrop has the cover's proportions at any size.
fn downscale(pixels: &[u8], width: u32, height: u32) -> (u32, u32, Vec<u8>) {
    let side = width.min(height).max(1);
    let (x0, y0) = ((width - side) / 2, (height - side) / 2);
    let out = BACKDROP_SIDE.min(side);
    let mut result = vec![0u8; (out * out * 4) as usize];
    for oy in 0..out {
        for ox in 0..out {
            let (sx0, sx1) = (
                x0 + ox * side / out,
                x0 + ((ox + 1) * side / out).max(ox * side / out + 1),
            );
            let (sy0, sy1) = (
                y0 + oy * side / out,
                y0 + ((oy + 1) * side / out).max(oy * side / out + 1),
            );
            let mut sum = [0u64; 4];
            let mut count = 0u64;
            for y in sy0..sy1.min(height) {
                for x in sx0..sx1.min(width) {
                    let i = ((y * width + x) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += pixels[i + c] as u64;
                    }
                    count += 1;
                }
            }
            let o = ((oy * out + ox) * 4) as usize;
            for c in 0..4 {
                result[o + c] = (sum[c] / count.max(1)) as u8;
            }
        }
    }
    (out, out, result)
}

/// Repeated box blur, which approximates a gaussian on the thumbnail.
pub(crate) fn blur((width, height, mut pixels): (u32, u32, Vec<u8>)) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let mut scratch = pixels.clone();
    for _ in 0..BLUR_PASSES {
        for (horizontal, src_is_pixels) in [(true, true), (false, false)] {
            let (src, dst) = if src_is_pixels {
                (&pixels, &mut scratch)
            } else {
                (&scratch, &mut pixels)
            };
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 4];
                    for d in -BLUR_RADIUS..=BLUR_RADIUS {
                        let (sx, sy) = if horizontal {
                            ((x + d).clamp(0, w - 1), y)
                        } else {
                            (x, (y + d).clamp(0, h - 1))
                        };
                        let i = ((sy * w + sx) * 4) as usize;
                        for c in 0..4 {
                            sum[c] += src[i + c] as u32;
                        }
                    }
                    let o = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        dst[o + c] = (sum[c] / (2 * BLUR_RADIUS as u32 + 1)) as u8;
                    }
                }
            }
        }
    }
    (width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(rgb: [u8; 3], count: usize) -> Vec<u8> {
        (0..count)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect()
    }

    #[test]
    fn accent_follows_the_dominant_vivid_hue() {
        let mut pixels = solid([20, 20, 20], 600);
        pixels.extend(solid([30, 80, 220], 300));
        pixels.extend(solid([220, 40, 40], 100));
        let accent = dominant_color(&pixels).expect("a vivid hue");
        assert!(accent.b > accent.r && accent.b > accent.g, "{accent:?}");
        assert!(dominant_color(&solid([128, 128, 128], 500)).is_none());
        assert!(dominant_color(&solid([4, 4, 30], 500)).is_none());
    }

    #[test]
    fn backdrop_is_small_square_and_smooth() {
        let (w, h) = (300u32, 200u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let v = if x < w / 2 { 255 } else { 0 };
                pixels[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let (bw, bh, out) = blur(downscale(&pixels, w, h));
        assert_eq!((bw, bh), (BACKDROP_SIDE, BACKDROP_SIDE));
        assert!(out.chunks_exact(4).all(|p| p[3] == 255));
        let mid = ((bh / 2 * bw + bw / 2) * 4) as usize;
        assert!(out[mid] > 40 && out[mid] < 215, "edge is softened");
    }
}
