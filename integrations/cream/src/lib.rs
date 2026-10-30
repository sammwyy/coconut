//! Translate Coconut's effective tokens to a native CreamUI appearance.
mod migration;
use coconut_theme::{Corners, Package, ResolvedTheme, Selection, Store};
use creamui_theme::{
    AccentPreset, AppearanceSelection, Color, ColorScheme, CornerStyle, ResolvedAppearance, Theme,
    ThemeDefinition, Typography,
};
pub use migration::import_legacy;
use std::collections::BTreeMap;

fn color(c: coconut_theme::Color) -> Color {
    Color::rgba(c.r, c.g, c.b, c.a)
}

pub fn appearance(resolved: &ResolvedTheme) -> ResolvedAppearance {
    let tokens = &resolved.adapters["cream"].tokens;
    let c = &tokens.colors;
    let g = &tokens.geometry;
    let t = &tokens.typography;
    let theme = Theme {
        colors: ColorScheme {
            surface: color(c.surface),
            surface_elevated: color(c.surface_elevated),
            surface_hover: color(c.surface_hover),
            accent: color(c.accent),
            accent_hover: color(c.accent_hover),
            accent_pressed: color(c.accent_pressed),
            selection_background: color(c.selection_background),
            selection_text: color(c.selection_text),
            text_primary: color(c.text_primary),
            text_secondary: color(c.text_secondary),
            text_disabled: color(c.text_disabled),
            border: color(c.border),
            border_strong: color(c.border_strong),
            danger: color(c.danger),
            warning: color(c.warning),
            success: color(c.success),
        },
        typography: Typography {
            caption: t.caption_size,
            body: t.body_size,
            section: t.section_size,
            title: t.title_size,
        },
        font_family: creamui_theme_loader::intern_font_family(&t.family),
        radius_small: g.radius_small,
        radius_medium: g.radius_medium,
        radius_large: g.radius_large,
        button_radius: g.control_radius,
        checkbox_radius: g.control_radius,
        input_radius: g.input_radius,
        textarea_radius: g.input_radius,
        input_border_width: g.border_width,
        card_radius: g.card_radius,
        menu_radius: g.menu_radius,
        menu_item_radius: g.menu_item_radius,
        scroll_radius: g.radius_small,
        tabs_radius: g.radius_medium,
        tab_radius: g.radius_small,
        sidebar_radius: g.card_radius,
        sidebar_item_radius: g.control_radius,
        sidebar_icon_radius: g.control_radius,
        spacing_small: g.spacing_small,
        spacing_medium: g.spacing_medium,
        spacing_large: g.spacing_large,
        text_scale: resolved.selection.accessibility.text_scale,
        high_contrast: resolved.selection.accessibility.high_contrast,
        ..Theme::default()
    };
    ResolvedAppearance {
        theme_id: resolved.theme_id.clone(),
        variant_id: resolved.variant_id.clone(),
        accent: color(c.accent),
        corners: corner(resolved.selection.overrides.corners),
        font_family: Some(t.family.clone()),
        theme,
    }
}

fn corner(corners: Option<Corners>) -> CornerStyle {
    match corners {
        Some(Corners::Square) => CornerStyle::Square,
        Some(Corners::Soft) => CornerStyle::Soft,
        _ => CornerStyle::Round,
    }
}

pub fn load_system_appearance() -> Result<ResolvedAppearance, String> {
    let store = Store::default();
    match import_legacy(&store).and_then(|()| store.load()) {
        Ok(resolved) => Ok(appearance(&resolved)),
        Err(error) => creamui_theme_loader::load_appearance_snapshot(
            &store.generated_dir().join("cream.toml"),
        )
        .map_err(|_| error),
    }
}

pub fn publish_native(store: &Store, resolved: &ResolvedTheme) -> Result<bool, String> {
    let native = creamui_theme_loader::appearance_snapshot(&appearance(resolved))
        .map_err(|e| e.to_string())?;
    coconut_theme::atomic_write(&store.generated_dir().join("cream.toml"), &native)
}

/// Translate the UI's optional selections without persisting framework state.
pub fn selection_for(store: &Store, next: &AppearanceSelection) -> Result<Selection, String> {
    // A complete explicit UI selection can also repair a malformed file.
    let mut selection = store.selection().or_else(|error| {
        if next.theme.is_some() && next.variant.is_some() {
            Ok(Selection::default())
        } else {
            Err(error)
        }
    })?;
    if let Some(id) = &next.theme {
        selection.theme = id.clone();
    }
    if let Some(id) = &next.variant {
        selection.variant = id.clone();
    }
    selection.overrides.accent = next.accent.map(|c| coconut_theme::Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    });
    selection.overrides.font_family = next.font_family.clone();
    selection.overrides.corners = next.corners.map(|c| match c {
        CornerStyle::Square => Corners::Square,
        CornerStyle::Soft => Corners::Soft,
        CornerStyle::Round => Corners::Round,
    });
    Ok(selection)
}

