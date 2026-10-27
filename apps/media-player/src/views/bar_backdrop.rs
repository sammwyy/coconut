use creamui_core::runtime::ImageFit;
use creamui_image::ImageData;

pub(super) fn blurred(
    source: &ImageData,
    viewport: (f32, f32),
    bar_width: f32,
    bar_height: f32,
    fit: ImageFit,
) -> Option<ImageData> {
    let (vw, vh) = viewport;
    if vw <= 0.0 || vh <= 0.0 || bar_width <= 0.0 || bar_height <= 0.0 {
        return None;
    }
    let width = (bar_width / 3.0).ceil() as u32;
    let height = (bar_height / 3.0).ceil() as u32;
    let (sw, sh) = (source.width(), source.height());
    let scale = match fit {
        ImageFit::Contain => (vw / sw as f32).min(vh / sh as f32),
        _ => (vw / sw as f32).max(vh / sh as f32),
    };
    let (draw_w, draw_h) = (sw as f32 * scale, sh as f32 * scale);
    let (draw_x, draw_y) = ((vw - draw_w) / 2.0, (vh - draw_h) / 2.0);
    let (card_x, card_y) = ((vw - bar_width) / 2.0, vh - 16.0 - bar_height);
    let pixels = source.pixels();
    let mut output = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let px = card_x + (x as f32 + 0.5) * bar_width / width as f32;
            let py = card_y + (y as f32 + 0.5) * bar_height / height as f32;
            let sx = ((px - draw_x) / scale).floor() as i32;
            let sy = ((py - draw_y) / scale).floor() as i32;
            let dst = ((y * width + x) * 4) as usize;
            output[dst + 3] = 255;
            if (0..sw as i32).contains(&sx) && (0..sh as i32).contains(&sy) {
                let src = ((sy as u32 * sw + sx as u32) * 4) as usize;
                output[dst..dst + 3].copy_from_slice(&pixels[src..src + 3]);
            }
        }
    }
    let (_, _, output) = crate::artwork::blur((width, height, output));
    ImageData::from_premultiplied_rgba(width, height, output).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_the_card_area_and_softens_the_frame() {
        let mut pixels = vec![0u8; 60 * 30 * 4];
        for (i, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel[0] = if i / 60 >= 22 { 255 } else { 0 };
            pixel[3] = 255;
        }
        let image = ImageData::from_premultiplied_rgba(60, 30, pixels).unwrap();
        let blurred = blurred(&image, (600.0, 300.0), 300.0, 84.0, ImageFit::Contain).unwrap();
        assert_eq!((blurred.width(), blurred.height()), (100, 28));
        assert!(blurred
            .pixels()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255));
        assert!(blurred.pixels()[0] > 0 && blurred.pixels()[0] < 255);
    }

    #[test]
    fn letterboxed_video_has_black_margins() {
        let image =
            ImageData::from_premultiplied_rgba(2, 1, vec![255, 0, 0, 255, 255, 0, 0, 255]).unwrap();
        let blurred = blurred(&image, (200.0, 400.0), 100.0, 84.0, ImageFit::Contain).unwrap();
        assert_eq!(blurred.pixels()[0], 0);
    }
}
