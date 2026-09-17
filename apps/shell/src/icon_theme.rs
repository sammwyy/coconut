use creamui_image::{ImageData, SvgSize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn resolve_icon(icon: &str, index: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    let path = PathBuf::from(icon);
    if path.is_file() && is_icon_file(&path) {
        return Some(path);
    }
    let needle = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(icon)
        .to_ascii_lowercase();
    index.get(&needle).cloned()
}

pub fn load_icon(path: &Path, size: u32) -> Option<ImageData> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        let bytes = fs::read(path).ok()?;
        return ImageData::from_svg(&bytes, SvgSize::Max(size)).ok();
    }
    ImageData::from_path(path).ok()
}

pub fn build_icon_index(theme: &str) -> HashMap<String, PathBuf> {
    build_icon_index_from_paths(theme, &xdg_data_directories())
}

fn build_icon_index_from_paths(theme: &str, data_dirs: &[PathBuf]) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    let mut visited = HashSet::new();
    collect_theme(theme, data_dirs, &mut visited, &mut index);
    if theme != "hicolor" {
        collect_theme("hicolor", data_dirs, &mut visited, &mut index);
    }
    for directory in data_dirs {
        collect_flat_icons(&directory.join("pixmaps"), &mut index);
        collect_flat_icons(&directory.join("flatpak/appstream"), &mut index);
    }
    collect_flat_icons(Path::new("/var/lib/flatpak/appstream"), &mut index);
    index
}

fn collect_theme(
    theme: &str,
    data_dirs: &[PathBuf],
    visited: &mut HashSet<String>,
    index: &mut HashMap<String, PathBuf>,
) {
    if !visited.insert(theme.to_owned()) {
        return;
    }
    let Some(directory) = data_dirs
        .iter()
        .map(|root| root.join("icons").join(theme))
        .find(|directory| directory.is_dir())
    else {
        return;
    };
    collect_application_icons(&directory, index);
    for inherited in inherited_themes(&directory) {
        collect_theme(&inherited, data_dirs, visited, index);
    }
}

fn inherited_themes(directory: &Path) -> Vec<String> {
    let Ok(index) = fs::read_to_string(directory.join("index.theme")) else {
        return Vec::new();
    };
    index
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("Inherits="))
        .flat_map(|themes| themes.split(','))
        .map(str::trim)
        .filter(|theme| !theme.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn collect_application_icons(directory: &Path, index: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("apps") {
                collect_flat_icons(&path, index);
            } else {
                collect_application_icons(&path, index);
            }
        }
    }
}

fn collect_flat_icons(directory: &Path, index: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_flat_icons(&path, index);
        } else if is_icon_file(&path) {
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                index.entry(name.to_ascii_lowercase()).or_insert(path);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_theme_inherits_before_hicolor() {
        let directory = std::env::temp_dir().join(format!(
            "coconut-icon-theme-test-{}-inheritance",
            std::process::id()
        ));
        let icons = directory.join("icons");
        fs::create_dir_all(icons.join("selected/48x48/apps")).unwrap();
        fs::create_dir_all(icons.join("parent/48x48/apps")).unwrap();
        fs::create_dir_all(icons.join("hicolor/48x48/apps")).unwrap();
        fs::write(icons.join("selected/index.theme"), "Inherits=parent\n").unwrap();
        fs::write(icons.join("parent/48x48/apps/shared.png"), b"").unwrap();
        fs::write(icons.join("hicolor/48x48/apps/fallback.png"), b"").unwrap();
        let index = build_icon_index_from_paths("selected", &[directory.clone()]);
        fs::remove_dir_all(&directory).ok();
        assert!(index.contains_key("shared"));
        assert!(index.contains_key("fallback"));
    }
}
