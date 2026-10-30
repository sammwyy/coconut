//! Preserve custom legacy CreamUI palettes when adopting Coconut packages.
use coconut_theme::{atomic_write, Color, Package, Store};
use std::fs;

pub fn import_legacy(store: &Store) -> Result<(), String> {
    if store.selection_path().exists() {
        return Ok(());
    }
    let Ok(source) = fs::read_to_string(store.paths.user_config_file("cream", "appearance.toml"))
    else {
        return Ok(());
    };
    let selection: toml::Value =
        toml::from_str(&source).map_err(|e| format!("invalid legacy appearance: {e}"))?;
    let Some(id) = selection
        .get("theme")
        .and_then(|v| v.as_str())
        .filter(|id| *id != "default")
    else {
        return Ok(());
    };
    if store.package(id).is_ok() {
        return Ok(());
    }
    coconut_theme::valid_id(id)?;
    let legacy = creamui_theme_loader::find_theme(id, &store.paths.data_search_paths())
        .map_err(|e| e.to_string())?;
    let destination = store.paths.data_home.join("coconut/themes").join(id);
    if destination.exists() {
        return Err(format!("cannot import over existing Coconut theme {id}"));
    }
    let temporary = destination.with_file_name(format!(".import-{id}-{}", std::process::id()));
    fs::create_dir_all(&temporary).map_err(|e| e.to_string())?;
    let result = (|| {
        let builtin = Package::builtin()?;
        atomic_write(
            &temporary.join("tokens.toml"),
            &builtin.read("tokens.toml")?,
        )?;
        atomic_write(&temporary.join("shell.toml"), &builtin.read("shell.toml")?)?;
        let mut manifest = toml::Table::new();
        manifest.insert("schema".into(), 1.into());
        manifest.insert("id".into(), id.into());
        manifest.insert("name".into(), legacy.name.into());
        manifest.insert("default_variant".into(), legacy.default_variant.into());
        manifest.insert("shell".into(), "shell.toml".into());
        let mut variants = toml::Table::new();
        for (variant, theme) in legacy.variants {
            coconut_theme::valid_id(&variant)?;
            let c = theme
                .with_accent(legacy.default_accent.unwrap_or(theme.colors.accent))
                .colors;
            let hex = |c: creamui_theme::Color| {
                Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                }
                .to_string()
            };
            let mut colors = toml::Table::new();
            macro_rules! export { ($($field:ident),*) => { $(colors.insert(stringify!($field).into(), hex(c.$field).into());)* } }
            export!(
                surface,
                surface_elevated,
                surface_hover,
                accent,
                accent_hover,
                accent_pressed,
                selection_background,
                selection_text,
                text_primary,
                text_secondary,
                text_disabled,
                border,
                border_strong,
                danger,
                warning,
                success
            );
            let accent = Color {
                r: c.accent.r,
                g: c.accent.g,
                b: c.accent.b,
                a: c.accent.a,
            };
            colors.insert("on_accent".into(), accent.foreground().to_string().into());
            let document = toml::Table::from_iter([("colors".into(), colors.into())]);
            let relative = format!("variants/{variant}.toml");
            atomic_write(
                &temporary.join(&relative),
                &toml::to_string_pretty(&document).map_err(|e| e.to_string())?,
            )?;
            let dark = u32::from(c.surface.r) * 299
                + u32::from(c.surface.g) * 587
                + u32::from(c.surface.b) * 114
                < 128_000;
            variants.insert(
                variant,
                toml::Table::from_iter([
                    (
                        "appearance".into(),
                        if dark { "dark" } else { "light" }.into(),
                    ),
                    ("file".into(), relative.into()),
                ])
                .into(),
            );
        }
        manifest.insert("variants".into(), variants.into());
        manifest.insert(
            "accent".into(),
            toml::Table::from_iter([
                ("customizable".into(), true.into()),
                (
                    "presets".into(),
                    toml::Value::Array(
                        legacy
                            .accent_presets
                            .into_iter()
                            .map(|p| {
                                Color {
                                    r: p.color.r,
                                    g: p.color.g,
                                    b: p.color.b,
                                    a: p.color.a,
                                }
                                .to_string()
                                .into()
                            })
                            .collect(),
                    ),
                ),
            ])
            .into(),
        );
        atomic_write(
            &temporary.join("theme.toml"),
            &toml::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
        )?;
        // Validate every imported variant before making the package discoverable.
        let package = Package::open(&temporary)?;
        for variant in package.manifest.variants.keys() {
            package.resolve(&coconut_theme::Selection {
                theme: id.into(),
                variant: variant.clone(),
                ..Default::default()
            })?;
        }
        fs::rename(&temporary, &destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}
