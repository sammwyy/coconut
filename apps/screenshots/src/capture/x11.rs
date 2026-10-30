//! X11 capture: `GetImage` on the root window reads the whole virtual screen.

use crate::image::Shot;
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt, ImageFormat},
};

pub fn screen() -> Result<Shot, String> {
    let (connection, screen_index) =
        x11rb::connect(None).map_err(|error| format!("X11 connect failed: {error}"))?;
    let screen = &connection.setup().roots[screen_index];
    let (width, height) = (
        u32::from(screen.width_in_pixels),
        u32::from(screen.height_in_pixels),
    );
    let depth = screen.root_depth;
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            screen.root,
            0,
            0,
            width as u16,
            height as u16,
            (1u32 << depth) - 1,
        )
        .map_err(|error| format!("X11 GetImage failed: {error}"))?
        .reply()
        .map_err(|error| format!("X11 GetImage failed: {error}"))?;
    let format = connection
        .setup()
        .pixmap_formats
        .iter()
        .find(|format| format.depth == depth)
        .map(|format| (format.bits_per_pixel, format.scanline_pad))
        .unwrap_or((32, 32));
    let (bpp, scanline_pad) = format;
    let channels = match bpp {
        32 => 4,
        24 => 3,
        other => {
            return Err(format!("unsupported X11 root depth {depth} ({other} bpp)"));
        }
    };
    let pad = (scanline_pad as usize).max(8);
    let row_bytes = (width as usize * channels * 8).div_ceil(pad) * pad / 8;
    let pixels = convert_packed(&image.data, width, height, row_bytes, channels)?;
    Shot::new(width, height, pixels)
}

/// X `ZPixmap` data arrives B,G,R (little-endian X servers are the norm);
/// `channels` is bytes per pixel in the wire data and rows may carry
/// `row_bytes` of padding at their tail.
fn convert_packed(
    data: &[u8],
    width: u32,
    height: u32,
    row_bytes: usize,
    channels: usize,
) -> Result<Vec<u8>, String> {
    let (width, height) = (width as usize, height as usize);
    if data.len() < row_bytes.saturating_mul(height.saturating_sub(1)) + width * channels {
        return Err(format!(
            "X11 image holds {} bytes for {width}x{height}",
            data.len()
        ));
    }
    let mut pixels = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let from = y * row_bytes + x * channels;
            let to = (y * width + x) * 4;
            pixels[to] = data[from + 2];
            pixels[to + 1] = data[from + 1];
            pixels[to + 2] = data[from];
            pixels[to + 3] = 255;
        }
    }
    Ok(pixels)
}
