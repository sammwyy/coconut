//! Desktop theme packages, effective tokens and user preferences owned by
//! Coconut. This crate has no dependency on a UI framework or a session bus.
mod color;
pub mod template;
mod tokens;
pub use color::Color;
pub use tokens::{Colors, Geometry, ShellTokens, Tokens, Typography};

use delib::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use template::{flatten, merge, render_text, render_value, Context};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Light,
    Dark,
}
impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Corners {
    Square,
    Soft,
    Round,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corners: Option<Corners>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Accessibility {
    pub high_contrast: bool,
    pub text_scale: f32,
}
impl Default for Accessibility {
    fn default() -> Self {
        Self {
            high_contrast: false,
            text_scale: 1.,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub theme: String,
    #[serde(default)]
    pub variant: String,
    pub overrides: Overrides,
    pub accessibility: Accessibility,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            theme: "coconut".into(),
            variant: "dark".into(),
            overrides: Overrides::default(),
            accessibility: Accessibility::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub default_variant: String,
    #[serde(default = "tokens_file")]
    pub tokens: String,
    #[serde(default)]
    pub shell: Option<String>,
    pub variants: BTreeMap<String, Variant>,
    #[serde(default)]
    pub accent: Accent,
    #[serde(default)]
    pub adapters: BTreeMap<String, Adapter>,
}
fn tokens_file() -> String {
    "tokens.toml".into()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub appearance: Scheme,
    pub file: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Accent {
    pub customizable: bool,
    pub presets: Vec<Color>,
}
impl Default for Accent {
    fn default() -> Self {
        Self {
            customizable: true,
            presets: vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterBase {
    #[default]
    Auto,
    Manual,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Adapter {
    pub base: AdapterBase,
    pub tokens: Option<String>,
    pub gtk3: Option<String>,
    pub gtk4: Option<String>,
    pub palette: Option<String>,
    pub stylesheet: Option<String>,
    pub variants: BTreeMap<String, AdapterOverride>,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdapterOverride {
    pub base: Option<AdapterBase>,
    pub tokens: Option<String>,
    pub gtk3: Option<String>,
    pub gtk4: Option<String>,
    pub palette: Option<String>,
    pub stylesheet: Option<String>,
}
impl Adapter {
    fn for_variant(&self, variant: &str) -> Self {
        let mut result = self.clone();
        if let Some(next) = self.variants.get(variant) {
            if let Some(base) = next.base {
                result.base = base;
            }
            macro_rules! replace { ($($field:ident),*) => { $(if next.$field.is_some() { result.$field = next.$field.clone(); })* } }
            replace!(tokens, gtk3, gtk4, palette, stylesheet);
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct Package {
    pub root: Option<PathBuf>,
    pub manifest: Manifest,
}

impl Package {
    pub fn builtin() -> Result<Self, String> {
        Self::parse(None, include_str!("../../../themes/coconut/theme.toml"))
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        Self::parse(
            Some(path.to_owned()),
            &fs::read_to_string(path.join("theme.toml"))
                .map_err(|e| format!("{}: {e}", path.display()))?,
        )
    }
    fn parse(root: Option<PathBuf>, source: &str) -> Result<Self, String> {
        let manifest: Manifest = toml::from_str(source).map_err(|e| e.to_string())?;
        if manifest.schema != 1 {
            return Err(format!(
                "unsupported Coconut theme schema {}",
                manifest.schema
            ));
        }
        valid_id(&manifest.id)?;
        if manifest.name.trim().is_empty()
            || !manifest.variants.contains_key(&manifest.default_variant)
        {
            return Err("a theme needs a name and an existing default variant".into());
        }
        for id in manifest.variants.keys() {
            valid_id(id)?;
        }
        for (name, adapter) in &manifest.adapters {
            if !["cream", "gtk", "qt"].contains(&name.as_str()) {
                return Err(format!("unknown adapter {name}"));
            }
            for variant in adapter.variants.keys() {
                if !manifest.variants.contains_key(variant) {
                    return Err(format!("adapter {name} names an unknown variant {variant}"));
                }
            }
        }
        Ok(Self { root, manifest })
    }
    pub fn read(&self, relative: &str) -> Result<String, String> {
        if Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!(
                "theme resource must be a relative path inside the package: {relative}"
            ));
        }
        if let Some(root) = &self.root {
            let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
            let path =
                fs::canonicalize(root.join(relative)).map_err(|e| format!("{relative}: {e}"))?;
            if !path.starts_with(&root) {
                return Err(format!("theme resource escapes its package: {relative}"));
            }
            fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        } else {
            let source = match relative {
                "tokens.toml" => include_str!("../../../themes/coconut/tokens.toml"),
                "shell.toml" => include_str!("../../../themes/coconut/shell.toml"),
                "variants/light.toml" => {
                    include_str!("../../../themes/coconut/variants/light.toml")
                }
                "variants/dark.toml" => include_str!("../../../themes/coconut/variants/dark.toml"),
                "variants/midnight.toml" => {
                    include_str!("../../../themes/coconut/variants/midnight.toml")
                }
                _ => return Err(format!("missing built-in resource {relative}")),
            };
            Ok(source.into())
        }
    }
    fn document(&self, relative: &str) -> Result<toml::Value, String> {
        toml::from_str(&self.read(relative)?).map_err(|e| format!("{relative}: {e}"))
    }

    pub fn resolve(&self, selection: &Selection) -> Result<ResolvedTheme, String> {
        if selection.variant.is_empty() {
            let mut selected = selection.clone();
            selected.variant = self.manifest.default_variant.clone();
            return self.resolve(&selected);
        }
        let variant = self
            .manifest
            .variants
            .get(&selection.variant)
            .ok_or_else(|| {
                format!(
                    "theme {} has no variant {}",
                    self.manifest.id, selection.variant
                )
            })?;
        if !selection.accessibility.text_scale.is_finite()
            || !(0.75..=2.0).contains(&selection.accessibility.text_scale)
        {
            return Err("text_scale must be between 0.75 and 2".into());
        }
        if !self.manifest.accent.customizable && selection.overrides.accent.is_some() {
            return Err("this theme does not support a custom accent".into());
        }
        let mut base = self.document(&self.manifest.tokens)?;
        let mut variant_data = self.document(&variant.file)?;
        let shell_override = variant_data.as_table_mut().and_then(|v| v.remove("shell"));
        merge(&mut base, variant_data);
        let authored: Tokens = base
            .clone()
            .try_into()
            .map_err(|e| format!("invalid tokens: {e}"))?;
        let mut adapter_context = context_for(selection, variant.appearance, &authored)?;
        self.resource_context(&mut adapter_context)?;
        let mut tokens = authored;
        tokens.customize(
            &selection.overrides,
            &selection.accessibility,
            variant.appearance,
        );
        tokens.validate()?;
        adapter_context.insert(
            "appearance.accent".into(),
            tokens.colors.accent.to_string().into(),
        );
        let mut context = context_for(selection, variant.appearance, &tokens)?;
        self.resource_context(&mut context)?;
        let shell_source = if let Some(path) = &self.manifest.shell {
            self.document(path)?
        } else {
            toml::from_str(include_str!("../../../themes/coconut/shell.toml"))
                .map_err(|e| e.to_string())?
        };
        let mut shell_source = shell_source;
        if let Some(overlay) = shell_override {
            merge(&mut shell_source, overlay);
        }
        let mut shell: ShellTokens = render_value(&shell_source, &context)?
            .try_into()
            .map_err(|e| format!("invalid shell tokens: {e}"))?;
        if selection.accessibility.high_contrast {
            shell.dock_background = tokens.colors.surface;
            shell.island_background = tokens.colors.surface;
            shell.dock_opacity = 1.;
            shell.island_opacity = 1.;
            shell.blur = false;
        }
        if let Some(corners) = selection.overrides.corners {
            let factor = match corners {
                Corners::Square => 0.,
                Corners::Soft => 0.5,
                Corners::Round => 1.,
            };
            // Shell defaults already reference resolved radii; only multiply
            // explicit authored radii, avoiding double scaling references.
            for name in ["dock_radius", "island_radius"] {
                if shell_source
                    .get(name)
                    .is_some_and(|v| !v.as_str().is_some_and(|s| s.contains("${")))
                {
                    if name == "dock_radius" {
                        shell.dock_radius *= factor;
                    } else {
                        shell.island_radius *= factor;
                    }
                }
            }
        }
        shell.validate()?;
        let mut adapters = BTreeMap::new();
        for target in ["cream", "gtk", "qt"] {
            let spec = self
                .manifest
                .adapters
                .get(target)
                .cloned()
                .unwrap_or_default()
                .for_variant(&selection.variant);
            let mut adapted = base.clone();
            if let Some(path) = &spec.tokens {
                merge(
                    &mut adapted,
                    render_value(&self.document(path)?, &adapter_context)?,
                );
            }
            let mut target_tokens: Tokens = adapted
                .try_into()
                .map_err(|e| format!("invalid {target} tokens: {e}"))?;
            target_tokens.customize(
                &selection.overrides,
                &selection.accessibility,
                variant.appearance,
            );
            target_tokens.validate()?;
            let mut target_context = context_for(selection, variant.appearance, &target_tokens)?;
            self.resource_context(&mut target_context)?;
            let read_template = |path: &Option<String>| {
                path.as_ref()
                    .map(|p| self.read(p).and_then(|s| render_text(&s, &target_context)))
                    .transpose()
            };
            let palette = spec
                .palette
                .as_ref()
                .map(|p| {
                    self.document(p)
                        .and_then(|v| render_value(&v, &target_context))
                })
                .transpose()?;
            if spec.base == AdapterBase::Manual
                && ((target == "gtk" && (spec.gtk3.is_none() || spec.gtk4.is_none()))
                    || (target == "qt" && palette.is_none()))
            {
                return Err(format!(
                    "manual {target} adapter needs complete native resources"
                ));
            }
            adapters.insert(
                target.into(),
                ResolvedAdapter {
                    base: spec.base,
                    tokens: target_tokens,
                    gtk3: read_template(&spec.gtk3)?,
                    gtk4: read_template(&spec.gtk4)?,
                    palette,
                    stylesheet: read_template(&spec.stylesheet)?,
                },
            );
        }
        Ok(ResolvedTheme {
            theme_id: self.manifest.id.clone(),
            name: self.manifest.name.clone(),
            variant_id: selection.variant.clone(),
            scheme: variant.appearance,
            tokens,
            shell,
            adapters,
            selection: selection.clone(),
        })
    }

    fn resource_context(&self, context: &mut Context) -> Result<(), String> {
        if let Some(root) = &self.root {
            let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
            let uri = url::Url::from_directory_path(&root)
                .map_err(|_| "invalid package resource path")?;
            context.insert("package.uri".into(), uri.to_string().into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedAdapter {
    pub base: AdapterBase,
    pub tokens: Tokens,
    pub gtk3: Option<String>,
    pub gtk4: Option<String>,
    pub palette: Option<toml::Value>,
    pub stylesheet: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTheme {
    pub theme_id: String,
    pub name: String,
    pub variant_id: String,
    pub scheme: Scheme,
    pub tokens: Tokens,
    pub shell: ShellTokens,
    pub adapters: BTreeMap<String, ResolvedAdapter>,
    pub selection: Selection,
}

fn context_for(selection: &Selection, scheme: Scheme, tokens: &Tokens) -> Result<Context, String> {
    let mut context = Context::new();
    flatten(
        "tokens",
        &toml::Value::try_from(tokens).map_err(|e| e.to_string())?,
        &mut context,
    );
    context.insert("appearance.theme".into(), selection.theme.clone().into());
    context.insert(
        "appearance.variant".into(),
        selection.variant.clone().into(),
    );
    context.insert("appearance.color_scheme".into(), scheme.as_str().into());
    context.insert(
        "appearance.accent".into(),
        tokens.colors.accent.to_string().into(),
    );
    context.insert(
        "appearance.high_contrast".into(),
        selection.accessibility.high_contrast.into(),
    );
    context.insert(
        "appearance.text_scale".into(),
        f64::from(selection.accessibility.text_scale).into(),
    );
    Ok(context)
}

pub fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err(format!("invalid theme/variant id {id}"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Store {
    pub paths: XdgPaths,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            paths: XdgPaths::default(),
        }
    }
}
impl Store {
    pub fn selection_path(&self) -> PathBuf {
        self.paths.user_config_file("coconut", "appearance.toml")
    }
    pub fn generated_dir(&self) -> PathBuf {
        self.paths.data_home.join("coconut/appearance")
    }
    pub fn packages(&self) -> Result<Vec<Package>, String> {
        let mut packages = BTreeMap::new();
        for root in self.paths.data_search_paths() {
            let Ok(entries) = fs::read_dir(root.join("coconut/themes")) else {
                continue;
            };
            for entry in entries.flatten() {
                let id = entry.file_name().to_string_lossy().into_owned();
                if packages.contains_key(&id) {
                    continue;
                }
                match Package::open(&entry.path()) {
                    Ok(package) if package.manifest.id == id => {
                        packages.insert(id, package);
                    }
                    Ok(_) => {
                        eprintln!("coconut-theme: package directory and manifest id differ: {id}")
                    }
                    Err(error) => eprintln!("coconut-theme: skipping {id}: {error}"),
                }
            }
        }
        packages
            .entry("coconut".into())
            .or_insert(Package::builtin()?);
        Ok(packages.into_values().collect())
    }
    pub fn package(&self, id: &str) -> Result<Package, String> {
        valid_id(id)?;
        for root in self.paths.data_search_paths() {
            let path = root.join("coconut/themes").join(id);
            if path.join("theme.toml").is_file() {
                let package = Package::open(&path)?;
                if package.manifest.id != id {
                    return Err("package directory and manifest id differ".into());
                }
                return Ok(package);
            }
        }
        if id == "coconut" {
            Package::builtin()
        } else {
            Err(format!("Coconut theme {id} was not found"))
        }
    }
    pub fn selection(&self) -> Result<Selection, String> {
        match fs::read_to_string(self.selection_path()) {
            Ok(source) => {
                toml::from_str(&source).map_err(|e| format!("invalid Coconut appearance: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.legacy_selection(),
            Err(e) => Err(e.to_string()),
        }
    }
    fn legacy_selection(&self) -> Result<Selection, String> {
        let mut selection = Selection::default();
        if let Ok(source) =
            fs::read_to_string(self.paths.user_config_file("cream", "appearance.toml"))
        {
            let old: toml::Value =
                toml::from_str(&source).map_err(|e| format!("invalid legacy appearance: {e}"))?;
            if let Some(id) = old
                .get("theme")
                .and_then(|v| v.as_str())
                .filter(|id| *id != "default")
            {
                // Never silently substitute a custom theme with a different one.
                let package = self.package(id).map_err(|_| {
                    format!("legacy CreamUI theme {id} needs importing as a Coconut package")
                })?;
                selection.theme = id.into();
                selection.variant = package.manifest.default_variant;
            }
            if let Some(id) = old.get("variant").and_then(|v| v.as_str()) {
                selection.variant = id.into();
            }
            if let Some(accent) = old.get("accent").and_then(|v| v.as_str()) {
                selection.overrides.accent = Some(accent.parse()?);
            }
            selection.overrides.font_family = old
                .get("font_family")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            if let Some(value) = old.get("corners") {
                selection.overrides.corners =
                    Some(value.clone().try_into().map_err(|e| e.to_string())?);
            }
        }
        if let Ok(source) =
            fs::read_to_string(self.paths.user_config_file("cream", "accessibility.toml"))
        {
            selection.accessibility = toml::from_str(&source).map_err(|e| e.to_string())?;
        }
        Ok(selection)
    }
    pub fn load(&self) -> Result<ResolvedTheme, String> {
        let selection = self.selection()?;
        self.package(&selection.theme)?.resolve(&selection)
    }
    pub fn apply(&self, selection: &Selection) -> Result<ResolvedTheme, String> {
        let resolved = self.package(&selection.theme)?.resolve(selection)?;
        atomic_write(
            &self.selection_path(),
            &toml::to_string_pretty(&resolved.selection).map_err(|e| e.to_string())?,
        )?;
        Ok(resolved)
    }
    pub fn ensure_selection(&self) -> Result<ResolvedTheme, String> {
        let selection = self.selection()?;
        if self.selection_path().exists() {
            self.package(&selection.theme)?.resolve(&selection)
        } else {
            self.apply(&selection)
        }
    }
}

/// Replace a generated/configuration file atomically, preserving permissions
/// and existing symlink targets. Unchanged content does not touch its mtime.
pub fn atomic_write(path: &Path, contents: &str) -> Result<bool, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(false);
    }
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.into());
    let path = resolved.as_path();
    fs::create_dir_all(path.parent().ok_or("file has no parent")?).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
        .map(|()| true)
        .map_err(|e: std::io::Error| e.to_string())
}
