//! Resolves a `.desktop` file's `Icon=` value — usually a bare theme name
//! like `firefox`, not a path — against the system's installed icon themes,
//! the same way `app_drawer` and desktop icons both need to.

use creamui_image::{ImageData, SvgSize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves `icon` (an `Icon=` value, or already an absolute path) to a file
/// on disk, using `index` from [`build_icon_index`].
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

/// Decodes an icon file [`resolve_icon`] found, rasterizing an SVG source at
/// `size` pixels since [`ImageData::from_path`] only decodes raster formats.
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

/// Indexes every application icon under the system's icon themes and
/// pixmap directories by file stem, lowercased — built fresh on every call
/// since app/desktop entries are only (re)loaded occasionally, off the UI
/// thread.
pub fn build_icon_index() -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    for directory in xdg_data_directories() {
        collect_themed_icons(&directory.join("icons"), &mut index);
        collect_flat_icons(&directory.join("pixmaps"), &mut index);
        collect_flat_icons(&directory.join("flatpak/appstream"), &mut index);
    }
    collect_flat_icons(Path::new("/var/lib/flatpak/appstream"), &mut index);
    index
}

/// Walks an icon theme root (e.g. `.../icons`), keeping only icons under an
/// `apps` category folder — what a `.desktop` file's `Icon=` key refers to,
/// as opposed to same-named status/mimetype/action glyphs elsewhere in the
/// same theme.
fn collect_themed_icons(directory: &Path, index: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("apps") {
            collect_flat_icons(&path, index);
        } else {
            collect_themed_icons(&path, index);
        }
    }
}

fn collect_flat_icons(directory: &Path, index: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_flat_icons(&path, index);
        } else if is_icon_file(&path) {
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                index.entry(name.to_ascii_lowercase()).or_insert(path);
            }
        }
    }
}

/// `$XDG_DATA_HOME` (or its default) followed by `$XDG_DATA_DIRS` — the
/// search path both icon themes and `.desktop` application directories live
/// under.
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
    fn an_absolute_path_is_used_directly() {
        let index = HashMap::new();
        let directory = std::env::temp_dir().join(format!(
            "coconut-icon-theme-test-{}-absolute",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let icon = directory.join("custom.png");
        fs::write(&icon, b"not really a png").unwrap();

        let resolved = resolve_icon(icon.to_str().unwrap(), &index);

        fs::remove_dir_all(&directory).ok();
        assert_eq!(resolved, Some(icon));
    }

    #[test]
    fn a_bare_name_is_looked_up_in_the_index() {
        let mut index = HashMap::new();
        index.insert(
            "firefox".to_string(),
            PathBuf::from("/usr/share/icons/firefox.svg"),
        );

        assert_eq!(
            resolve_icon("firefox", &index),
            Some(PathBuf::from("/usr/share/icons/firefox.svg"))
        );
        assert_eq!(resolve_icon("does-not-exist", &index), None);
    }
}