pub fn definition(package: &Package) -> Result<ThemeDefinition, String> {
    let mut variants = BTreeMap::new();
    for id in package.manifest.variants.keys() {
        let selection = Selection {
            theme: package.manifest.id.clone(),
            variant: id.clone(),
            ..Selection::default()
        };
        variants.insert(id.clone(), appearance(&package.resolve(&selection)?).theme);
    }
    Ok(ThemeDefinition {
        id: package.manifest.id.clone(),
        name: package.manifest.name.clone(),
        default_variant: package.manifest.default_variant.clone(),
        variants,
        accent_presets: package
            .manifest
            .accent
            .presets
            .iter()
            .enumerate()
            .map(|(i, c)| AccentPreset {
                id: format!("accent-{i}"),
                name: c.to_string(),
                color: color(*c),
            })
            .collect(),
        default_accent: None,
    })
}

pub fn builtin_theme() -> ThemeDefinition {
    definition(&Package::builtin().expect("valid bundled Coconut theme"))
        .expect("valid bundled Coconut variants")
}

pub fn list_themes() -> Result<Vec<ThemeDefinition>, String> {
    let store = Store::default();
    import_legacy(&store)?;
    store.packages()?.iter().map(definition).collect()
}

pub fn supports_custom_accent(id: &str) -> bool {
    Store::default()
        .package(id)
        .is_ok_and(|p| p.manifest.accent.customizable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn store(root: &std::path::Path) -> Store {
        let mut store = Store::default();
        store.paths.config_home = root.join("config");
        store.paths.data_home = root.join("data");
        store.paths.data_dirs.clear();
        store.paths.config_dirs.clear();
        store
    }

    #[test]
    fn exported_native_source_preserves_the_effective_style() {
        let root = tempfile::tempdir().unwrap();
        let store = store(root.path());
        let mut selection = Selection::default();
        selection.overrides.corners = Some(Corners::Square);
        selection.accessibility.text_scale = 1.5;
        let resolved = store.apply(&selection).unwrap();
        assert!(publish_native(&store, &resolved).unwrap());
        assert!(!publish_native(&store, &resolved).unwrap());
        let native = creamui_theme_loader::load_appearance_snapshot(
            &store.generated_dir().join("cream.toml"),
        )
        .unwrap();
        assert_eq!(native, appearance(&resolved));
        assert_eq!(native.theme.sidebar_radius, 0.);
        assert_eq!(native.theme.button_radius, 0.);
        assert_eq!(native.theme.typography.body, 19.5);
    }

    #[test]
    fn custom_legacy_palettes_are_imported_without_touching_the_original() {
        let root = tempfile::tempdir().unwrap();
        let store = store(root.path());
        let original = store.paths.data_home.join("themes/legacy/cream/theme.toml");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        let resolved = Package::builtin()
            .unwrap()
            .resolve(&Selection::default())
            .unwrap();
        let colors = toml::Value::try_from(&resolved.tokens.colors).unwrap();
        let source = toml::Table::from_iter([
            ("id".into(), "legacy".into()),
            ("name".into(), "Legacy".into()),
            ("default_variant".into(), "ink".into()),
            ("default_accent".into(), "#cc11ee".into()),
            (
                "variants".into(),
                toml::Table::from_iter([(
                    "ink".into(),
                    toml::Table::from_iter([("colors".into(), colors)]).into(),
                )])
                .into(),
            ),
        ]);
        let source = toml::to_string_pretty(&source).unwrap();
        fs::write(&original, &source).unwrap();
        let legacy = store.paths.user_config_file("cream", "appearance.toml");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, "theme = 'legacy'\nvariant = 'ink'\n").unwrap();
        import_legacy(&store).unwrap();
        let current = store.ensure_selection().unwrap();
        assert_eq!(current.theme_id, "legacy");
        assert_eq!(current.variant_id, "ink");
        assert_eq!(current.tokens.colors.accent.to_string(), "#cc11ee");
        assert_eq!(fs::read_to_string(original).unwrap(), source);
        import_legacy(&store).unwrap();
    }
}
