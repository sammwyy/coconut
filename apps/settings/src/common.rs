use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Justify};
use creamui_widgets::{Heading, RawView, Surface, SurfaceRole, Text, TextSize};

/// A section's title, subtitle, and body — one or more [`group`]s, usually.
pub fn section(title: &str, subtitle: &str, body: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_large}>
            {Box::new(Heading::xl(title.to_owned())) as BoxedWidget}
            {Box::new(Text::secondary(subtitle.to_owned()).size(TextSize::Sm)) as BoxedWidget}
            <Flex direction={FlexDirection::Column} gap={theme.spacing_large} children={body} />
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
        <Flex direction={FlexDirection::Row} align={Align::Center} justify={Justify::Between} gap={theme.spacing_medium} padding={14.0}>
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
pub fn persist(config: &Signal<ShellConfig>) {
    if let Err(error) = config.peek().save() {
        eprintln!("settings: failed to save shell.toml: {error}");
    }
}

/// Mutates the shared config and immediately persists the result.
pub fn update_config(config: &Signal<ShellConfig>, mutate: impl FnOnce(&mut ShellConfig)) {
    config.update(mutate);
    persist(config);
}
