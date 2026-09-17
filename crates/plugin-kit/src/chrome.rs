use crate::icons::pixel_icon;
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, StateStyle, Style, StyleProp, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{fixed, full_width, Align, Flex, Justify};
use creamui_widgets::{RawButton, RawSlider, RawSwitch};
use std::rc::Rc;

pub fn shell_card() -> Color {
    use_theme().colors.surface
}
pub fn shell_panel() -> Color {
    use_theme().colors.surface_elevated
}
pub fn shell_text() -> Color {
    use_theme().colors.text_primary
}
pub fn shell_muted() -> Color {
    use_theme().colors.text_secondary
}
pub fn shell_accent() -> Color {
    use_theme().colors.accent
}
pub fn shell_accent_hover() -> Color {
    use_theme().colors.accent_hover
}
pub fn shell_control() -> Color {
    use_theme().colors.surface_hover
}
pub fn shell_control_hover() -> Color {
    use_theme().colors.surface_hover
}
pub fn shell_selected() -> Color {
    use_theme().colors.selection_background
}
pub fn shell_border() -> Color {
    use_theme().colors.border
}
pub fn shell_track() -> Color {
    use_theme().colors.border_strong
}
pub fn shell_fill() -> Color {
    use_theme().colors.accent
}

pub const CARD_RADIUS: f32 = 16.0;
pub const ISLAND_RADIUS: f32 = 12.0;

/// The content width shared by every scrollable device/network list — the
/// panel width (380) minus its own padding and the scroll container's.
pub const LIST_WIDTH: f32 = 348.0;

/// Width of one card in the always-two-wide LAN/VPN row: [`LIST_WIDTH`]
/// split evenly across its `gap(8.0)`.
const CONNECTION_CARD_WIDTH: f32 = (LIST_WIDTH - 8.0) / 2.0;

/// A labeled on/off switch, shared by every tile and panel that exposes a
/// toggleable device.
pub fn compact_switch(checked: bool, on_toggle: Rc<dyn Fn()>) -> BoxedWidget {
    let mut toggle = RawSwitch::new(
        checked,
        shell_fill(),
        shell_track(),
        shell_text(),
        move || on_toggle(),
    )
    .radii(10.0, 8.0)
    .thumb_inset(2.0)
    .hover_colors(shell_accent_hover(), shell_track())
    .pressed_colors(shell_fill(), shell_track());
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
        <Flex direction={FlexDirection::Row} padding={14.0} gap={8.0} align={Align::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={ISLAND_RADIUS}>
            <RawText color={shell_text()} font_size={14.0} align={TextAlign::Start}>{label.to_owned()}</RawText>
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
        .property(StyleProp::Background(shell_panel().into()))
        .property(StyleProp::Border(creamui_core::Border::new(shell_border(), 1.0)))
        .property(StyleProp::CornerRadius(ISLAND_RADIUS))
        .child(pixel_icon(icon, 40.0, shell_text()))
        .child(Box::new(jsx! {
            <RawText color={shell_text()} font_size={20.0} align={TextAlign::Center}>{title}</RawText>
        }))
        .child(Box::new(jsx! {
            <RawText color={shell_muted()} font_size={13.0} align={TextAlign::Center}>{caption}</RawText>
        }));
    Box::new(card)
}

/// A smaller, non-growing alternative to [`hero_card`] for a panel that
/// still wants a status card but not one that fills the rest of its column:
/// the icon sits at the left, the value at the right of the same line, and
/// the caption underneath — about half the height of the full hero card.
pub fn compact_hero_card(icon: &str, value: String, caption: String) -> BoxedWidget {
    let card = Flex::column()
        .gap(4.0)
        .padding(12.0)
        .property(StyleProp::Background(shell_panel().into()))
        .property(StyleProp::Border(creamui_core::Border::new(shell_border(), 1.0)))
        .property(StyleProp::CornerRadius(ISLAND_RADIUS))
        .child(Box::new(jsx! {
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
                {pixel_icon(icon, 22.0, shell_text())}
                <Flex grow={1.0} />
                <RawText color={shell_text()} font_size={18.0} align={TextAlign::End}>{value}</RawText>
            </Flex>
        }))
        .child(Box::new(jsx! {
            <RawText color={shell_muted()} font_size={12.0} align={TextAlign::Start}>{caption}</RawText>
        }));
    Box::new(card)
}

/// A compact card for a row of side-by-side connections (a wired LAN port,
/// a VPN/mesh tunnel). `active` picks the highlighted look; `on_click`
/// (meaningful only while active) makes the whole card a button, e.g. into
/// that connection's detail view. Uses an explicit [`CONNECTION_CARD_WIDTH`]
/// for the same reason [`list_row`] does — see its doc comment.
pub fn connection_card(
    icon: &str,
    title: String,
    caption: String,
    active: bool,
    on_click: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let title_color = if active { shell_text() } else { shell_muted() };
    let card: BoxedWidget = Box::new(
        Flex::column()
            .grow(1.0)
            .gap(6.0)
            .padding(10.0)
            .property(StyleProp::Width(CONNECTION_CARD_WIDTH.into()))
            .property(StyleProp::Background((if active { shell_selected() } else { shell_panel() }).into()))
            .property(StyleProp::Border(creamui_core::Border::new(shell_border(), 1.0)))
            .property(StyleProp::CornerRadius(ISLAND_RADIUS))
            .child(Box::new(jsx! {
                <Flex direction={FlexDirection::Row} align={Align::Center} gap={6.0}>
                    {pixel_icon(icon, 16.0, shell_text())}
                    <RawText color={title_color} font_size={12.0} align={TextAlign::Start}>{title}</RawText>
                </Flex>
            }))
            .child(Box::new(jsx! {
                <RawText color={shell_muted()} font_size={10.0} align={TextAlign::Start}>{caption}</RawText>
            })),
    );
    match on_click {
        Some(handler) => Box::new(
            RawButton::new(Style::new().corner_radius(ISLAND_RADIUS), move || handler())
                .child(card),
        ),
        None => card,
    }
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
            shell_track(),
            shell_fill(),
            shell_text(),
            on_change,
        )
        .track(22.0, 11.0)
        .handle(22.0, 8.0)
        .hover_handle_color(shell_text())
        .pressed_handle_color(shell_fill()),
    );
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
                {slider_title(icon, label, on_open)}
                <Flex grow={1.0} />
                <RawText color={shell_muted()} font_size={18.0}>{value_text}</RawText>
            </Flex>
            {slider}
        </Flex>
    })
}

