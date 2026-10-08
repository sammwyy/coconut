use creamui_core::layout::{Dimension, FlexDirection, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_widgets::{RawButton, RawView, Text, TextSize};

pub(crate) fn item(
    label: impl Into<String>,
    hint: impl Into<String>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    icon_item(label, hint, None, trailing, click)
}

pub(crate) fn icon_item(
    label: impl Into<String>,
    hint: impl Into<String>,
    icon: Option<BoxedWidget>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let hint = hint.into();
    Box::new(RawButton::new(Style {
        size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
        min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(56.0) },
        flex_shrink: 0.0,
        align_items: Some(creamui_core::layout::AlignItems::Center),
        gap: creamui_core::layout::Size { width: LengthPercentage::Length(12.0), height: LengthPercentage::Length(0.0) },
        padding: creamui_core::layout::Rect { left: LengthPercentage::Length(16.0), right: LengthPercentage::Length(16.0), top: LengthPercentage::Length(10.0), bottom: LengthPercentage::Length(10.0) },
        ..Default::default()
    }, click).hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover)).child(Box::new(jsx! {
        <RawView style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto }, align_items: Some(creamui_core::layout::AlignItems::Center), gap: creamui_core::layout::Size { width: LengthPercentage::Length(12.0), height: LengthPercentage::Length(0.0) }, ..Default::default() }}>
            {icon.unwrap_or_else(|| Box::new(RawView::new(Style { display: creamui_core::layout::Display::None, ..Default::default() })))}
            <RawView style={Style { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_shrink: 1.0, gap: creamui_core::layout::Size { width: LengthPercentage::Length(0.0), height: LengthPercentage::Length(2.0) }, min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Auto }, ..Default::default() }}>
                {Box::new(Text::new(label.into())) as BoxedWidget}
                {if hint.is_empty() { Box::new(RawView::new(Style { display: creamui_core::layout::Display::None, ..Default::default() })) as BoxedWidget } else { Box::new(Text::secondary(hint).size(TextSize::Sm)) as BoxedWidget }}
            </RawView>
            <Flex shrink={0.0}>{trailing}</Flex>
        </RawView>
    })))
}

pub(crate) fn key_value(label: impl Into<String>, value: impl Into<String>) -> BoxedWidget {
    Box::new(RawView::new(Style {
        size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
        ..Default::default()
    }).child(Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto }, ..Default::default() }} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Between} gap={16.0} padding={16.0}>
            {Box::new(Text::secondary(label.into()).size(TextSize::Sm)) as BoxedWidget}
            {Box::new(Text::new(value.into()).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    })))
}
