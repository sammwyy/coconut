//! Shared native surfaces and text that respects its measured bounds.
use creamui_core::{
    layout::{AlignItems, Dimension, FlexDirection, LengthPercentage, Style},
    BoxedWidget, Painter, Point, Rect, Styled, TextAlign, Widget,
};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{RawView, Text};

pub const CLEAR: Color = Color::rgba(0, 0, 0, 0);

pub fn opaque(color: Color) -> Color {
    Color::rgb(color.r, color.g, color.b)
}
pub fn content_color() -> Color {
    opaque(use_theme().colors.background_secondary())
}
pub fn card_color() -> Color {
    let t = use_theme();
    opaque(
        t.colors
            .background_primary()
            .mix(t.colors.background_secondary(), 0.45),
    )
}

/// The shell material is painted once by the window. Content is a separate,
/// opaque, clipped surface whose shape follows the desktop's corner tokens.
pub struct ContentSurface(pub RawView);
impl ContentSurface {
    pub fn boxed(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
        let t = use_theme();
        Box::new(Self(
            RawView::new(style)
                .background(content_color())
                .corner_radius(t.card_radius)
                .outline(t.colors.border.with_alpha(0.45), 1.0)
                .with_children(children),
        ))
    }
}
impl Widget for ContentSurface {
    fn style(&self) -> creamui_core::Style {
        self.0.style()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.0.paint(p, r);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.0.children()
    }
    fn clips_children(&self) -> bool {
        true
    }
    fn clip_corner_radius(&self) -> f32 {
        self.0.style().paint.corner_radius.unwrap_or_default()
    }
}

pub trait Tint {
    fn with_alpha(self, opacity: f32) -> Color;
}
impl Tint for Color {
    fn with_alpha(self, opacity: f32) -> Color {
        Color::rgba(
            self.r,
            self.g,
            self.b,
            (opacity.clamp(0.0, 1.0) * self.a as f32) as u8,
        )
    }
}

pub fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        ..Default::default()
    }
}
pub fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        flex_shrink: 0.0,
        ..row(gap)
    }
}
pub fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}
pub fn grow() -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::Length(0.0),
        ..column(0.0)
    }
}
pub fn pad(mut style: Style, x: f32, y: f32) -> Style {
    style.padding = creamui_core::layout::Rect {
        left: LengthPercentage::Length(x),
        right: LengthPercentage::Length(x),
        top: LengthPercentage::Length(y),
        bottom: LengthPercentage::Length(y),
    };
    style
}
pub fn view(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(RawView::new(style).with_children(children))
}
pub fn label(text: impl Into<String>, size: f32, secondary: bool) -> BoxedWidget {
    Box::new(if secondary {
        Text::secondary(text).font_size(size)
    } else {
        Text::new(text).font_size(size)
    })
}
pub fn heading(text: impl Into<String>, size: f32) -> BoxedWidget {
    Box::new(Text::new(text).font_size(size).bold(true))
}
pub fn divider() -> BoxedWidget {
    Box::new(
        RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(1.0),
            },
            flex_shrink: 0.0,
            ..row(0.0)
        })
        .background(use_theme().colors.border.with_alpha(0.45)),
    )
}

/// Truncate against the actual font and pixel width, never a character count.
/// A fixed line box and explicit clip prevent long commands, paths and hostnames
/// from wrapping into adjacent cells during resize or live updates.
pub struct Line {
    text: String,
    color: Color,
    size: f32,
    align: TextAlign,
    family: String,
}
impl Line {
    pub fn new(text: impl Into<String>, size: f32, color: Color) -> Self {
        let theme = use_theme();
        Self {
            text: text.into().replace(['\n', '\r', '\t'], " "),
            color,
            size: size * theme.text_scale,
            align: TextAlign::Start,
            family: theme.font_family.to_string(),
        }
    }
    pub fn end(mut self) -> Self {
        self.align = TextAlign::End;
        self
    }
}
impl Widget for Line {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.size * 1.5),
            },
            flex_shrink: 0.0,
            ..row(0.0)
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if rect.width <= 1.0 {
            return;
        }
        let value = creamui_widgets::clamp_to_lines(
            &self.text,
            self.size,
            (rect.width - 1.0).max(1.0),
            Some(&self.family),
            1,
        );
        painter.push_clip(rect);
        painter.fill_text_font(
            rect,
            &value,
            self.color,
            self.size,
            self.align,
            Some(&self.family),
            false,
            false,
        );
        painter.pop_clip();
    }
}
pub fn line(text: impl Into<String>, size: f32, secondary: bool) -> BoxedWidget {
    let t = use_theme();
    Box::new(Line::new(
        text,
        size,
        if secondary {
            t.colors.text_secondary
        } else {
            t.colors.text_primary
        },
    ))
}

#[derive(Clone, Copy)]
pub enum Glyph {
    Cpu,
    Memory,
    Storage,
    Network,
    Hardware,
    Refresh,
    Pause,
    Play,
}

