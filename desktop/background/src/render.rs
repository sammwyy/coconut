use coconut_core::{BackgroundConfig, BackgroundMode, GradientConfig, WallpaperSource};
use image::{imageops, Rgba, RgbaImage};

/// Rasterizes one output into an opaque RGBA frame. Keeping this independent
/// from Wayland makes placement behavior deterministic and cheap to test.
/// [`WallpaperSource::Shader`] draws on the GPU; this returns the configured
/// solid color as its CPU fallback.
pub fn render(
    config: &BackgroundConfig,
    image: Option<&RgbaImage>,
    width: u32,
    height: u32,
) -> RgbaImage {
    if config.source == WallpaperSource::Gradient {
        return render_gradient(&config.gradient, width, height);
    }
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

/// A linear gradient running along `angle_degrees`: the extremes of the
/// output's projection onto that direction map to the two endpoints.
fn render_gradient(gradient: &GradientConfig, width: u32, height: u32) -> RgbaImage {
    let mut frame = RgbaImage::new(width, height);
    if width == 0 || height == 0 {
        return frame;
    }
    let (dy, dx) = (
        f64::from(gradient.angle_degrees.to_radians().sin()),
        f64::from(gradient.angle_degrees.to_radians().cos()),
    );
    let project = |x: f64, y: f64| x * dx + y * dy;
    let corners = [
        project(0.0, 0.0),
        project(width as f64, 0.0),
        project(0.0, height as f64),
        project(width as f64, height as f64),
    ];
    let min = corners.iter().copied().fold(f64::INFINITY, f64::min);
    let extent = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max) - min;
    if extent <= 0.0 {
        return frame;
    }
    let from = [gradient.from.r, gradient.from.g, gradient.from.b];
    let to = [gradient.to.r, gradient.to.g, gradient.to.b];
    for (x, y, pixel) in frame.enumerate_pixels_mut() {
        let t = ((project(x as f64 + 0.5, y as f64 + 0.5) - min) / extent).clamp(0.0, 1.0) as f32;
        for channel in 0..3 {
            pixel[channel] = (from[channel] as f32
                + (to[channel] as f32 - from[channel] as f32) * t)
                .round() as u8;
        }
        pixel[3] = 255;
    }
    frame
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

    fn gradient_config(angle_degrees: f32) -> BackgroundConfig {
        BackgroundConfig {
            source: WallpaperSource::Gradient,
            gradient: coconut_core::GradientConfig {
                from: DesktopColor { r: 0, g: 0, b: 0 },
                to: DesktopColor {
                    r: 100,
                    g: 50,
                    b: 200,
                },
                angle_degrees,
            },
            ..Default::default()
        }
    }

    #[test]
    fn gradient_runs_left_to_right_at_zero_degrees() {
        let frame = render(&gradient_config(0.0), None, 2, 2);
        assert_eq!(*frame.get_pixel(0, 0), Rgba([25, 13, 50, 255]));
        assert_eq!(*frame.get_pixel(1, 0), Rgba([75, 38, 150, 255]));
        assert_eq!(frame.get_pixel(0, 0), frame.get_pixel(0, 1));
    }

    #[test]
    fn gradient_runs_top_to_bottom_at_ninety_degrees() {
        let frame = render(&gradient_config(90.0), None, 2, 2);
        assert_eq!(*frame.get_pixel(0, 0), Rgba([25, 13, 50, 255]));
        assert_eq!(*frame.get_pixel(0, 1), Rgba([75, 38, 150, 255]));
        assert_eq!(frame.get_pixel(0, 0), frame.get_pixel(1, 0));
    }

    #[test]
    fn shader_source_falls_back_to_the_solid_color() {
        let mut config = config(BackgroundMode::Full);
        config.source = WallpaperSource::Shader;
        let frame = render(&config, Some(&source()), 2, 2);
        assert!(frame
            .pixels()
            .all(|pixel| *pixel == Rgba([10, 20, 30, 255])));
    }
}
