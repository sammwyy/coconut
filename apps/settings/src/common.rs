use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::{use_theme, Color};
use creamui_widgets::{
    Icon, IconSource, RawButton, RawScrollView, RawView, ScrollController, Symbol, TabColors, Text,
    TextInput, TextSize,
};

/// Shared by all rows, including drill-downs. The category color decorates
/// navigation and icons; the user's accent still identifies active controls.
#[derive(Clone)]
pub struct PageStyle {
    pub color: Color,
    pub title: String,
}

pub fn category_color() -> Color {
    creamui_reactive::try_use_context::<PageStyle>()
        .map(|style| style.color)
        .unwrap_or_else(|| use_theme().colors.accent)
}

pub fn setting_icon(label: &str) -> IconSource {
    let glyph = match label {
        "Panel settings" | "Top panel" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 8h18"/>"#
        }
        "Dock settings" | "Dock position" | "Dock size" | "Auto-hide dock" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><rect x="6" y="15" width="12" height="3" rx="1"/>"#
        }
        "Default layout" | "Arrangement" | "Layout settings" | "Initial size" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M11 3v18"/>"#
        }
        "Workspaces" | "Dynamic workspaces" | "Wrap around" => {
            r#"<path d="m12 3 9 5-9 5-9-5Z m-9 9 9 5 9-5M3 16l9 5 9-5"/>"#
        }
        "Decorations" | "Use titlebar" | "Control style" | "Buttons on" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M6 6h.01M9 6h.01"/>"#
        }
        "Effects" | "Effect settings" | "Enable animations" | "Window open" | "Window close"
        | "Workspace switch" | "Minimize" => {
            r#"<path d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5ZM20 2v4M18 4h4"/>"#
        }
        "Window focus"
        | "Focus follows mouse"
        | "Center new floating windows"
        | "Raise on focus"
        | "Focus new windows"
        | "Focus previous on close"
        | "Warp cursor"
        | "Center title" => {
            r#"<path d="M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3"/>"#
        }
        "Window rules" | "Apply changes automatically" => {
            r#"<path d="m3 6 2 2 4-4M12 6h9M3 13l2 2 4-4M12 13h9M3 20l2 2 4-4M12 20h9"/>"#
        }
        "Widgets" | "Desktop icons" | "Icon size" | "App launcher" | "Open windows"
        | "Show app icon" => {
            r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#
        }
        "Notifications & popups" | "Notifications" | "Visual alerts" => {
            r#"<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/>"#
        }
        "Hot corners" | "Shape" | "Background" => {
            r#"<path d="M3 9V3h6M15 3h6v6M21 15v6h-6M9 21H3v-6"/>"#
        }
        "Status icons" | "Control center" => {
            r#"<path d="M3 6h18M3 12h18M3 18h18"/><circle cx="8" cy="6" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="8" cy="18" r="2"/>"#
        }
        "Clock" | "Clock format" | "Duration" | "Repeat delay (ms)" => {
            r#"<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>"#
        }
        "Weather" | "Dim screen when idle" => {
            r#"<path d="M7 18a4 4 0 1 1 0-8 6 6 0 0 1 11 2 3 3 0 0 1 0 6Z"/>"#
        }
        "Background color" | "Color" => {
            r#"<path d="M12 3S5 11 5 15a7 7 0 0 0 14 0c0-4-7-12-7-12Z"/>"#
        }
        _ => return IconSource::Symbol(Symbol::Sliders),
    };
    crate::icons::outline(glyph)
}

/// Form fields use a quieter surface than their containing card, with
/// compact numeric fields and enough width for text values.
pub fn form_input(input: TextInput, width: f32) -> TextInput {
    let theme = use_theme();
    input
        .width(width)
        .height(34.0)
        .font_size(13.0)
        .corner_radius(10.0)
        .background(theme.colors.surface.mix(theme.colors.surface_elevated, 0.5))
        .border(theme.colors.border, 1.0)
}

pub fn settings_theme(theme: creamui_theme::Theme) -> creamui_theme::Theme {
    coconut_plugin_kit::design::coconut_theme(theme)
}

pub fn load_settings_fonts() {
    coconut_plugin_kit::design::load_fonts();
}

pub fn icon_badge(icon: IconSource, color: creamui_theme::Color) -> BoxedWidget {
    coconut_plugin_kit::design::icon_badge(icon, color, 24.0)
}

pub fn back_button(on_click: impl Fn() + 'static) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawButton::new(
            creamui_core::layout::Style {
                size: creamui_widgets::layout::fixed(28.0, 28.0),
                flex_shrink: 0.0,
                align_items: Some(creamui_core::layout::AlignItems::Center),
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                ..Default::default()
            },
            on_click,
        )
        .corner_radius(8.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .pressed_style(
            creamui_core::StateStyle::new().background(theme.colors.selection_background),
        )
        .child(Box::new(
            Icon::new(Symbol::ChevronLeft, theme.colors.text_primary).size(20.0),
        )),
    )
}

/// The scrollable body of a settings page. Page chrome is owned by the
/// Settings window so every view has one consistent title bar.
pub fn section(title: &str, description: &str, body: Vec<BoxedWidget>) -> BoxedWidget {
    let page = creamui_reactive::try_use_context::<PageStyle>();
    let nested = !title.is_empty() && page.as_ref().is_some_and(|page| page.title != title);
    let mut children = Vec::new();
    if nested {
        children.push(section_label(title));
    }
    if !description.is_empty() {
        children.push(Box::new(Text::secondary(description).size(TextSize::Sm)) as BoxedWidget);
    }
    children.extend(body);
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={24.0} children={children} />
    })
}