fn slider_title(icon: &str, label: &str, on_open: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
            {pixel_icon(icon, 18.0, shell_text())}
            <RawText color={shell_text()} font_size={13.0}>{label.to_owned()}</RawText>
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
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

/// A panel's title row. With `on_back`, a back arrow is shown ahead of the
/// title so a panel reached from the control center can return to it.
pub fn panel_header(title: &str, on_back: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let Some(on_back) = on_back else {
        return Box::new(jsx! {
            <RawText color={shell_muted()} font_size={14.0}>{title.to_owned()}</RawText>
        });
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
            {back_button(on_back)}
            <RawText color={shell_muted()} font_size={14.0}>{title.to_owned()}</RawText>
        </Flex>
    })
}

fn back_button(on_back: Rc<dyn Fn()>) -> BoxedWidget {
    icon_button("chevron-left", 20.0, on_back)
}

/// A small square icon-only button, used for compact actions inside device
/// and network rows (connect, disconnect, forget, rescan) as well as the
/// panel back button.
pub fn icon_button(icon: &str, size: f32, on_click: Rc<dyn Fn()>) -> BoxedWidget {
    Box::new(
        RawButton::new(icon_button_style(size), move || on_click()).child(Box::new(jsx! {
            <Flex size={(size, size)} align={Align::Center} justify={Justify::Center}>
                {pixel_icon(icon, size * 0.6, shell_text())}
            </Flex>
        })),
    )
}

fn icon_button_style(size: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(size, size),
            ..Default::default()
        })
        .corner_radius(6.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

/// A small muted uppercase-style caption used to head a section of a
/// scrollable list (e.g. "CONNECTIONS", "WI-FI NETWORKS", "DEVICES").
pub fn section_label(text: &str) -> BoxedWidget {
    Box::new(jsx! {
        <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>{text.to_owned()}</RawText>
    })
}

/// Maps a Wi-Fi signal percentage to the matching strength icon, shared by
/// every place that shows a wireless signal (the control center tile, the
/// network panel's hero card, and each row of its network list).
pub fn wifi_strength_icon(strength: u8) -> &'static str {
    match strength {
        75.. => "wifi-excellent",
        50..=74 => "wifi-good",
        25..=49 => "wifi-fair",
        _ => "wifi-weak",
    }
}

