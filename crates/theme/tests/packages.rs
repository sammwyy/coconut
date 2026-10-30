use coconut_theme::{Color, Corners, Package, Scheme, Selection, Store};
use delib::xdg::XdgPaths;
use std::{fs, path::Path};

fn package(root: &Path) -> Package {
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/coconut");
    fs::create_dir_all(root.join("variants")).unwrap();
    for path in [
        "theme.toml",
        "tokens.toml",
        "shell.toml",
        "variants/light.toml",
        "variants/dark.toml",
        "variants/midnight.toml",
    ] {
        fs::copy(bundled.join(path), root.join(path)).unwrap();
    }
    Package::open(root).unwrap()
}

#[test]
fn bundled_variants_and_user_overrides_resolve_once() {
    let package = Package::builtin().unwrap();
    let dark = package.resolve(&Selection::default()).unwrap();
    let midnight = package
        .resolve(&Selection {
            variant: "midnight".into(),
            ..Selection::default()
        })
        .unwrap();
    assert_eq!(midnight.scheme, Scheme::Dark);
    assert!(midnight.tokens.colors.surface.r < dark.tokens.colors.surface.r);
    let mut selection = Selection::default();
    selection.overrides.accent = Some(Color::rgb(255, 255, 0));
    selection.overrides.font_family = Some("Custom Sans".into());
    selection.overrides.corners = Some(Corners::Soft);
    selection.accessibility.text_scale = 1.5;
    let theme = package.resolve(&selection).unwrap();
    for adapter in theme.adapters.values() {
        assert_eq!(adapter.tokens.colors.accent, Color::rgb(255, 255, 0));
        assert_eq!(adapter.tokens.colors.on_accent, Color::rgb(0, 0, 0));
        assert_eq!(adapter.tokens.geometry.control_radius, 5.);
        assert_eq!(adapter.tokens.typography.body_size, 19.5);
        assert_eq!(adapter.tokens.typography.family, "Custom Sans");
    }
    assert_eq!(theme.shell.dock_radius, 8.);
    selection.accessibility.high_contrast = true;
    let theme = package.resolve(&selection).unwrap();
    assert_eq!(theme.shell.dock_opacity, 1.);
    assert!(!theme.shell.blur);
    assert_eq!(theme.tokens.colors.text_primary, Color::rgb(255, 255, 255));
}

#[test]
fn manual_templates_and_typed_mappings_receive_effective_values() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    package(root);
    fs::create_dir(root.join("gtk")).unwrap();
    fs::write(root.join("gtk/tokens.toml"), "[geometry]\ncontrol_radius = 24\n[typography]\nbody_size = '${tokens.typography.body_size}'\n").unwrap();
    fs::write(root.join("gtk/style.css.in"), "/* ${appearance.variant}: ${appearance.color_scheme} */\nbutton { color: ${tokens.colors.accent}; border-radius: ${tokens.geometry.control_radius}px; }\n").unwrap();
    let manifest = fs::read_to_string(root.join("theme.toml")).unwrap();
    fs::write(root.join("theme.toml"), manifest + "\n[adapters.gtk]\ntokens = 'gtk/tokens.toml'\ngtk3 = 'gtk/style.css.in'\ngtk4 = 'gtk/style.css.in'\n").unwrap();
    let package = Package::open(root).unwrap();
    let mut selection = Selection {
        variant: "midnight".into(),
        ..Selection::default()
    };
    selection.overrides.accent = Some(Color::rgb(255, 0, 0));
    selection.overrides.corners = Some(Corners::Soft);
    selection.accessibility.text_scale = 1.5;
    let theme = package.resolve(&selection).unwrap();
    assert_eq!(
        theme.adapters["gtk"].tokens.typography.body_size, 19.5,
        "scale must apply once, including typed references"
    );
    assert_eq!(theme.adapters["gtk"].tokens.geometry.control_radius, 12.);
    assert_eq!(theme.adapters["cream"].tokens.geometry.control_radius, 5.);
    let css = theme.adapters["gtk"].gtk3.as_ref().unwrap();
    assert!(css.contains("midnight: dark"));
    assert!(css.contains("#ff0000"));
    assert!(css.contains("12px"));
}

#[test]
fn invalid_packages_or_variables_cannot_replace_the_saved_selection() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let store = Store {
        paths: XdgPaths {
            config_home: root.join("config"),
            data_home: root.join("data"),
            config_dirs: vec![],
            data_dirs: vec![],
        },
    };
    store.apply(&Selection::default()).unwrap();
    let saved = fs::read(store.selection_path()).unwrap();
    let path = store.paths.data_home.join("coconut/themes/custom");
    package(&path);
    let manifest = fs::read_to_string(path.join("theme.toml"))
        .unwrap()
        .replace("id = \"coconut\"", "id = \"custom\"");
    fs::write(
        path.join("theme.toml"),
        manifest + "\n[adapters.gtk]\ngtk3 = 'bad.css.in'\n",
    )
    .unwrap();
    fs::write(
        path.join("bad.css.in"),
        "button { color: ${tokens.colors.typo}; }",
    )
    .unwrap();
    assert!(store
        .apply(&Selection {
            theme: "custom".into(),
            ..Selection::default()
        })
        .unwrap_err()
        .contains("unknown theme variable"));
    assert_eq!(fs::read(store.selection_path()).unwrap(), saved);
    assert!(store
        .apply(&Selection {
            variant: "missing".into(),
            ..Selection::default()
        })
        .is_err());
    assert_eq!(fs::read(store.selection_path()).unwrap(), saved);
    assert!(store.package("../escape").is_err());
    assert!(store.package("custom").unwrap().read("../escape").is_err());
}

#[test]
fn legacy_selection_migrates_without_modifying_creamui_preferences() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let paths = XdgPaths {
        config_home: root.join("config"),
        data_home: root.join("data"),
        config_dirs: vec![],
        data_dirs: vec![],
    };
    fs::create_dir_all(paths.config_home.join("cream")).unwrap();
    let old = "theme = 'default'\nvariant = 'midnight'\naccent = '#abcdef'\nfont_family = 'Inter'\ncorners = 'soft'\n";
    fs::write(paths.config_home.join("cream/appearance.toml"), old).unwrap();
    let store = Store { paths };
    let resolved = store.ensure_selection().unwrap();
    assert_eq!(resolved.theme_id, "coconut");
    assert_eq!(resolved.variant_id, "midnight");
    assert_eq!(resolved.tokens.colors.accent, "#abcdef".parse().unwrap());
    assert!(store.selection_path().is_file());
    assert_eq!(
        fs::read_to_string(store.paths.config_home.join("cream/appearance.toml")).unwrap(),
        old
    );
}

#[test]
fn default_variant_and_declared_appearance_do_not_depend_on_names_or_brightness() {
    let root = tempfile::tempdir().unwrap();
    package(root.path());
    let manifest = fs::read_to_string(root.path().join("theme.toml"))
        .unwrap()
        .replace("default_variant = \"dark\"", "default_variant = \"paper\"")
        + "\n[variants.paper]\nappearance = 'light'\nfile = 'variants/dark.toml'\n";
    fs::write(root.path().join("theme.toml"), manifest).unwrap();
    let package = Package::open(root.path()).unwrap();
    let selection: Selection = toml::from_str("theme = 'coconut'\n").unwrap();
    let resolved = package.resolve(&selection).unwrap();
    assert_eq!(resolved.variant_id, "paper");
    assert_eq!(resolved.scheme, Scheme::Light);
    assert_eq!(resolved.tokens.colors.surface.r, 20);
}
