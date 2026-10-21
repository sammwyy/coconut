use coconut_core::{BackgroundConfig, BackgroundMode, WallpaperSource};
use image::{imageops, Rgba, RgbaImage};

/// Rasterizes one output into an opaque RGBA frame. Keeping this independent
/// from Wayland makes placement behavior deterministic and cheap to test.
pub fn render(
    config: &BackgroundConfig,
    image: Option<&RgbaImage>,
    width: u32,
    height: u32,
) -> RgbaImage {
    let color = config.solid_color;
    let mut frame = RgbaImage::from_pixel(width, height, Rgba([color.r, color.g, color.b, 255]));
    if config.source != WallpaperSource::Image {
        return frame;
    }
    let Some(image) = image.filter(|image| image.width() > 0 && image.height() > 0) else {
        return frame;
    };

    match config.mode {
        BackgroundMode::Full => draw_full(&mut frame, image),
        BackgroundMode::Centered => draw_centered(&mut frame, image),
        BackgroundMode::Stretched => draw_stretched(&mut frame, image),
        BackgroundMode::Mosaic => draw_tiled(&mut frame, image),
    }
    frame
}

fn draw_full(frame: &mut RgbaImage, image: &RgbaImage) {
    let scale = (frame.width() as f64 / image.width() as f64)
        .max(frame.height() as f64 / image.height() as f64);
    let width = (image.width() as f64 * scale).round().max(1.0) as u32;
    let height = (image.height() as f64 * scale).round().max(1.0) as u32;
    let scaled = imageops::resize(image, width, height, imageops::FilterType::CatmullRom);
    imageops::overlay(
        frame,
        &scaled,
        (frame.width() as i64 - width as i64) / 2,
        (frame.height() as i64 - height as i64) / 2,
    );
}

fn draw_centered(frame: &mut RgbaImage, image: &RgbaImage) {
    imageops::overlay(
        frame,
        image,
        (frame.width() as i64 - image.width() as i64) / 2,
        (frame.height() as i64 - image.height() as i64) / 2,
    );
}

fn draw_stretched(frame: &mut RgbaImage, image: &RgbaImage) {
    let scaled = imageops::resize(
        image,
        frame.width(),
        frame.height(),
        imageops::FilterType::CatmullRom,
    );
    imageops::overlay(frame, &scaled, 0, 0);
}

fn draw_tiled(frame: &mut RgbaImage, image: &RgbaImage) {
    for (x, y, pixel) in frame.enumerate_pixels_mut() {
        blend(
            pixel,
            image.get_pixel(x % image.width(), y % image.height()),
        );
    }
}

/// Composites an RGBA wallpaper pixel over the opaque fallback frame.
fn blend(destination: &mut Rgba<u8>, source: &Rgba<u8>) {
    let alpha = source[3] as u16;
    let inverse = 255 - alpha;
    for channel in 0..3 {
        destination[channel] =
            ((source[channel] as u16 * alpha + destination[channel] as u16 * inverse + 127) / 255)
                as u8;
    }
    destination[3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_core::{BackgroundMode, DesktopColor};

    fn config(mode: BackgroundMode) -> BackgroundConfig {
        BackgroundConfig {
            source: WallpaperSource::Image,
            mode,
            solid_color: DesktopColor {
                r: 10,
                g: 20,
                b: 30,
            },
            ..Default::default()
        }
    }

    fn source() -> RgbaImage {
        RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        })
    }

    #[test]
    fn centered_keeps_the_image_at_its_native_size() {
        let frame = render(&config(BackgroundMode::Centered), Some(&source()), 4, 3);
        assert_eq!(frame.get_pixel(0, 0), &Rgba([10, 20, 30, 255]));
        assert_eq!(frame.get_pixel(1, 1), &Rgba([255, 0, 0, 255]));
        assert_eq!(frame.get_pixel(2, 1), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn full_fills_the_entire_output() {
        let frame = render(&config(BackgroundMode::Full), Some(&source()), 4, 4);
        assert!(frame
            .pixels()
            .all(|pixel| *pixel != Rgba([10, 20, 30, 255])));
    }

    #[test]
    fn stretched_maps_the_image_to_both_output_axes() {
        let frame = render(&config(BackgroundMode::Stretched), Some(&source()), 4, 3);
        assert_eq!(frame.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(frame.get_pixel(3, 2), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn tiled_repeats_without_scaling() {
        let frame = render(&config(BackgroundMode::Mosaic), Some(&source()), 5, 2);
        assert_eq!(frame.get_pixel(0, 0), frame.get_pixel(2, 0));
        assert_eq!(frame.get_pixel(1, 1), frame.get_pixel(3, 1));
    }

    #[test]
    fn solid_source_ignores_the_image() {
        let mut config = config(BackgroundMode::Full);
        config.source = WallpaperSource::SolidColor;
        let frame = render(&config, Some(&source()), 2, 2);
        assert!(frame
            .pixels()
            .all(|pixel| *pixel == Rgba([10, 20, 30, 255])));
    }
}
