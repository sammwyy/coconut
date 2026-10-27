use creamui_core::{
    layout::{AlignItems, Dimension, FlexDirection, LengthPercentage, Style},
    BoxedWidget, StateStyle, Styled,
};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{RawButton, RawView, Text};
pub const CLEAR: Color = Color::rgba(0, 0, 0, 0);
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
        ..row(gap)
    }
}
pub fn fixed(w: f32, h: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(w),
            height: Dimension::Length(h),
        },
        flex_shrink: 0.0,
        ..row(0.0)
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
pub fn button(label: &str, width: f32, action: impl Fn() + 'static) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                ..fixed(width, 34.0)
            },
            action,
        )
        .background(CLEAR)
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .focus_style(StateStyle::new().outline(t.colors.accent, 2.0))
        .child(Box::new(Text::new(label).font_size(16.0))),
    )
}

pub fn positioned(mut style: Style, x: f32, y: f32) -> Style {
    use creamui_core::layout::{LengthPercentageAuto, Position};
    style.position = Position::Absolute;
    style.inset = creamui_core::layout::Rect {
        left: LengthPercentageAuto::Length(x),
        top: LengthPercentageAuto::Length(y),
        right: LengthPercentageAuto::Auto,
        bottom: LengthPercentageAuto::Auto,
    };
    style
}
