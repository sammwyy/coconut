use super::DesktopIntegration;
use crate::platform::OpenWindow;
use creamui_render::WindowHandle;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

/// KWin Wayland exposes its window-control interface over the user D-Bus.
/// kdotool is a small D-Bus client for that interface; using it avoids the X11
/// APIs, which cannot enumerate native Wayland windows.
pub struct KWinDbus {
    windows: Arc<Mutex<Vec<OpenWindow>>>,
}

impl KWinDbus {
    pub fn detect() -> Option<Self> {
        busctl(&["status", "org.kde.KWin"])
            .and_then(|_| kdotool(&["--help"]))
            .map(|_| Self::new())
    }

    fn new() -> Self {
        let windows = Arc::new(Mutex::new(Vec::new()));
        let cache = windows.clone();
        thread::spawn(move || loop {
            let next = query_windows();
            if let Ok(mut current) = cache.lock() {
                *current = next;
            }
            thread::sleep(Duration::from_millis(700));
        });
        Self { windows }
    }
}

impl DesktopIntegration for KWinDbus {
    fn prepare_window(&self, window: &WindowHandle) {
        window.set_always_on_top(true);
    }

    fn windows(&self) -> Vec<OpenWindow> {
        self.windows
            .lock()
            .map(|windows| windows.clone())
            .unwrap_or_default()
    }

    fn activate_window(&self, id: &str) {
        let command = if self
            .windows()
            .iter()
            .any(|window| window.id == id && window.active)
        {
            "windowminimize"
        } else {
            "windowactivate"
        };
        let id = id.to_owned();
        thread::spawn(move || {
            let _ = kdotool(&[command, &id]);
        });
    }
}

fn query_windows() -> Vec<OpenWindow> {
    let active = kdotool(&["getactivewindow"]);
    kdotool(&["search", "--name", ""])
        .map(|ids| {
            ids.lines()
                .filter_map(|id| window_from_id(id.trim(), active.as_deref()))
                .filter(|window| !window.title.starts_with("CreamShell"))
                .collect()
        })
        .unwrap_or_default()
}

fn window_from_id(id: &str, active: Option<&str>) -> Option<OpenWindow> {
    let title = kdotool(&["getwindowname", id])?;
    if title.is_empty() {
        return None;
    }
    let app_name = kdotool(&["getwindowclassname", id])
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| title.clone());
    Some(OpenWindow {
        id: id.to_owned(),
        icon_path: find_icon(&app_name),
        app_name,
        title,
        active: active == Some(id),
    })
}

/// KWin exposes an application identifier but not a ready-to-paint raster
/// icon. Resolve it once in the background worker and retain the result.
fn find_icon(app_name: &str) -> Option<PathBuf> {
    let key = app_name.to_ascii_lowercase();
    if let Ok(mut cache) = icon_cache().lock() {
        if let Some(icon) = cache.get(&key) {
            return icon.clone();
        }
        // Avoid blocking KWin's polling worker on recursive icon discovery.
        // The task initially uses its text fallback, then picks up this cache.
        cache.insert(key.clone(), None);
    }
    let lookup_key = key.clone();
    thread::spawn(move || {
        let icon = scan_icon(&lookup_key);
        if let Ok(mut cache) = icon_cache().lock() {
            cache.insert(lookup_key, icon);
        }
    });
    None
}

fn icon_cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn scan_icon(app_name: &str) -> Option<PathBuf> {
    let key = app_name.to_ascii_lowercase();
    let mut names = vec![key.clone()];
    if let Some(last) = app_name.rsplit('.').next() {
        names.push(last.to_ascii_lowercase());
    }
    names.sort();
    names.dedup();
    icon_roots()
        .into_iter()
        .filter_map(|root| find_icon_in(&root, &names))
        .min_by_key(|path| icon_score(path, &names))
}

fn icon_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".local/share/flatpak/appstream"));
    }
    roots.push(PathBuf::from("/var/lib/flatpak/appstream"));
    roots
}

fn find_icon_in(root: &Path, names: &[String]) -> Option<PathBuf> {
    let mut matches = Vec::new();
    collect_icons(root, names, &mut matches);
    matches
        .into_iter()
        .min_by_key(|path| icon_score(path, names))
}

fn collect_icons(directory: &Path, names: &[String], matches: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_icons(&path, names, matches);
            continue;
        }
        let is_png = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_ascii_lowercase);
        if is_png && stem.is_some_and(|stem| names.iter().any(|name| stem.contains(name))) {
            matches.push(path);
        }
    }
}

fn icon_score(path: &Path, names: &[String]) -> (u8, u16) {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let exact = names.contains(&stem);
    let size = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(|part| {
            part.split('x')
                .next()
                .and_then(|size| size.parse::<u16>().ok())
        })
        .unwrap_or(256);
    (u8::from(!exact), size.abs_diff(64))
}

fn busctl(args: &[&str]) -> Option<String> {
    Command::new("timeout")
        .args(["2s", "busctl", "--user"])
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn kdotool(args: &[&str]) -> Option<String> {
    Command::new("timeout")
        .args(["2s", "kdotool"])
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
