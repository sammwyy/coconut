//! Freedesktop icon-theme lookup shared by Coconut's desktop and plugins.
//!
//! The resolver keeps directory metadata: a flat `name -> path` index cannot
//! distinguish a 16px status icon from a 64px application icon, nor select a
//! HiDPI asset.

use creamui_image::{ImageData, SvgSize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const EXTENSIONS: [&str; 2] = ["png", "svg"];

/// Semantic fallbacks, tried in order within every theme before inheritance.
pub struct IconRequest {
    pub names: Vec<String>,
    pub size: u32,
    pub scale: u32,
}

impl IconRequest {
    pub fn named(name: impl Into<String>, size: u32) -> Self {
        Self {
            names: vec![name.into()],
            size,
            scale: 1,
        }
    }
}

#[derive(Clone)]
pub struct IconResolver {
    theme: String,
    bases: Vec<PathBuf>,
    fallback_dirs: Vec<PathBuf>,
    themes: Arc<Mutex<HashMap<String, Option<ThemeInfo>>>>,
}

impl IconResolver {
    pub fn new(theme: impl Into<String>) -> Self {
        let mut bases = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            bases.push(PathBuf::from(home).join(".icons"));
        }
        bases.extend(
            xdg_data_directories()
                .into_iter()
                .map(|path| path.join("icons")),
        );
        let mut fallback_dirs = vec![PathBuf::from("/usr/share/pixmaps")];
        fallback_dirs.extend(
            xdg_data_directories()
                .into_iter()
                .map(|path| path.join("pixmaps")),
        );
        Self {
            theme: theme.into(),
            bases,
            fallback_dirs,
            themes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Selected theme, recursive `Inherits`, `hicolor`, then unthemed pixmaps.
    pub fn resolve(&self, request: IconRequest) -> Option<PathBuf> {
        for name in &request.names {
            let path = Path::new(name);
            if path.is_absolute() && path.is_file() && is_icon_file(path) {
                return Some(path.to_path_buf());
            }
        }
        let mut visited = HashSet::new();
        if let Some(path) = self.resolve_theme(&self.theme, &request, &mut visited) {
            return Some(path);
        }
        if self.theme != "hicolor" {
            if let Some(path) = self.resolve_theme("hicolor", &request, &mut visited) {
                return Some(path);
            }
        }
        self.resolve_unthemed(&request)
    }

    /// Resolves and decodes at the requested physical pixel size.
    pub fn load(&self, request: IconRequest) -> Option<ImageData> {
        let size = request.size.saturating_mul(request.scale.max(1));
        self.resolve(request)
            .and_then(|path| load_icon(&path, size))
    }

    fn resolve_theme(
        &self,
        theme: &str,
        request: &IconRequest,
        visited: &mut HashSet<String>,
    ) -> Option<PathBuf> {
        if !visited.insert(theme.to_owned()) {
            return None;
        }
        let info = self.theme_info(theme)?;
        if let Some(path) = lookup_in_theme(theme, &self.bases, &info, request) {
            return Some(path);
        }
        for parent in info.inherits {
            if let Some(path) = self.resolve_theme(&parent, request, visited) {
                return Some(path);
            }
        }
        None
    }

    fn theme_info(&self, theme: &str) -> Option<ThemeInfo> {
        // Metadata comes from the first base directory, but files from the
        // same theme are allowed to be distributed across all bases.
        if let Some(info) = self.themes.lock().ok()?.get(theme).cloned() {
            return info;
        }
        let info = self
            .bases
            .iter()
            .map(|base| base.join(theme).join("index.theme"))
            .find_map(|path| fs::read_to_string(path).ok())
            .map(|contents| ThemeInfo::parse(&contents));
        if let Ok(mut themes) = self.themes.lock() {
            themes.insert(theme.to_owned(), info.clone());
        }
        info
    }

    fn resolve_unthemed(&self, request: &IconRequest) -> Option<PathBuf> {
        for name in &request.names {
            let stem = icon_name_stem(name);
            for directory in &self.fallback_dirs {
                for extension in EXTENSIONS {
                    let path = directory.join(format!("{stem}.{extension}"));
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone)]
struct ThemeInfo {
    inherits: Vec<String>,
    directories: Vec<ThemeDirectory>,
    hidden: bool,
}

#[derive(Clone)]
struct ThemeDirectory {
    path: String,
    size: u32,
    scale: u32,
    kind: DirectoryKind,
}

#[derive(Clone, Copy)]
enum DirectoryKind {
    Fixed,
    Scalable { min: u32, max: u32 },
    Threshold { threshold: u32 },
}

impl ThemeInfo {
    fn parse(contents: &str) -> Self {
        let sections = parse_ini(contents);
        let theme = sections.get("Icon Theme");
        let inherits = theme
            .and_then(|values| values.get("Inherits"))
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let directories = theme
            .and_then(|values| values.get("Directories"))
            .into_iter()
            .chain(theme.and_then(|values| values.get("ScaledDirectories")))
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .filter_map(|path| {
                let values = sections.get(path)?;
                let size = values.get("Size")?.parse().ok()?;
                let scale = values
                    .get("Scale")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1);
                let kind = match values.get("Type").map(String::as_str) {
                    Some("Fixed") => DirectoryKind::Fixed,
                    Some("Scalable") => DirectoryKind::Scalable {
                        min: values
                            .get("MinSize")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(size),
                        max: values
                            .get("MaxSize")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(size),
                    },
                    _ => DirectoryKind::Threshold {
                        threshold: values
                            .get("Threshold")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(2),
                    },
                };
                Some(ThemeDirectory {
                    path: path.to_owned(),
                    size,
                    scale,
                    kind,
                })
            })
            .collect();
        let hidden = theme
            .and_then(|values| values.get("Hidden"))
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
        Self {
            inherits,
            directories,
            hidden,
        }
    }
}

fn parse_ini(contents: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections = HashMap::<String, HashMap<String, String>>::new();
    let mut current = None::<String>;
    for line in contents.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            current = Some(name.to_owned());
        } else if let (Some(section), Some((key, value))) = (current.as_ref(), line.split_once('='))
        {
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    sections
}

fn lookup_in_theme(
    theme: &str,
    bases: &[PathBuf],
    info: &ThemeInfo,
    request: &IconRequest,
) -> Option<PathBuf> {
    // A semantic candidate (for example `folder-documents`) wins over its
    // generic fallback (`folder`) in this same theme. Within that candidate,
    // use an exact size/scale before the closest representation.
    for name in &request.names {
        let stem = icon_name_stem(name);
        if let Some(path) = lookup_one_in_theme(theme, bases, info, stem, request) {
            return Some(path);
        }
    }
    None
}

fn lookup_one_in_theme(
    theme: &str,
    bases: &[PathBuf],
    info: &ThemeInfo,
    stem: &str,
    request: &IconRequest,
) -> Option<PathBuf> {
    for exact in [true, false] {
        let mut best = None::<(u32, PathBuf)>;
        for directory in &info.directories {
            let distance = directory.distance(request.size, request.scale.max(1));
            if exact && distance != 0 {
                continue;
            }
            for base in bases {
                for extension in EXTENSIONS {
                    let path = base
                        .join(theme)
                        .join(&directory.path)
                        .join(format!("{stem}.{extension}"));
                    if path.is_file()
                        && best
                            .as_ref()
                            .is_none_or(|(best_distance, _)| distance < *best_distance)
                    {
                        best = Some((distance, path));
                    }
                }
            }
        }
        if let Some((_, path)) = best {
            return Some(path);
        }
    }
    None
}

impl ThemeDirectory {
    fn distance(&self, size: u32, scale: u32) -> u32 {
        let requested = size.saturating_mul(scale);
        let nominal = self.size.saturating_mul(self.scale);
        match self.kind {
            DirectoryKind::Fixed => nominal.abs_diff(requested),
            DirectoryKind::Scalable { min, max } => {
                let min = min.saturating_mul(self.scale);
                let max = max.saturating_mul(self.scale);
                if requested < min {
                    min - requested
                } else {
                    requested.saturating_sub(max)
                }
            }
            DirectoryKind::Threshold { threshold } => {
                let threshold = threshold.saturating_mul(self.scale);
                if nominal.abs_diff(requested) <= threshold {
                    0
                } else {
                    nominal.abs_diff(requested)
                }
            }
        }
    }
}

pub fn load_icon(path: &Path, size: u32) -> Option<ImageData> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return ImageData::from_svg(&fs::read(path).ok()?, SvgSize::Max(size)).ok();
    }
    ImageData::from_path(path).ok()
}

pub fn xdg_data_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(directory) = data_home {
        directories.push(directory);
    }
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    directories.extend(std::env::split_paths(&data_dirs));
    directories
}

/// Installed, user-selectable freedesktop themes, in search-precedence order.
/// Directories without an `index.theme` and fallback-only themes (`Hidden=true`)
/// are deliberately excluded from the Settings picker.
pub fn installed_icon_themes() -> Vec<String> {
    let mut bases = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(PathBuf::from(home).join(".icons"));
    }
    bases.extend(
        xdg_data_directories()
            .into_iter()
            .map(|path| path.join("icons")),
    );
    let mut seen = HashSet::new();
    let mut themes = Vec::new();
    for base in bases {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            let Ok(index) = fs::read_to_string(path.join("index.theme")) else {
                continue;
            };
            if seen.insert(name.clone()) && !ThemeInfo::parse(&index).hidden {
                themes.push(name);
            }
        }
    }
    themes
}

