use creamui_core::layout::FlexDirection;
use creamui_core::BoxedWidget;
use creamui_macros::jsx;
use creamui_theme::use_theme;
use creamui_widgets::{Heading, Text, TextSize};
use coconut_core::ShellConfig;
use creamui_reactive::Signal;

/// A section's title, subtitle, and body rows, laid out consistently across
/// every settings page.
pub fn section(title: &str, subtitle: &str, body: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_large} padding={24.0} grow={1.0}>
            {Box::new(Heading::xl(title.to_owned())) as BoxedWidget}
            {Box::new(Text::secondary(subtitle.to_owned()).size(TextSize::Sm)) as BoxedWidget}
            <Flex direction={FlexDirection::Column} gap={theme.spacing_medium} children={body} />
        </Flex>
    })
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
