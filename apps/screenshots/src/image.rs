use creamui_core::{Point, Rect};
use std::path::Path;

/// A captured screen: premultiplied RGBA8 pixels, `width * 4` byte rows.
#[derive(Clone)]
pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Shot {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        if pixels.len() != width as usize * height as usize * 4 {
            return Err(format!(
                "pixel buffer holds {} bytes for {width}x{height}",
                pixels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Pastes `source` at `origin`, downsampling when `source` is larger than
    /// the destination span (e.g. a scaled output captured at native pixels).
    pub fn blit(&mut self, origin: (i32, i32), span: (u32, u32), source: &Self) {
        if span.0 == 0 || span.1 == 0 {
            return;
        }
        let sx = source.width as f64 / span.0 as f64;
        let sy = source.height as f64 / span.1 as f64;
        for dy in 0..span.1 {
            let ty = origin.1 + dy as i32;
            if ty < 0 || ty >= self.height as i32 {
                continue;
            }
            let src_y = ((dy as f64 + 0.5) * sy) as u32;
            let src_y = src_y.min(source.height - 1);
            for dx in 0..span.0 {
                let tx = origin.0 + dx as i32;
                if tx < 0 || tx >= self.width as i32 {
                    continue;
                }
                let src_x = ((dx as f64 + 0.5) * sx) as u32;
                let src_x = src_x.min(source.width - 1);
                let from = (src_y * source.width + src_x) as usize * 4;
                let to = (ty as u32 * self.width + tx as u32) as usize * 4;
                self.pixels[to..to + 4].copy_from_slice(&source.pixels[from..from + 4]);
            }
        }
    }

    /// The screenshot area `rect` covers, as a new image. Returns `None` when
    /// the rect leaves no visible pixels.
    pub fn crop(&self, rect: Rect) -> Option<Self> {
        let x0 = rect.x.floor().max(0.0) as u32;
        let y0 = rect.y.floor().max(0.0) as u32;
        let x1 = (rect.x + rect.width).ceil().min(self.width as f32) as u32;
        let y1 = (rect.y + rect.height).ceil().min(self.height as f32) as u32;
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (width, height) = (x1 - x0, y1 - y0);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for row in 0..height {
            let from = ((y0 + row) * self.width + x0) as usize * 4;
            let to = (row * width) as usize * 4;
            pixels[to..to + width as usize * 4]
                .copy_from_slice(&self.pixels[from..from + width as usize * 4]);
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    /// Separable box blur used for the blur annotation brush.
    pub fn blurred(&self, radius: u32) -> Self {
        let radius = radius.clamp(1, 256) as usize;
        let (width, height) = (self.width as usize, self.height as usize);
        let mut horizontal = vec![0u8; self.pixels.len()];
        box_pass(&self.pixels, &mut horizontal, width, height, radius, true);
        let mut pixels = vec![0u8; self.pixels.len()];
        box_pass(&horizontal, &mut pixels, width, height, radius, false);
        Self {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// Straight-alpha RGBA8, for targets that don't understand premultiplied
    /// input (system clipboards). Opaque pixels pass through unchanged.
    pub fn unpremultiplied(&self) -> Vec<u8> {
        let mut out = self.pixels.clone();
        for pixel in out.chunks_exact_mut(4) {
            let alpha = pixel[3] as u16;
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((*channel as u16 * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        out
    }

    /// Layers `overlay` over this shot inside `rect` and returns the cropped
    /// result (still premultiplied, with opaque alpha).
    pub fn flattened(&self, rect: Rect, overlay: Option<&Annotations>) -> Option<Self> {
        let mut result = self.crop(rect)?;
        if let Some(overlay) = overlay {
            let x0 = rect.x.floor().max(0.0) as u32;
            let y0 = rect.y.floor().max(0.0) as u32;
            for row in 0..result.height {
                for column in 0..result.width {
                    let above = overlay.pixel(x0 + column, y0 + row);
                    if above[3] == 0 {
                        continue;
                    }
                    let index = (row * result.width + column) as usize * 4;
                    let base = &mut result.pixels[index..index + 4];
                    blend_over(base, above);
                }
            }
        }
        for pixel in result.pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        Some(result)
    }

    pub fn write_png(&self, path: &Path) -> Result<(), String> {
        let image = image::RgbaImage::from_raw(self.width, self.height, self.unpremultiplied())
            .ok_or_else(|| "capture has an inconsistent pixel buffer".to_owned())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        image
            .save(path)
            .map_err(|error| format!("could not save {}: {error}", path.display()))
    }
}

fn blend_over(base: &mut [u8], above: [u8; 4]) {
    let a = above[3] as u16;
    let inv = 255 - a;
    for channel in 0..3 {
        base[channel] = (above[channel] as u16 + (base[channel] as u16 * inv + 127) / 255) as u8;
    }
    base[3] = base[3].max(above[3]);
}

fn box_pass(source: &[u8], target: &mut [u8], width: usize, height: usize, r: usize, rows: bool) {
    let (outer, inner) = if rows {
        (height, width)
    } else {
        (width, height)
    };
    let window = (2 * r + 1) as u32;
    let stride = if rows { width * 4 } else { 4 };
    let step = if rows { 4 } else { width * 4 };
    for o in 0..outer {
        let base = o * stride;
        let mut sums = [0u32; 4];
        for i in 0..=(r.min(inner - 1)) {
            let index = base + i * step;
            for c in 0..4 {
                sums[c] += source[index + c] as u32;
            }
        }
        for i in 0..inner {
            let index = base + i * step;
            for c in 0..4 {
                target[index + c] = ((sums[c] + window / 2) / window) as u8;
            }
            let add = (i + r + 1).min(inner - 1);
            let sub = i.saturating_sub(r);
            for c in 0..4 {
                sums[c] += source[base + add * step + c] as u32;
                sums[c] -= source[base + sub * step + c] as u32;
            }
        }
    }
}

/// The transparent annotation layer: every dab writes premultiplied RGBA at
/// screenshot resolution; the eraser clears pixels back to transparent.
pub struct Annotations {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub dirty: bool,
}

#[derive(Clone, Copy)]
pub enum Dab {
    Brush { color: [u8; 3] },
    Blur,
    Erase,
}

impl Annotations {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            dirty: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.dirty
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        let index = (y * self.width + x) as usize * 4;
        self.pixels[index..index + 4].try_into().unwrap_or([0; 4])
    }

    /// Stamps `dab` at `point`, a circle of `radius` image pixels.
    pub fn stamp(&mut self, point: Point, radius: f32, dab: Dab, blurred: Option<&Shot>) {
        let radius = radius.max(1.0);
        let x0 = (point.x - radius).floor().max(0.0) as u32;
        let y0 = (point.y - radius).floor().max(0.0) as u32;
        let x1 = (point.x + radius).ceil().min(self.width as f32) as u32;
        let y1 = (point.y + radius).ceil().min(self.height as f32) as u32;
        let feather = (radius - 1.0).max(0.5);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - point.x;
                let dy = y as f32 + 0.5 - point.y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > radius {
                    continue;
                }
                let coverage = ((radius - distance) / feather).clamp(0.0, 1.0);
                let index = (y * self.width + x) as usize * 4;
                match dab {
                    Dab::Brush { color } => {
                        let alpha = (255.0 * coverage).round() as u16;
                        let pixel = &mut self.pixels[index..index + 4];
                        for c in 0..3 {
                            let src = color[c] as u16 * alpha / 255;
                            let inv = 255 - alpha;
                            pixel[c] = (src + (pixel[c] as u16 * inv + 127) / 255) as u8;
                        }
                        pixel[3] = pixel[3].max(alpha as u8);
                    }
                    Dab::Blur => {
                        let Some(blurred) = blurred else { continue };
                        if x >= blurred.width || y >= blurred.height {
                            continue;
                        }
                        let from = (y * blurred.width + x) as usize * 4;
                        let pixel = &mut self.pixels[index..index + 4];
                        for (channel, source) in
                            pixel.iter_mut().zip(blurred.pixels[from..from + 4].iter())
                        {
                            let (src, dst) = (*source as f32, *channel as f32);
                            *channel = (src * coverage + dst * (1.0 - coverage)).round() as u8;
                        }
                    }
                    Dab::Erase => {
                        let pixel = &mut self.pixels[index..index + 4];
                        for channel in pixel.iter_mut() {
                            *channel = (*channel as f32 * (1.0 - coverage)).round() as u8;
                        }
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// Stamps along the segment `from`→`to` so fast drags stay continuous.
    pub fn stroke(
        &mut self,
        from: Point,
        to: Point,
        radius: f32,
        dab: Dab,
        blurred: Option<&Shot>,
    ) {
        let distance = (to.x - from.x).hypot(to.y - from.y);
        let step = (radius * 0.4).max(1.0);
        let dabs = (distance / step).ceil().max(1.0) as u32;
        for i in 0..=dabs {
            let t = i as f32 / dabs as f32;
            self.stamp(
                Point {
                    x: from.x + (to.x - from.x) * t,
                    y: from.y + (to.y - from.y) * t,
                },
                radius,
                dab,
                blurred,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(width: u32, height: u32, color: [u8; 4]) -> Shot {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        Shot {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn blit_downscales_into_the_destination_span() {
        let source = shot(4, 4, [10, 20, 30, 255]);
        let mut target = shot(4, 4, [0, 0, 0, 255]);
        target.blit((1, 1), (2, 2), &source);
        assert_eq!(&target.pixels[20..24], &[10, 20, 30, 255]);
        assert_eq!(&target.pixels[0..4], &[0, 0, 0, 255]);
    }

    #[test]
    fn crop_clamps_to_bounds() {
        let image = shot(8, 8, [9, 9, 9, 255]);
        let cropped = image
            .crop(Rect {
                x: -4.0,
                y: 2.0,
                width: 8.0,
                height: 10.0,
            })
            .unwrap();
        assert_eq!((cropped.width, cropped.height), (4, 6));
    }

    #[test]
    fn flatten_blends_the_annotation_layer() {
        let image = shot(4, 4, [0, 0, 0, 255]);
        let mut overlay = Annotations::new(4, 4);
        overlay.stamp(
            Point { x: 2.0, y: 2.0 },
            2.0,
            Dab::Brush { color: [255, 0, 0] },
            None,
        );
        let flat = image
            .flattened(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 4.0,
                    height: 4.0,
                },
                Some(&overlay),
            )
            .unwrap();
        let center = (2 * 4 + 2) as usize * 4;
        assert_eq!(flat.pixels[center], 255);
        assert_eq!(flat.pixels[center + 3], 255);
    }

    #[test]
    fn eraser_restores_transparency() {
        let mut overlay = Annotations::new(8, 8);
        let point = Point { x: 4.0, y: 4.0 };
        overlay.stamp(point, 3.0, Dab::Brush { color: [0, 255, 0] }, None);
        overlay.stamp(point, 2.0, Dab::Erase, None);
        let center = (4 * 8 + 4) as usize * 4;
        assert!(overlay.pixels[center + 3] < 32);
    }
}
