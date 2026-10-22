use crate::routes::destination::Section;
use coconut_core::{BackgroundConfig, ShellConfig, WallpaperSource};
use creamui_core::BoxedWidget;
use creamui_reactive::Signal;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub mod appearance;
pub mod asset_packs;
pub mod wallpaper;

pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let config = &context.config;
    let background = &context.background;
    let appearance = &context.appearance;
    let view = &context.view;
    let window = &context.window;
    let appearance_accent_picker = &context.appearance_accent_picker;
    let appearance_custom_accent = &context.appearance_custom_accent;
    let native = &context.native;
    build(
        native,
        config,
        background,
        appearance,
        view,
        crate::views::personalization::appearance::overview(
            appearance,
            window,
            appearance_accent_picker,
            appearance_custom_accent,
        ),
    )
}

fn build(
    state: &State,
    config: &Signal<ShellConfig>,
    background: &Signal<BackgroundConfig>,
    appearance: &Signal<creamui_theme::ResolvedAppearance>,
    nav: &creamui_router::Router,
    appearance_controls: crate::views::personalization::appearance::OverviewControls,
) -> BoxedWidget {
    let config = config.get();
    let background = background.get();
    let selected = appearance.get();
    let crate::views::personalization::appearance::OverviewControls {
        theme,
        style,
        accent,
        corners,
    } = appearance_controls;
    page(
        vec![
            card(
                "Appearance",
                vec![
                    theme,
                    style,
                    link(
                        "Wallpaper",
                        match background.source {
                            WallpaperSource::SolidColor => "Solid color".into(),
                            WallpaperSource::Gradient => "Gradient".into(),
                            WallpaperSource::Shader => {
                                format!("Shader · {}", background.shader.name())
                            }
                            WallpaperSource::Image => background
                                .wallpaper
                                .as_ref()
                                .and_then(|p| p.file_stem())
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Image, slideshow or solid color".into()),
                        },
                        Section::Wallpaper,
                        nav,
                    ),
                    accent,
                ],
            ),
            card(
                "Details",
                vec![
                    link(
                        "Icon pack",
                        config.appearance.icon_theme,
                        Section::IconPack,
                        nav,
                    ),
                    link(
                        "Cursor",
                        config.appearance.cursor_theme,
                        Section::CursorTheme,
                        nav,
                    ),
                    detail_link(
                        "Fonts",
                        format!(
                            "Interface · {} {}",
                            selected
                                .font_family
                                .unwrap_or_else(|| "Plus Jakarta Sans".into()),
                            creamui_theme::use_theme().typography.body
                        ),
                        Page::Fonts,
                        nav,
                    ),
                    link(
                        "Sound pack",
                        config.appearance.sound_theme,
                        Section::Sound,
                        nav,
                    ),
                ],
            ),
            card("Additional settings", vec![corners]),
        ],
        state,
    )
}
pub mod fonts;
