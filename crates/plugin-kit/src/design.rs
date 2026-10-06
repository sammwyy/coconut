//! Coconut's shared visual language. Settings and every shell plugin use
//! these tokens; native surface clipping is always left to the compositor.

use creamui_core::{BoxShadow, BoxedWidget, LinearGradient, StateStyle, Style, Styled};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::{Icon, IconSource, RawView};
use std::cell::Cell;

pub const FONT_FAMILY: &str = "Plus Jakarta Sans, system-ui";
pub const CARD_RADIUS: f32 = 16.0;
pub const CONTROL_RADIUS: f32 = 10.0;
pub const PILL_RADIUS: f32 = 16.0;

/// Apply Coconut's geometry and typography without replacing CreamUI's palette.
pub fn coconut_theme(mut theme: Theme) -> Theme {
    theme.typography.body = 13.0;
    theme.font_family = FONT_FAMILY;
    theme.spacing_large = 28.0;
    theme.button_radius = CONTROL_RADIUS;
    theme.input_radius = 12.0;
    theme.card_radius = CARD_RADIUS;
    theme.menu_radius = CARD_RADIUS;
    theme.menu_item_radius = CONTROL_RADIUS;
    theme
}

/// Explicit user selections take precedence over the default visual tokens.
pub fn coconut_appearance(appearance: &creamui_theme::ResolvedAppearance) -> Theme {
    let mut theme = coconut_theme(appearance.theme);
    if let Some(family) = appearance.font_family.as_deref() {
        use std::sync::{Mutex, OnceLock};
        static FAMILIES: OnceLock<Mutex<std::collections::BTreeMap<String, &'static str>>> =
            OnceLock::new();
        let mut families = FAMILIES
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        theme.font_family = *families
            .entry(family.to_owned())
            .or_insert_with(|| Box::leak(family.to_owned().into_boxed_str()));
        creamui_fonts::use_system_font(family);
    }
    theme
}

/// Fonts are registered per render thread, just like CreamUI's registry.
pub fn load_fonts() {
    thread_local! { static LOADED: Cell<bool> = const { Cell::new(false) }; }
    LOADED.with(|loaded| {
        if loaded.get() {
            return;
        }
        for (weight, bytes) in [
            (
                creamui_fonts::FontWeight::Regular,
                include_bytes!("../../../assets/fonts/plus-jakarta-sans/Regular.ttf").as_slice(),
            ),
            (
                creamui_fonts::FontWeight::Bold,
                include_bytes!("../../../assets/fonts/plus-jakarta-sans/SemiBold.ttf").as_slice(),
            ),
        ] {
            if let Err(error) = creamui_fonts::register_bytes("Plus Jakarta Sans", weight, bytes) {
                eprintln!("coconut: could not load interface font: {error}");
                return;
            }
        }
        loaded.set(true);
    });
    // Raw text and custom-painted clocks use the same family as themed text.
    creamui_fonts::set_preferred_family(Some(FONT_FAMILY.to_owned()));
}

pub fn island_surface(theme: Theme) -> Color {
    let color = theme.colors.surface;
    Color::rgba(
        color.r,
        color.g,
        color.b,
        if color.r > 128 {
            112
        } else if color.r <= 16 {
            234
        } else {
            214
        },
    )
}

pub fn dock_surface(theme: Theme) -> Color {
    let color = theme.colors.surface;
    Color::rgba(
        color.r,
        color.g,
        color.b,
        if color.r > 128 {
            166
        } else if color.r <= 16 {
            244
        } else {
            230
        },
    )
}

pub fn card_shadow() -> BoxShadow {
    BoxShadow::new(0.0, 1.0, 2.0, 0.0, Color::rgba(25, 35, 50, 10))
}

pub fn island_shadow(scale: f32) -> BoxShadow {
    BoxShadow::new(
        0.0,
        2.0 * scale,
        12.0 * scale,
        0.0,
        Color::rgba(25, 45, 65, 24),
    )
}

pub fn card_style(layout: creamui_core::layout::Style) -> Style {
    let theme = use_theme();
    Style::new()
        .layout(layout)
        .background(theme.colors.surface_elevated)
        .outline(theme.colors.border, 1.0)
        .corner_radius(CARD_RADIUS)
        .box_shadow(card_shadow())
}

pub fn control_style(layout: creamui_core::layout::Style) -> Style {
    let theme = use_theme();
    card_style(layout)
        .corner_radius(CONTROL_RADIUS)
        .hover(
            StateStyle::new()
                .background(theme.colors.surface_elevated.mix(theme.colors.accent, 0.08)),
        )
        .pressed(StateStyle::new().background(theme.colors.selection_background))
        .focus(StateStyle::new().outline(theme.colors.accent, 1.5))
}

