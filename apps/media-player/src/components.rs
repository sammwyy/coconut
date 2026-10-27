use crate::state::PlayerState;
pub(crate) use creamui_core::{
    layout::{
        AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage,
        LengthPercentageAuto, Style,
    },
    BoxedWidget, Point, Rect, StateStyle, Styled,
};
pub(crate) use creamui_theme::{use_theme, Color};
pub(crate) use creamui_widgets::{Icon, RawButton, RawView, Text};

pub(crate) const CLEAR: Color = Color::rgba(0, 0, 0, 0);
pub(crate) const BLACK: Color = Color::rgb(0, 0, 0);

pub(crate) fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    }
}

pub(crate) fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        ..row(gap)
    }
}

pub(crate) fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}

pub(crate) fn pad(mut style: Style, x: f32, y: f32) -> Style {
    style.padding = creamui_core::layout::Rect {
        left: LengthPercentage::Length(x),
        right: LengthPercentage::Length(x),
        top: LengthPercentage::Length(y),
        bottom: LengthPercentage::Length(y),
    };
    style
}

pub(crate) fn view(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(RawView::new(style).with_children(children))
}

pub(crate) fn label(text: impl Into<String>, size: f32, secondary: bool) -> BoxedWidget {
    Box::new(if secondary {
        Text::secondary(text).font_size(size)
    } else {
        Text::new(text).font_size(size)
    })
}

pub(crate) fn glyph(p: &PlayerState, name: &str, size: f32, color: Color) -> BoxedWidget {
    Box::new(Icon::new(p.icons.get(name), color).size(size))
}

pub(crate) fn icon_button(
    p: &PlayerState,
    icon: &str,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    icon_button_sized(p, icon, 32.0, 17.0, active, click)
}

pub(crate) fn icon_button_sized(
    p: &PlayerState,
    icon: &str,
    side: f32,
    icon_size: f32,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                ..fixed(side, side)
            },
            click,
        )
        .keyboard_focus(false)
        .background(if active {
            t.colors.selection_background
        } else {
            CLEAR
        })
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .child(glyph(
            p,
            icon,
            icon_size,
            if active {
                t.colors.accent
            } else {
                t.colors.text_secondary
            },
        )),
    )
}

pub(crate) fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub(crate) fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!(
            "{}…",
            text.chars().take(max - 1).collect::<String>().trim_end()
        )
    }
}
