use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Justify};
use creamui_widgets::{
    Icon, IconSource, RawButton, RawScrollView, RawView, ScrollController, Symbol, TabColors, Text,
    TextSize,
};

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
pub fn section(_: &str, _: &str, body: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_large} children={body} />
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
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} justify={Justify::Between} gap={theme.spacing_large} padding={16.0}>
            {Box::new(Text::new(label.to_owned())) as BoxedWidget}
            {control}
        </Flex>
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
