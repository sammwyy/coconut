use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Justify};
use creamui_widgets::{RawScrollView, RawView, ScrollController, Surface, SurfaceRole, Text};

/// The scrollable body of a settings page. Page chrome is owned by the
/// Settings window so every view has one consistent title bar.
pub fn section(_: &str, _: &str, body: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_large} children={body} />
    })
}

/// A settings page with fixed navigation chrome and a scrollable body. The
/// body owns its bottom breathing room, so scrolling never clips the last
/// control against the viewport edge.
pub fn fixed_body(
    toolbar: BoxedWidget,
    body: BoxedWidget,
    scroll: ScrollController,
) -> BoxedWidget {
    let body: BoxedWidget = Box::new(
        RawView::new(creamui_core::layout::Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentage::Length(2.0),
                right: creamui_core::layout::LengthPercentage::Length(16.0),
                top: creamui_core::layout::LengthPercentage::Length(20.0),
                bottom: creamui_core::layout::LengthPercentage::Length(28.0),
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
    let last = rows.len().saturating_sub(1);
    let mut children = Vec::with_capacity(rows.len() * 2);
    for (index, item) in rows.into_iter().enumerate() {
        children.push(item);
        if index != last {
            children.push(divider());
        }
    }
    Box::new(
        Surface::new(
            SurfaceRole::Inset,
            creamui_core::layout::Style {
                flex_direction: FlexDirection::Column,
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..Default::default()
            },
        )
        .with_children(children),
    )
}

/// One label-left, control-right row inside a [`group`].
pub fn row(label: &str, control: BoxedWidget) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} justify={Justify::Between} gap={theme.spacing_large} padding={18.0}>
            {Box::new(Text::new(label.to_owned())) as BoxedWidget}
            {control}
        </Flex>
    })
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
