use crate::icons::pixel_icon;
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, StateStyle, Style, StyleProp, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::layout::{fixed, Align, Flex, Justify};
use creamui_widgets::{RawButton, RawSlider, RawSwitch};
use std::rc::Rc;

pub const CARD: Color = Color::rgba(27, 28, 30, 252);
pub const PANEL: Color = Color::rgba(39, 40, 42, 245);
pub const TEXT: Color = Color::rgb(244, 244, 245);
pub const MUTED: Color = Color::rgb(166, 168, 171);
pub const ACCENT: Color = Color::rgba(255, 255, 255, 28);
pub const CONTROL: Color = Color::rgba(255, 255, 255, 12);
pub const CONTROL_HOVER: Color = Color::rgba(255, 255, 255, 26);
pub const SELECTED: Color = Color::rgba(255, 255, 255, 42);
pub const BORDER: Color = Color::rgba(255, 255, 255, 20);
pub const TRACK: Color = Color::rgba(255, 255, 255, 38);
pub const FILL: Color = Color::rgb(215, 222, 255);

pub const CARD_RADIUS: f32 = 16.0;
pub const ISLAND_RADIUS: f32 = 12.0;

/// A labeled on/off switch, shared by every tile and panel that exposes a
/// toggleable device.
pub fn compact_switch(checked: bool, on_toggle: Rc<dyn Fn()>) -> BoxedWidget {
    let mut toggle = RawSwitch::new(checked, FILL, TRACK, TEXT, move || on_toggle())
        .radii(10.0, 8.0)
        .thumb_inset(2.0)
        .hover_colors(Color::rgb(230, 235, 255), TRACK)
        .pressed_colors(FILL, TRACK);
    toggle.style = Style::new().layout(LayoutStyle {
        size: fixed(36.0, 20.0),
        ..Default::default()
    });
    Box::new(toggle)
}

/// A full-width row pairing a label with a [`compact_switch`]. Used by
/// dedicated panels (network, bluetooth, energy) for their single toggle.
pub fn toggle_row(label: &str, checked: bool, on_toggle: Rc<dyn Fn()>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} padding={14.0} gap={8.0} align={Align::Center} background={PANEL} border={(BORDER, 1.0)} corner_radius={ISLAND_RADIUS}>
            <RawText color={TEXT} font_size={14.0} align={TextAlign::Start}>{label.to_owned()}</RawText>
            <Flex grow={1.0} />
            {compact_switch(checked, on_toggle)}
        </Flex>
    })
}

/// The centered icon/title/caption card a dedicated panel uses to summarize
/// its device, filling whatever space its parent column grants it.
pub fn hero_card(icon: &str, title: String, caption: String) -> BoxedWidget {
    let card = Flex::column()
        .grow(1.0)
        .gap(10.0)
        .align(Align::Center)
        .justify(Justify::Center)
        .property(StyleProp::Background(PANEL.into()))
        .property(StyleProp::Border(creamui_core::Border::new(BORDER, 1.0)))
        .property(StyleProp::CornerRadius(ISLAND_RADIUS))
        .child(pixel_icon(icon, 40.0))
        .child(Box::new(jsx! {
            <RawText color={TEXT} font_size={20.0} align={TextAlign::Center}>{title}</RawText>
        }))
        .child(Box::new(jsx! {
            <RawText color={MUTED} font_size={13.0} align={TextAlign::Center}>{caption}</RawText>
        }));
    Box::new(card)
}

/// A labeled slider with its live value shown alongside, shared by the
/// control center and the dedicated brightness/volume panels. With
/// `on_open`, the icon and label become a link into that device's panel,
/// independent of the slider's own drag handling.
pub fn fat_slider(
    icon: &str,
    label: &str,
    value: f32,
    value_text: String,
    width: f32,
    on_open: Option<Rc<dyn Fn()>>,
    on_change: impl Fn(f32) + 'static,
) -> BoxedWidget {
    let slider: BoxedWidget = Box::new(
        RawSlider::new(
            Style::new()
                .layout(LayoutStyle {
                    size: fixed(width, 32.0),
                    ..Default::default()
                })
                .focus(StateStyle::new().outline(Color::rgba(0, 0, 0, 0), 0.0)),
            value,
            TRACK,
            FILL,
            TEXT,
            on_change,
        )
        .track(22.0, 11.0)
        .handle(22.0, 8.0)
        .hover_handle_color(Color::rgb(255, 255, 255))
        .pressed_handle_color(FILL),
    );
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
                {slider_title(icon, label, on_open)}
                <Flex grow={1.0} />
                <RawText color={MUTED} font_size={18.0}>{value_text}</RawText>
            </Flex>
            {slider}
        </Flex>
    })
}

fn slider_title(icon: &str, label: &str, on_open: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
            {pixel_icon(icon, 18.0)}
            <RawText color={TEXT} font_size={13.0}>{label.to_owned()}</RawText>
        </Flex>
    });
    let Some(on_open) = on_open else {
        return content;
    };
    Box::new(RawButton::new(slider_title_style(), move || on_open()).child(content))
}

fn slider_title_style() -> Style {
    Style::new()
        .corner_radius(6.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(SELECTED))
}

/// A panel's title row. With `on_back`, a back arrow is shown ahead of the
/// title so a panel reached from the control center can return to it.
pub fn panel_header(title: &str, on_back: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let Some(on_back) = on_back else {
        return Box::new(jsx! {
            <RawText color={MUTED} font_size={14.0}>{title.to_owned()}</RawText>
        });
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
            {back_button(on_back)}
            <RawText color={MUTED} font_size={14.0}>{title.to_owned()}</RawText>
        </Flex>
    })
}

fn back_button(on_back: Rc<dyn Fn()>) -> BoxedWidget {
    Box::new(
        RawButton::new(back_button_style(), move || on_back()).child(Box::new(jsx! {
            <Flex size={(20.0, 20.0)} align={Align::Center} justify={Justify::Center}>
                {pixel_icon("back", 12.0)}
            </Flex>
        })),
    )
}

fn back_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(20.0, 20.0),
            ..Default::default()
        })
        .corner_radius(6.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(SELECTED))
}
