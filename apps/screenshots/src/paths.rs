use std::path::{Path, PathBuf};

/// The directory screenshots are saved to by default: the platform's
/// pictures directory plus a `Screenshots` subfolder, mirroring the
/// `Wallpapers` convention used elsewhere.
pub fn screenshots_dir() -> PathBuf {
    pictures_directory()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Screenshots")
}

#[cfg(target_os = "windows")]
fn pictures_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Pictures"))
}

#[cfg(not(target_os = "windows"))]
fn pictures_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("XDG_PICTURES_DIR") {
        return Some(PathBuf::from(directory));
    }
    if let Some(directory) = pictures_directory_from_user_dirs() {
        return Some(directory);
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Pictures"))
}

#[cfg(not(target_os = "windows"))]
fn pictures_directory_from_user_dirs() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let config = delib::xdg::XdgPaths::default().config_home;
    let contents = std::fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    let value = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("XDG_PICTURES_DIR="))?;
    parse_user_dir(value, &home)
}

#[cfg(not(target_os = "windows"))]
fn parse_user_dir(value: &str, home: &Path) -> Option<PathBuf> {
    let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    let value = value.replace("\\\"", "\"").replace("\\\\", "\\");
    let path = if value == "$HOME" {
        home.to_path_buf()
    } else if let Some(tail) = value.strip_prefix("$HOME/") {
        home.join(tail)
    } else {
        PathBuf::from(value)
    };
    path.is_absolute().then_some(path)
}

/// `Screenshot from 2026-10-30 14-23-45.png`, with a suffix when the name is
/// already taken inside the same second.
pub fn timestamped_target(directory: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H-%M-%S");
    for attempt in 0..100 {
        let name = if attempt == 0 {
            format!("Screenshot from {stamp}.png")
        } else {
            format!("Screenshot from {stamp} ({attempt}).png")
        };
        let target = directory.join(name);
        if !target.exists() {
            return target;
        }
    }
    directory.join(format!("Screenshot from {stamp}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dirs_expand_home_and_reject_relative_paths() {
        let home = PathBuf::from("/home/test");
        assert_eq!(
            parse_user_dir("\"$HOME/Imagenes\"", &home),
            Some(PathBuf::from("/home/test/Imagenes"))
        );
        assert_eq!(
            parse_user_dir("\"$HOME\"", &home),
            Some(PathBuf::from("/home/test"))
        );
        assert_eq!(parse_user_dir("\"relative/Pictures\"", &home), None);
        assert_eq!(parse_user_dir("$HOME/Pictures", &home), None);
    }
}
