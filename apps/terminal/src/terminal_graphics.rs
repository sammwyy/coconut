//! Terminal graphics occupy their cells, independently of font bearings and line spacing.
use creamui_core::{Painter, Rect};
use creamui_theme::Color;

/// Draw a block or a common box-drawing stroke. Other text uses the selected font.
pub fn paint(painter: &mut dyn Painter, text: &str, rect: Rect, color: Color) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    let scale = painter.scale_factor();
    let edge = |value: f32| (value * scale).round() / scale;
    let mut fill = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let (x, y, right, bottom) = (edge(x0), edge(y0), edge(x1), edge(y1));
        if right > x && bottom > y {
            painter.fill_rect(
                Rect {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                },
                color,
                0.0,
            );
        }
    };
    let x = rect.x;
    let y = rect.y;
    let right = x + rect.width;
    let bottom = y + rect.height;
    match ch {
        '▀' => fill(x, y, right, y + rect.height / 2.0),
        '▁'..='█' => fill(
            x,
            bottom - rect.height * (ch as u32 - 0x2580) as f32 / 8.0,
            right,
            bottom,
        ),
        '▉'..='▏' => fill(
            x,
            y,
            x + rect.width * (0x2590 - ch as u32) as f32 / 8.0,
            bottom,
        ),
        '▐' => fill(x + rect.width / 2.0, y, right, bottom),
        '▔' => fill(x, y, right, y + rect.height / 8.0),
        '▕' => fill(right - rect.width / 8.0, y, right, bottom),
        '▖'..='▟' => {
            let quadrants = match ch {
                '▖' => 4,
                '▗' => 8,
                '▘' => 1,
                '▙' => 13,
                '▚' => 9,
                '▛' => 7,
                '▜' => 11,
                '▝' => 2,
                '▞' => 6,
                '▟' => 14,
                _ => return false,
            };
            let middle_x = x + rect.width / 2.0;
            let middle_y = y + rect.height / 2.0;
            if quadrants & 1 != 0 {
                fill(x, y, middle_x, middle_y);
            }
            if quadrants & 2 != 0 {
                fill(middle_x, y, right, middle_y);
            }
            if quadrants & 4 != 0 {
                fill(x, middle_y, middle_x, bottom);
            }
            if quadrants & 8 != 0 {
                fill(middle_x, middle_y, right, bottom);
            }
        }
        '░' | '▒' | '▓' => {
            // A screen-anchored Bayer pattern remains continuous between cells.
            const BAYER: [[u8; 4]; 4] =
                [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
            let threshold = match ch {
                '░' => 4,
                '▒' => 8,
                _ => 12,
            };
            for py in (y * scale).round() as i32..(bottom * scale).round() as i32 {
                for px in (x * scale).round() as i32..(right * scale).round() as i32 {
                    if BAYER[py.rem_euclid(4) as usize][px.rem_euclid(4) as usize] < threshold {
                        fill(
                            px as f32 / scale,
                            py as f32 / scale,
                            (px + 1) as f32 / scale,
                            (py + 1) as f32 / scale,
                        );
                    }
                }
            }
        }
        _ => {
            // Bits describe strokes towards left, right, up and down.
            let (arms, heavy) = match ch {
                '─' => (3, false),
                '│' => (12, false),
                '┌' => (10, false),
                '┐' => (9, false),
                '└' => (6, false),
                '┘' => (5, false),
                '├' => (14, false),
                '┤' => (13, false),
                '┬' => (11, false),
                '┴' => (7, false),
                '┼' => (15, false),
                '━' => (3, true),
                '┃' => (12, true),
                '┏' => (10, true),
                '┓' => (9, true),
                '┗' => (6, true),
                '┛' => (5, true),
                '┣' => (14, true),
                '┫' => (13, true),
                '┳' => (11, true),
                '┻' => (7, true),
                '╋' => (15, true),
                '╴' => (1, false),
                '╵' => (4, false),
                '╶' => (2, false),
                '╷' => (8, false),
                '╸' => (1, true),
                '╹' => (4, true),
                '╺' => (2, true),
                '╻' => (8, true),
                _ => return false,
            };
            let thickness = ((if heavy { 2.0 } else { 1.0 }) * scale).round().max(1.0) / scale;
            let line_x = edge(x + (rect.width - thickness) / 2.0);
            let line_y = edge(y + (rect.height - thickness) / 2.0);
            if arms & 1 != 0 {
                fill(x, line_y, line_x + thickness, line_y + thickness);
            }
            if arms & 2 != 0 {
                fill(line_x, line_y, right, line_y + thickness);
            }
            if arms & 4 != 0 {
                fill(line_x, y, line_x + thickness, line_y + thickness);
            }
            if arms & 8 != 0 {
                fill(line_x, line_y, line_x + thickness, bottom);
            }
        }
    }
    true
}