fn is_icon_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "svg"
            )
        })
}

/// Desktop-entry icon names are not filenames. In particular, reverse-DNS
/// application IDs such as `com.mitchellh.ghostty` are valid icon names and
/// must retain every dot. Strip an extension only when it is one we support.
fn icon_name_stem(name: &str) -> &str {
    let path = Path::new(name);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension)
            if matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "svg"
            ) =>
        {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(name)
        }
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scaled_directories_and_inheritance() {
        let info = ThemeInfo::parse("[Icon Theme]\nInherits=parent,hicolor\nDirectories=48x48/apps\nScaledDirectories=48x48@2/apps\n[48x48/apps]\nSize=48\nType=Fixed\n[48x48@2/apps]\nSize=48\nScale=2\nType=Fixed\n");
        assert_eq!(info.inherits, ["parent", "hicolor"]);
        assert_eq!(info.directories.len(), 2);
        assert_eq!(info.directories[1].distance(48, 2), 0);
    }

    #[test]
    fn closest_fixed_size_wins() {
        let directory = ThemeDirectory {
            path: "64x64/apps".into(),
            size: 64,
            scale: 1,
            kind: DirectoryKind::Fixed,
        };
        assert_eq!(directory.distance(48, 1), 16);
        assert_eq!(directory.distance(64, 1), 0);
    }

    #[test]
    fn dotted_application_ids_are_not_treated_as_filenames() {
        assert_eq!(
            icon_name_stem("com.mitchellh.ghostty"),
            "com.mitchellh.ghostty"
        );
        assert_eq!(icon_name_stem("folder.svg"), "folder");
    }
}