/// One optical grid for navigation, cards and controls; no font-dependent glyphs.
pub struct MonitorIcon {
    glyph: Glyph,
    color: Color,
    size: f32,
}
impl MonitorIcon {
    pub fn new(glyph: Glyph, color: Color, size: f32) -> Self {
        Self { glyph, color, size }
    }
}
impl Widget for MonitorIcon {
    fn style(&self) -> creamui_core::Style {
        fixed(self.size, self.size).into()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        let unit = self.size / 24.0;
        let mut path = |points: &[(f32, f32)]| {
            for pair in points.windows(2) {
                p.stroke_line(
                    Point {
                        x: r.x + pair[0].0 * unit,
                        y: r.y + pair[0].1 * unit,
                    },
                    Point {
                        x: r.x + pair[1].0 * unit,
                        y: r.y + pair[1].1 * unit,
                    },
                    self.color,
                    1.65 * unit,
                );
            }
        };
        match self.glyph {
            Glyph::Cpu => {
                path(&[(6., 6.), (18., 6.), (18., 18.), (6., 18.), (6., 6.)]);
                path(&[(10., 10.), (14., 10.), (14., 14.), (10., 14.), (10., 10.)]);
                for v in [9., 15.] {
                    path(&[(v, 3.), (v, 6.)]);
                    path(&[(v, 18.), (v, 21.)]);
                    path(&[(3., v), (6., v)]);
                    path(&[(18., v), (21., v)]);
                }
            }
            Glyph::Memory => {
                path(&[(3., 7.), (21., 7.), (21., 17.), (3., 17.), (3., 7.)]);
                for x in [7., 12., 17.] {
                    path(&[(x, 10.), (x, 14.)]);
                    path(&[(x, 17.), (x, 20.)]);
                }
            }
            Glyph::Storage => {
                path(&[
                    (3., 14.),
                    (6., 4.),
                    (18., 4.),
                    (21., 14.),
                    (21., 20.),
                    (3., 20.),
                    (3., 14.),
                    (21., 14.),
                ]);
                path(&[(16., 17.), (18., 17.)]);
                path(&[(6., 17.), (7., 17.)]);
            }
            Glyph::Network => {
                path(&[(3., 7.), (21., 7.), (17., 3.)]);
                path(&[(21., 17.), (3., 17.), (7., 21.)]);
                path(&[(3., 7.), (7., 11.)]);
                path(&[(21., 17.), (17., 13.)]);
            }
            Glyph::Hardware => {
                path(&[(3., 4.), (21., 4.), (21., 16.), (3., 16.), (3., 4.)]);
                path(&[(12., 16.), (12., 21.)]);
                path(&[(7., 21.), (17., 21.)]);
            }
            Glyph::Refresh => {
                path(&[
                    (20., 9.),
                    (17., 4.),
                    (10., 3.),
                    (4., 7.),
                    (3., 14.),
                    (7., 20.),
                    (14., 21.),
                    (19., 17.),
                ]);
                path(&[(20., 3.), (20., 9.), (14., 9.)]);
            }
            Glyph::Pause => {
                path(&[(8., 5.), (8., 19.)]);
                path(&[(16., 5.), (16., 19.)]);
            }
            Glyph::Play => path(&[(7., 4.), (20., 12.), (7., 20.), (7., 4.)]),
        }
    }
}
pub fn icon(glyph: Glyph, color: Color, size: f32) -> BoxedWidget {
    Box::new(MonitorIcon::new(glyph, color, size))
}

/// A bounded, filled time series. Missing telemetry is represented by no samples.
pub struct Chart {
    pub samples: Vec<f32>,
    pub color: Color,
    pub height: f32,
    pub ceiling: f32,
    pub grid: bool,
}
impl Chart {
    pub fn boxed(
        samples: Vec<f32>,
        color: Color,
        height: f32,
        ceiling: f32,
        grid: bool,
    ) -> BoxedWidget {
        Box::new(Self {
            samples,
            color,
            height,
            ceiling,
            grid,
        })
    }
}
impl Widget for Chart {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.height),
            },
            flex_shrink: 0.0,
            ..row(0.0)
        }
        .into()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        p.push_clip(r);
        let ceiling = if self.ceiling > 0.0 {
            self.ceiling
        } else {
            self.samples.iter().copied().fold(1.0_f32, f32::max) * 1.1
        };
        if self.grid {
            for i in 0..=10 {
                let x = r.x + r.width * i as f32 / 10.0;
                p.stroke_line(
                    Point { x, y: r.y },
                    Point {
                        x,
                        y: r.y + r.height,
                    },
                    self.color.with_alpha(0.12),
                    1.0,
                );
            }
            for i in 0..=4 {
                let y = r.y + r.height * i as f32 / 4.0;
                p.stroke_line(
                    Point { x: r.x, y },
                    Point {
                        x: r.x + r.width,
                        y,
                    },
                    self.color.with_alpha(0.14),
                    1.0,
                );
            }
        }
        let step = r.width / 29.0;
        let start = 30_usize.saturating_sub(self.samples.len());
        let y =
            |value: f32| r.y + 1.0 + (1.0 - (value / ceiling).clamp(0.0, 1.0)) * (r.height - 2.0);
        for (i, pair) in self.samples.windows(2).enumerate() {
            let x1 = r.x + (start + i) as f32 * step;
            let x2 = x1 + step;
            let y1 = y(pair[0]);
            let y2 = y(pair[1]);
            for x in x1.floor() as i32..x2.floor() as i32 {
                let fraction = ((x as f32 - x1) / step).clamp(0.0, 1.0);
                let top = (y1 + (y2 - y1) * fraction).floor();
                p.fill_rect(
                    Rect {
                        x: x as f32,
                        y: top,
                        width: 1.0,
                        height: (r.y + r.height).ceil() - top,
                    },
                    self.color.with_alpha(0.13),
                    0.0,
                );
            }
            p.stroke_line(
                Point { x: x1, y: y1 },
                Point { x: x2, y: y2 },
                self.color,
                1.4,
            );
        }
        if self.grid {
            p.stroke_rect(r, self.color.with_alpha(0.45), 1.0, 0.0);
        }
        p.pop_clip();
    }
}