/// A tappable row used by scrollable device/network lists: an icon, a
/// title/subtitle pair, and trailing content (a lock glyph, a checkmark,
/// small action buttons) pinned flush to the right edge with a gap from the
/// text. `active` tints the row to mark it as the current connection or
/// device. With no `on_click`, the row renders the same look without being
/// interactive.
///
/// The content row is given an explicit [`LIST_WIDTH`] rather than relying
/// on it stretching to fill its parent: a [`RawButton`]'s single child is
/// sized at its own natural width, not the button's resolved box, so
/// without this the trailing content would hug the title text instead of
/// sitting at the row's right edge.
pub fn list_row(
    icon: &str,
    title: String,
    subtitle: String,
    trailing: Option<BoxedWidget>,
    active: bool,
    on_click: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let trailing = trailing.unwrap_or_else(|| Box::new(jsx! { <Flex/> }));
    let text_column = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} grow={1.0} gap={2.0}>
            <RawText color={shell_text()} font_size={13.0} align={TextAlign::Start}>{title}</RawText>
            <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>{subtitle}</RawText>
        </Flex>
    });
    let content: BoxedWidget = Box::new(
        Flex::row()
            .padding(10.0)
            .gap(10.0)
            .align(Align::Center)
            .property(StyleProp::Width(LIST_WIDTH.into()))
            .child(pixel_icon(icon, 18.0, shell_text()))
            .child(text_column)
            .child(trailing),
    );
    match on_click {
        Some(handler) => {
            Box::new(RawButton::new(list_row_style(active), move || handler()).child(content))
        }
        None => Box::new(jsx! {
            <Flex background={if active { shell_selected() } else { shell_panel() }} border={(shell_border(), 1.0)} corner_radius={ISLAND_RADIUS}>
                {content}
            </Flex>
        }),
    }
}

fn list_row_style(active: bool) -> Style {
    let idle = if active {
        shell_selected()
    } else {
        shell_panel()
    };
    let hover = if active {
        shell_selected()
    } else {
        shell_control_hover()
    };
    Style::new()
        .layout(full_width(LayoutStyle::default()))
        .background(idle)
        .border(shell_border(), 1.0)
        .corner_radius(ISLAND_RADIUS)
        .hover(StateStyle::new().background(hover))
        .pressed(StateStyle::new().background(shell_selected()))
}

/// One label/value line for a detail view (a connection's IP address, a
/// device's Bluetooth address, ...). Placed directly as a plain row rather
/// than a button, so it stretches full width without the explicit-size
/// workaround [`list_row`] and [`connection_card`] need.
pub fn detail_row(label: &str, value: String) -> BoxedWidget {
    detail_row_inner(label, value, None)
}

/// A [`detail_row`] with a small trailing action (e.g. a reveal-password
/// eye button) between the value and the row's edge.
pub fn detail_row_with_action(label: &str, value: String, action: BoxedWidget) -> BoxedWidget {
    detail_row_inner(label, value, Some(action))
}

fn detail_row_inner(label: &str, value: String, action: Option<BoxedWidget>) -> BoxedWidget {
    let action = action.unwrap_or_else(|| Box::new(jsx! { <Flex/> }));
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} padding={10.0} gap={8.0} align={Align::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={ISLAND_RADIUS}>
            <RawText color={shell_muted()} font_size={12.0} align={TextAlign::Start}>{label.to_owned()}</RawText>
            <Flex grow={1.0} />
            <RawText color={shell_text()} font_size={12.0} align={TextAlign::End}>{value}</RawText>
            {action}
        </Flex>
    })
}

/// A small icon-and-label pill button, used for compact actions that read
/// better with a word attached (Scan/Stop, Connect/Disconnect, Forget)
/// than [`icon_button`]'s bare icon.
pub fn action_button(icon: &str, label: &str, on_click: Rc<dyn Fn()>) -> BoxedWidget {
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={6.0} padding={6.0}>
            {pixel_icon(icon, 12.0, shell_text())}
            <RawText color={shell_text()} font_size={11.0}>{label.to_owned()}</RawText>
        </Flex>
    });
    Box::new(RawButton::new(action_button_style(), move || on_click()).child(content))
}

fn action_button_style() -> Style {
    Style::new()
        .corner_radius(8.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}
