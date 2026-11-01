use coconut_plugin_kit::chrome::{
    shell_accent, shell_fill, shell_muted, shell_on_accent, shell_track,
};
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Painter, Point, Rect, Size, Style, TextAlign, Widget,
};
use creamui_widgets::layout::fixed;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(super) enum LevelShape {
    Vertical,
    Dial,
}

pub(super) struct LevelControl {
    value: f32,
    size: Size,
    shape: LevelShape,
    change: Rc<dyn Fn(f32)>,
    accent: creamui_theme::Color,
    fill: creamui_theme::Color,
    track: creamui_theme::Color,
    on_fill: creamui_theme::Color,
    text: creamui_theme::Color,
}

impl LevelControl {
    pub fn new(value: f32, size: Size, shape: LevelShape, change: impl Fn(f32) + 'static) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            size,
            shape,
            change: Rc::new(change),
            accent: shell_accent(),
            fill: shell_fill(),
            track: shell_track(),
            on_fill: shell_on_accent(),
            text: shell_muted(),
        }
    }

    fn fraction(shape: LevelShape, point: Point, rect: Rect) -> f32 {
        match shape {
            LevelShape::Vertical => (1.0 - point.y / rect.height.max(1.0)).clamp(0.0, 1.0),
            LevelShape::Dial => {
                let angle = (point.y - rect.height / 2.0).atan2(point.x - rect.width / 2.0);
                let angle = (angle - PI * 0.75).rem_euclid(TAU);
                if angle <= PI * 1.5 {
                    angle / (PI * 1.5)
                } else if angle < PI * 1.75 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

impl Widget for LevelControl {
    fn style(&self) -> Style {
        Style::new()
            .layout(creamui_core::layout::Style {
                size: fixed(self.size.width, self.size.height),
                ..Default::default()
            })
            .corner_radius(12.0)
            .focus(creamui_core::StateStyle::new().outline(self.accent, 2.0))
    }
    fn focusable(&self) -> bool {
        true
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let change = self.change.clone();
        let value = self.value;
        Some(Rc::new(move |input| {
            let next = match input.key {
                Key::Up | Key::Right => value + 0.05,
                Key::Down | Key::Left => value - 0.05,
                Key::Home => 0.0,
                Key::End => 1.0,
                _ => return,
            };
            change(next.clamp(0.0, 1.0));
        }))
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        let value = self.value;
        let change = self.change.clone();
        Some(Rc::new(move |delta| {
            change((value - delta * 0.002).clamp(0.0, 1.0))
        }))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let change = self.change.clone();
        let shape = self.shape;
        Some(Rc::new(move |point, rect| {
            if matches!(shape, LevelShape::Dial)
                && (point.x - rect.width / 2.0).hypot(point.y - rect.height / 2.0)
                    < rect.width.min(rect.height) * 0.22
            {
                return;
            }
            change(Self::fraction(shape, point, rect))
        }))
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        match self.shape {
            LevelShape::Vertical => {
                painter.fill_rect(rect, self.track, rect.width / 2.0);
                painter.push_clip_rounded(rect, rect.width / 2.0);
                painter.fill_rect(
                    Rect {
                        y: rect.y + rect.height * (1.0 - self.value),
                        height: rect.height * self.value,
                        ..rect
                    },
                    self.fill,
                    0.0,
                );
                painter.pop_clip();
                let y = (rect.y + rect.height * (1.0 - self.value))
                    .clamp(rect.y + 8.0, rect.y + rect.height - 8.0);
                painter.stroke_line(
                    Point {
                        x: rect.x + rect.width * 0.3,
                        y,
                    },
                    Point {
                        x: rect.x + rect.width * 0.7,
                        y,
                    },
                    self.on_fill,
                    3.0,
                );
            }
            LevelShape::Dial => {
                let center = Point {
                    x: rect.x + rect.width / 2.0,
                    y: rect.y + rect.height / 2.0,
                };
                let radius = rect.width.min(rect.height) / 2.0 - 4.0;
                for step in 0..64 {
                    let start = PI * 0.75 + step as f32 / 64.0 * PI * 1.5;
                    let end = PI * 0.75 + (step + 1) as f32 / 64.0 * PI * 1.5;
                    let color = if (step as f32 + 0.5) / 64.0 <= self.value {
                        self.fill
                    } else {
                        self.track
                    };
                    painter.stroke_line(
                        Point {
                            x: center.x + radius * start.cos(),
                            y: center.y + radius * start.sin(),
                        },
                        Point {
                            x: center.x + radius * end.cos(),
                            y: center.y + radius * end.sin(),
                        },
                        color,
                        5.0,
                    );
                }
                painter.fill_text(
                    rect,
                    &format!("{}%", (self.value * 100.0).round()),
                    self.text,
                    12.0,
                    TextAlign::Center,
                );
            }
        }
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertical_drag_increases_upwards_and_clamps_at_both_ends() {
        let rect = Rect {
            x: 20.0,
            y: 30.0,
            width: 40.0,
            height: 160.0,
        };
        for (y, expected) in [
            (-10.0, 1.0),
            (0.0, 1.0),
            (80.0, 0.5),
            (160.0, 0.0),
            (200.0, 0.0),
        ] {
            assert_eq!(
                LevelControl::fraction(LevelShape::Vertical, Point { x: 20.0, y }, rect),
                expected
            );
        }
    }
    #[test]
    fn dial_maps_the_arc_and_clamps_the_bottom_gap() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 60.0,
        };
        for (angle, expected) in [(PI * 0.75, 0.0), (PI * 1.5, 0.5), (PI * 2.25, 1.0)] {
            let value = LevelControl::fraction(
                LevelShape::Dial,
                Point {
                    x: 30.0 + 20.0 * angle.cos(),
                    y: 30.0 + 20.0 * angle.sin(),
                },
                rect,
            );
            assert!((value - expected).abs() < 0.001);
        }
    }
}
