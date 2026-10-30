use creamui_core::{
    layout::{self, AlignItems, Dimension, FlexDirection, LengthPercentage, Style},
    BoxedWidget, StateStyle, Styled,
};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{RawButton, RawView, Text};
pub const CLEAR: Color = Color::rgba(0, 0, 0, 0);
pub fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        gap: layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        min_size: layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    }
}
pub fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        ..row(gap)
    }
}
pub fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}
pub fn pad(mut style: Style, x: f32, y: f32) -> Style {
    style.padding = layout::Rect {
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
pub fn button(label: &str, width: f32, active: bool, action: impl Fn() + 'static) -> BoxedWidget {
    let colors = use_theme().colors;
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(layout::JustifyContent::Center),
                ..fixed(width, 30.0)
            },
            action,
        )
        .background(if active { colors.surface_hover } else { CLEAR })
        .corner_radius(6.0)
        .hover_style(StateStyle::new().background(colors.surface_hover))
        .focus_style(StateStyle::new().outline(colors.accent, 2.0))
        .child(Box::new(Text::new(label).font_size(12.0).color(
            if active {
                colors.accent
            } else {
                colors.text_primary
            },
        ))),
    )
}
pub fn anchored(
    child: BoxedWidget,
    rect: std::rc::Rc<std::cell::Cell<creamui_core::Rect>>,
) -> BoxedWidget {
    Box::new(Anchor {
        style: child.style(),
        child: Some(child),
        rect,
    })
}
struct Anchor {
    style: creamui_core::Style,
    child: Option<BoxedWidget>,
    rect: std::rc::Rc<std::cell::Cell<creamui_core::Rect>>,
}
impl creamui_core::Widget for Anchor {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, _: &mut dyn creamui_core::Painter, _: creamui_core::Rect) {}
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: creamui_core::Rect) -> bool {
        self.rect.set(rect);
        false
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }
}
pub fn menu_icon() -> BoxedWidget {
    Box::new(MenuGlyph)
}
struct MenuGlyph;
impl creamui_core::Widget for MenuGlyph {
    fn style(&self) -> creamui_core::Style {
        fixed(18.0, 18.0).into()
    }
    fn paint(&self, painter: &mut dyn creamui_core::Painter, rect: creamui_core::Rect) {
        for offset in [4.0, 9.0, 14.0] {
            painter.fill_rect(
                creamui_core::Rect {
                    x: rect.x + 1.0,
                    y: rect.y + offset,
                    width: 16.0,
                    height: 1.5,
                },
                use_theme().colors.text_primary,
                0.75,
            );
        }
    }
}
