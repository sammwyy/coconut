use creamui_core::{ImageEffects, Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub grayscale: bool,
    pub invert: bool,
}
impl Default for Adjustments {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            grayscale: false,
            invert: false,
        }
    }
}
impl Adjustments {
    pub fn effects(self, quarter_turns: u8) -> ImageEffects {
        let saturation = if self.grayscale {
            0.0
        } else {
            self.saturation.clamp(0.0, 2.0)
        };
        let contrast = self.contrast.clamp(0.0, 2.0);
        let brightness = self.brightness.clamp(0.0, 2.0);
        let mut rows = [[0.0; 4]; 3];
        let luma = [0.2126, 0.7152, 0.0722];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, weight) in luma.iter().enumerate() {
                row[j] = ((1.0 - saturation) * weight + if i == j { saturation } else { 0.0 })
                    * contrast
                    * brightness;
            }
            row[3] = 0.5 * (1.0 - contrast) * brightness;
            if self.invert {
                for value in &mut row[..3] {
                    *value = -*value;
                }
                row[3] = 1.0 - row[3];
            }
        }
        ImageEffects {
            quarter_turns: quarter_turns % 4,
            color_matrix: rows,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Fit,
    Scale(f32),
}
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub zoom: Zoom,
    pub pan: Point,
    pub quarter_turns: u8,
    pub adjustments: Adjustments,
}
impl Default for View {
    fn default() -> Self {
        Self {
            zoom: Zoom::Fit,
            pan: Point::default(),
            quarter_turns: 0,
            adjustments: Adjustments::default(),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub image: Rect,
    pub scale: f32,
    pub pan: Point,
}
impl View {
    pub fn dimensions(self, image: Size) -> Size {
        if self.quarter_turns.is_multiple_of(2) {
            image
        } else {
            Size {
                width: image.height,
                height: image.width,
            }
        }
    }
    pub fn layout(self, viewport: Size, image: Size) -> Layout {
        let size = self.dimensions(image);
        let fit = (viewport.width / size.width.max(1.0))
            .min(viewport.height / size.height.max(1.0))
            .clamp(0.000001, 1.0);
        let scale = match self.zoom {
            Zoom::Fit => fit,
            Zoom::Scale(value) => value.clamp(0.01, 32.0),
        };
        let width = size.width * scale;
        let height = size.height * scale;
        let max_x = ((width - viewport.width) / 2.0).max(0.0);
        let max_y = ((height - viewport.height) / 2.0).max(0.0);
        let pan = if self.zoom == Zoom::Fit {
            Point::default()
        } else {
            Point {
                x: self.pan.x.clamp(-max_x, max_x),
                y: self.pan.y.clamp(-max_y, max_y),
            }
        };
        Layout {
            image: Rect {
                x: (viewport.width - width) / 2.0 + pan.x,
                y: (viewport.height - height) / 2.0 + pan.y,
                width,
                height,
            },
            scale,
            pan,
        }
    }
    pub fn zoom_at(&mut self, factor: f32, anchor: Point, viewport: Size, image: Size) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.layout(viewport, image);
        let scale = (before.scale * factor).clamp(0.01, 32.0);
        let x = (anchor.x - before.image.x) / before.image.width.max(1.0);
        let y = (anchor.y - before.image.y) / before.image.height.max(1.0);
        let size = self.dimensions(image);
        self.zoom = Zoom::Scale(scale);
        self.pan = Point {
            x: anchor.x - x * size.width * scale - (viewport.width - size.width * scale) / 2.0,
            y: anchor.y - y * size.height * scale - (viewport.height - size.height * scale) / 2.0,
        };
        self.pan = self.layout(viewport, image).pan;
    }
    pub fn set_pan(&mut self, pan: Point, viewport: Size, image: Size) {
        self.pan = pan;
        self.pan = self.layout(viewport, image).pan;
    }
    pub fn rotate(&mut self, clockwise: bool) {
        self.quarter_turns = (self.quarter_turns + if clockwise { 1 } else { 3 }) % 4;
        self.pan = Point::default();
    }
    pub fn minimap(self, viewport: Size, image: Size) -> Option<(Rect, Rect)> {
        let layout = self.layout(viewport, image);
        if layout.image.width <= viewport.width + 1.0
            && layout.image.height <= viewport.height + 1.0
        {
            return None;
        }
        let source = self.dimensions(image);
        let scale = (140.0 / source.width)
            .min(100.0 / source.height)
            .min((viewport.width - 24.0).max(1.0) / source.width);
        let map = Rect {
            x: viewport.width - source.width * scale - 12.0,
            y: viewport.height - source.height * scale - 12.0,
            width: source.width * scale,
            height: source.height * scale,
        };
        let ratio = map.width / layout.image.width;
        let visible = Rect {
            x: map.x + (-layout.image.x).max(0.0) * ratio,
            y: map.y + (-layout.image.y).max(0.0) * ratio,
            width: viewport.width.min(layout.image.width) * ratio,
            height: viewport.height.min(layout.image.height) * ratio,
        };
        Some((map, visible))
    }
    pub fn navigate_map(&mut self, point: Point, viewport: Size, image: Size) {
        if let Some((map, _)) = self.minimap(viewport, image) {
            let source = self.dimensions(image);
            let scale = self.layout(viewport, image).scale;
            self.set_pan(
                Point {
                    x: (0.5 - (point.x - map.x) / map.width) * source.width * scale,
                    y: (0.5 - (point.y - map.y) / map.height) * source.height * scale,
                },
                viewport,
                image,
            );
        }
    }
}
pub fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.y >= rect.y
        && point.x <= rect.x + rect.width
        && point.y <= rect.y + rect.height
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_actual_size_anchor_zoom_and_pan_clamp() {
        let viewport = Size {
            width: 400.0,
            height: 300.0,
        };
        let image = Size {
            width: 1600.0,
            height: 1200.0,
        };
        let mut view = View::default();
        assert_eq!(view.layout(viewport, image).scale, 0.25);
        let anchor = Point { x: 100.0, y: 75.0 };
        view.zoom_at(2.0, anchor, viewport, image);
        let result = view.layout(viewport, image);
        assert_eq!(result.scale, 0.5);
        assert_eq!(result.image.x, -100.0);
        assert_eq!(result.image.y, -75.0);
        view.set_pan(
            Point {
                x: 9999.0,
                y: -9999.0,
            },
            viewport,
            image,
        );
        let result = view.layout(viewport, image);
        assert_eq!(
            result.pan,
            Point {
                x: 200.0,
                y: -150.0
            }
        );
        assert!(view.minimap(viewport, image).is_some());
        view.zoom = Zoom::Fit;
        assert!(view.minimap(viewport, image).is_none());
    }
    #[test]
    fn rotation_and_minimap_navigation_use_displayed_dimensions() {
        let viewport = Size {
            width: 400.0,
            height: 300.0,
        };
        let image = Size {
            width: 1600.0,
            height: 800.0,
        };
        let mut view = View {
            zoom: Zoom::Scale(1.0),
            ..Default::default()
        };
        view.rotate(true);
        assert_eq!(
            view.dimensions(image),
            Size {
                width: 800.0,
                height: 1600.0
            }
        );
        let (map, _) = view.minimap(viewport, image).unwrap();
        view.navigate_map(Point { x: map.x, y: map.y }, viewport, image);
        assert_eq!(
            view.layout(viewport, image).pan,
            Point { x: 200.0, y: 650.0 }
        );
        view.rotate(false);
        assert_eq!(view.quarter_turns, 0);
    }
    #[test]
    fn temporary_color_adjustments_preserve_alpha_and_reset_to_identity() {
        let mut pixel = [100, 50, 25, 128];
        let adjustments = Adjustments {
            grayscale: true,
            ..Default::default()
        };
        adjustments.effects(0).transform_pixel(&mut pixel);
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
        assert_eq!(pixel[3], 128);
        assert_eq!(Adjustments::default().effects(0), ImageEffects::default());
        let mut pixel = [0, 0, 0, 0];
        Adjustments {
            invert: true,
            ..Default::default()
        }
        .effects(0)
        .transform_pixel(&mut pixel);
        assert_eq!(pixel, [0, 0, 0, 0]);
    }
}