/// Keep modal focus and pointer ownership with the bounded, scrollable sheet.
pub struct Modal(pub creamui_widgets::Overlay);
impl Widget for Modal {
    fn is_modal(&self) -> bool {
        true
    }
    fn style(&self) -> creamui_core::Style {
        self.0.style()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.0.paint(p, r);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.0.children()
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<std::rc::Rc<dyn Fn()>> {
        self.0.on_click()
    }
}

pub fn percent(a: u64, b: u64) -> f32 {
    if b == 0 {
        0.0
    } else {
        a as f32 / b as f32 * 100.0
    }
}
pub fn progress(value: f32, color: Color) -> BoxedWidget {
    let style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(5.0),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    };
    let fill = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(value.clamp(0.0, 100.0) / 100.0),
            height: Dimension::Length(5.0),
        },
        ..row(0.0)
    };
    Box::new(
        RawView::new(style)
            .background(color.with_alpha(0.13))
            .corner_radius(use_theme().radius_small * 0.375)
            .child(Box::new(
                RawView::new(fill)
                    .background(color)
                    .corner_radius(use_theme().radius_small * 0.375),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_fonts::{FontWeight, LayoutSettings};
    use creamui_theme::ThemeProvider;

    #[test]
    fn content_material_and_clip_follow_the_theme() {
        creamui_reactive::with_context_scope(|| {
            for corners in creamui_theme::CornerStyle::ALL {
                let theme = creamui_theme::Theme::light().with_corners(corners);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let panel = ContentSurface::boxed(fixed(100.0, 100.0), vec![]);
                assert_eq!(panel.clip_corner_radius(), theme.card_radius);
                assert!(panel.clips_children());
                assert_eq!(
                    content_color().a,
                    255,
                    "content never shows rows through its background"
                );
                assert_ne!(content_color(), opaque(theme.colors.background_primary()));
            }
        });
    }

    struct TextBounds {
        clip: Option<Rect>,
        painted: usize,
    }
    impl Painter for TextBounds {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
        fn push_clip(&mut self, rect: Rect) {
            self.clip = Some(rect);
        }
        fn pop_clip(&mut self) {
            self.clip = None;
        }
        fn fill_text_font(
            &mut self,
            rect: Rect,
            text: &str,
            _: Color,
            size: f32,
            _: TextAlign,
            family: Option<&str>,
            _: bool,
            _: bool,
        ) {
            assert_eq!(
                self.clip,
                Some(rect),
                "every single-line label clips to its own cell"
            );
            let face = creamui_fonts::resolve(family.unwrap(), FontWeight::Regular);
            let layout = creamui_fonts::cached_layout(
                &face,
                text,
                size,
                &LayoutSettings {
                    max_width: Some(rect.width),
                    ..Default::default()
                },
            );
            assert_eq!(
                layout.lines.len(),
                1,
                "truncated text must not wrap into the next row: {text}"
            );
            assert!(layout.lines[0].width <= rect.width + 0.01);
            self.painted += 1;
        }
    }
    #[test]
    fn long_unicode_labels_stay_inside_their_cells() {
        creamui_reactive::with_context_scope(|| {
            for scale in [1.0, 1.5, 2.0] {
                let mut theme = creamui_theme::Theme::light();
                theme.text_scale = scale;
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let text = Line::new(
                    "/home/sammy/a very long path/東京/ñ\nwith arguments --flag=long-value",
                    12.0,
                    theme.colors.text_primary,
                );
                let mut painter = TextBounds {
                    clip: None,
                    painted: 0,
                };
                for width in [42.0, 80.0, 180.0, 320.0] {
                    text.paint(
                        &mut painter,
                        Rect {
                            x: 20.0,
                            y: 40.0,
                            width,
                            height: 36.0,
                        },
                    );
                }
                assert_eq!(painter.painted, 4);
            }
        });
    }
}