/// The small all-caps label placed above a settings card in the mock layout.
pub fn section_label(label: &str) -> BoxedWidget {
    Box::new(
        Text::secondary(label.to_uppercase())
            .size(TextSize::Xs)
            .bold(true)
            .padding_left(4.0)
            .height(14.0),
    )
}

pub fn labeled_group(label: &str, card: BoxedWidget) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            {section_label(label)}
            {card}
        </Flex>
    })
}

/// A settings page with fixed navigation chrome and a scrollable body. The
/// body owns its bottom breathing room, so scrolling never clips the last
/// control against the viewport edge. Left/right padding matches every other
/// settings page's content inset, so the toolbar's own background (a
/// [`creamui_widgets::Tabs`] fills its full container) reads as a
/// deliberately placed card rather than a colored strip flush with the
/// window edge.
pub fn fixed_body(
    toolbar: BoxedWidget,
    body: BoxedWidget,
    scroll: ScrollController,
) -> BoxedWidget {
    let toolbar: BoxedWidget = Box::new(
        RawView::new(creamui_core::layout::Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(28.0),
                right: creamui_core::layout::LengthPercentage::Length(36.0),
                top: creamui_core::layout::LengthPercentage::Length(24.0),
                bottom: creamui_core::layout::LengthPercentage::Length(12.0),
            },
            ..Default::default()
        })
        .child(toolbar),
    );
    let body: BoxedWidget = Box::new(
        RawView::new(creamui_core::layout::Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(28.0),
                right: creamui_core::layout::LengthPercentage::Length(36.0),
                top: creamui_core::layout::LengthPercentage::Length(12.0),
                bottom: creamui_core::layout::LengthPercentage::Length(32.0),
            },
            ..Default::default()
        })
        .child(body),
    );
    let scroll: BoxedWidget = Box::new(
        RawScrollView::controlled(
            creamui_core::layout::Style {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                ..Default::default()
            },
            scroll,
        )
        .scrollbar_gap(12.0)
        .child(body),
    );
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} grow={1.0} gap={0.0}>
            {toolbar}
            {scroll}
        </Flex>
    })
}

/// A macOS-style grouped list: `rows` in a recessed card, each separated by
/// a thin divider rather than its own border/radius.
pub fn group(rows: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    let last = rows.len().saturating_sub(1);
    let mut children = Vec::with_capacity(rows.len() * 2);
    for (index, item) in rows.into_iter().enumerate() {
        children.push(item);
        if index != last {
            children.push(divider());
        }
    }
    Box::new(
        RawView::new(creamui_core::layout::Style {
            flex_direction: FlexDirection::Column,
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..Default::default()
        })
        // Cards sit one elevation above the cool page canvas, just like the
        // white/translucent cards in the concept.
        .background(theme.colors.surface_elevated)
        .outline(theme.colors.border, 1.0)
        .corner_radius(16.0)
        .box_shadow(coconut_plugin_kit::design::card_shadow())
        .with_children(children),
    )
}

/// One label-left, control-right row inside a [`group`].
pub fn row(label: &str, control: BoxedWidget) -> BoxedWidget {
    use creamui_core::layout::{AlignItems, LengthPercentage, Rect, Size, Style};
    let style = Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        min_size: Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(56.0),
        },
        gap: Size {
            width: LengthPercentage::Length(12.0),
            height: LengthPercentage::Length(0.0),
        },
        padding: Rect {
            left: LengthPercentage::Length(16.0),
            right: LengthPercentage::Length(16.0),
            top: LengthPercentage::Length(10.0),
            bottom: LengthPercentage::Length(10.0),
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={style}>
            {icon_badge(setting_icon(label), category_color())}
            <RawView style={Style { flex_grow: 1.0, flex_shrink: 1.0, min_size: Size { width: Dimension::Length(0.0), height: Dimension::Auto }, ..Default::default() }}>
                {Box::new(Text::new(label.to_owned())) as BoxedWidget}
            </RawView>
            <Flex shrink={0.0}>{control}</Flex>
        </RawView>
    })
}

/// [`TabColors::dark`] tints its container with `theme.surface` — the base
/// app canvas color, used behind the sidebar — but every settings page's
/// content instead sits on `theme.surface_elevated`. That mismatch is why a
/// tab bar showed a visibly different-colored rectangle in the gaps between
/// tabs, reading as a rendering bug rather than an intentional pill group.
/// This keeps the container blended into the page it's actually placed on.
pub fn tab_colors() -> TabColors {
    let theme = use_theme();
    let mut colors = TabColors::dark();
    colors.background = theme.surface_elevated;
    colors
}

fn divider() -> BoxedWidget {
    let theme = use_theme();
    let style = creamui_core::layout::Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(1.0),
        },
        ..Default::default()
    };
    Box::new(RawView::new(style).background(theme.border))
}

/// Writes `config` to `shell.toml`, logging (not panicking) on failure —
/// matches `ShellConfig::load`'s own tolerance for a config file that can't
/// be written.
pub fn persist(config: &Signal<ShellConfig>) -> bool {
    match config.peek().save() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("settings: failed to save shell.toml: {error}");
            false
        }
    }
}

/// Mutates the shared config and immediately persists the result.
pub fn update_config(config: &Signal<ShellConfig>, mutate: impl FnOnce(&mut ShellConfig)) {
    config.update(mutate);
    if persist(config) {
        if let Err(error) = coconut_core::ipc::publish_shell_config(&config.peek()) {
            eprintln!("settings: failed to notify the desktop process about the update: {error}");
        }
    }
}