pub fn icon_badge(icon: IconSource, color: Color, size: f32) -> BoxedWidget {
    let layout = creamui_core::layout::Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Length(size),
            height: creamui_core::layout::Dimension::Length(size),
        },
        flex_shrink: 0.0,
        align_items: Some(creamui_core::layout::AlignItems::Center),
        justify_content: Some(creamui_core::layout::JustifyContent::Center),
        ..Default::default()
    };
    Box::new(
        RawView::new(layout)
            .background(LinearGradient::new(
                180.0,
                color.mix(Color::rgb(255, 255, 255), 0.22),
                color,
            ))
            .corner_radius(if size <= 24.0 {
                size / 2.0
            } else {
                size * 0.28
            })
            .box_shadow(card_shadow())
            .child(Box::new(
                Icon::new(icon, Color::rgb(255, 255, 255)).size(size * 0.58),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstyled_shell_labels_measure_with_the_shared_font() {
        use creamui_core::Widget;
        load_fonts();
        let face = creamui_fonts::resolve(FONT_FAMILY, creamui_fonts::FontWeight::Regular);
        for label in [
            "Control center",
            "Bluetooth",
            "Brightness",
            "Volume",
            "Scanning",
        ] {
            let text = creamui_widgets::RawText::new(label, Color::rgb(0, 0, 0), 13.0);
            let measure = text.measure().unwrap();
            let measured = measure(
                creamui_core::layout::Size {
                    width: None,
                    height: None,
                },
                creamui_core::layout::Size {
                    width: creamui_core::layout::AvailableSpace::MaxContent,
                    height: creamui_core::layout::AvailableSpace::MaxContent,
                },
            );
            let layout = creamui_fonts::cached_layout(&face, label, 13.0, &Default::default());
            assert_eq!(
                measured.width,
                layout.lines[0].width.ceil(),
                "{label}: layout and paint must resolve the same face"
            );
        }
    }
    #[test]
    fn dark_is_neutral_and_midnight_remains_darker_after_reload() {
        let dark = coconut_theme(Theme::dark());
        let midnight = coconut_theme(Theme::midnight());
        for theme in [dark, midnight] {
            for color in [
                theme.colors.surface,
                theme.colors.surface_elevated,
                theme.colors.surface_hover,
                theme.colors.text_primary,
                theme.colors.text_secondary,
                theme.colors.border,
                theme.colors.border_strong,
            ] {
                assert_eq!(color.r, color.g, "neutral dark palette");
                assert_eq!(color.g, color.b, "neutral dark palette");
            }
            assert_eq!(
                coconut_theme(theme),
                theme,
                "reload keeps the chosen variant"
            );
        }
        assert!(midnight.colors.surface.r < dark.colors.surface.r);
        assert!(midnight.colors.surface_elevated.r < dark.colors.surface_elevated.r);
        assert!(island_surface(midnight).a > island_surface(dark).a);
        assert!(dock_surface(midnight).a > dock_surface(dark).a);
    }

    #[test]
    fn every_surface_uses_the_same_tokens_in_light_and_dark() {
        for base in [Theme::light(), Theme::dark(), Theme::midnight()] {
            let theme = coconut_theme(base);
            assert_eq!(theme.colors, base.colors, "use CreamUI's palette unchanged");
            assert_eq!(theme.font_family, FONT_FAMILY);
            assert_eq!(theme.card_radius, CARD_RADIUS);
            assert_eq!(theme.typography.body, 13.0);
            assert!(theme.colors.surface.a < 255);
            assert!(island_surface(theme).a < theme.colors.surface.a);
            assert!(dock_surface(theme).a < theme.colors.surface.a);
            assert_eq!(coconut_theme(theme), theme, "theme reload is idempotent");
        }
    }

    #[test]
    fn resolved_custom_theme_is_not_recolored_by_coconut() {
        let mut custom = Theme::dark().with_accent(Color::rgb(255, 117, 181));
        custom.colors.surface = Color::rgba(45, 28, 54, 222);
        custom.colors.surface_elevated = Color::rgba(62, 38, 73, 230);
        custom.colors.selection_text = Color::rgb(245, 227, 251);
        let appearance = creamui_theme::ResolvedAppearance {
            theme_id: "custom".into(),
            variant_id: "plum".into(),
            accent: custom.colors.accent,
            theme: custom,
            font_family: None,
            corners: creamui_theme::CornerStyle::Round,
        };
        assert_eq!(coconut_theme(custom).colors, custom.colors);
        assert_eq!(coconut_appearance(&appearance).colors, custom.colors);
    }
}
