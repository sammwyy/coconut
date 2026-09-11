use super::{DesktopBackend, OpenWindow, Playback};
use creamui_render::WindowHandle;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;

pub struct PlasmaBackend {
    windows: Arc<Mutex<Vec<OpenWindow>>>,
}

impl PlasmaBackend {
    pub fn new() -> Self {
        let windows = Arc::new(Mutex::new(Vec::new()));
        if let Ok(mut current) = windows.lock() {
            *current = query_windows();
        }
        let worker_windows = windows.clone();
        runtime().spawn(async move {
            loop {
                let next = tokio::task::spawn_blocking(query_windows)
                    .await
                    .unwrap_or_default();
                if let Ok(mut current) = worker_windows.lock() {
                    *current = next;
                }
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
        });
        Self { windows }
    }
}

impl DesktopBackend for PlasmaBackend {
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
        let id = id.to_owned();
        let active = self
            .windows()
            .iter()
            .any(|window| window.id == id && window.active);
        runtime().spawn_blocking(move || {
            let command = if active {
                "windowminimize"
            } else {
                "windowactivate"
            };
            let _ = kdotool().args([command, &id]).status();
        });
    }

    fn playback(&self) -> Option<Playback> {
        playback()
    }

    fn toggle_playback(&self) {
        runtime().spawn_blocking(|| {
            let _ = playerctl().args(["play-pause"]).status();
        });
    }
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("CreamShell backend runtime"))
}

fn query_windows() -> Vec<OpenWindow> {
    let active = query(&["getactivewindow"]);
    query(&["search", "--name", ""])
        .map(|ids| {
            ids.lines()
                .filter_map(|id| window_from_id(id.trim(), active.as_deref()))
                .filter(|window| !window.title.starts_with("CreamShell"))
                .collect()
        })
        .unwrap_or_default()
}

fn playerctl() -> Command {
    let mut command = Command::new("timeout");
    command.args(["2s", "playerctl"]);
    command
}

fn playback() -> Option<Playback> {
    static CACHE: OnceLock<Arc<Mutex<Option<Playback>>>> = OnceLock::new();
    let cache = CACHE
        .get_or_init(|| {
            let cache = Arc::new(Mutex::new(None));
            let worker_cache = cache.clone();
            runtime().spawn(async move {
                loop {
                    let next = tokio::task::spawn_blocking(query_playback)
                        .await
                        .ok()
                        .flatten();
                    if let Ok(mut current) = worker_cache.lock() {
                        *current = next;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
            cache
        })
        .clone();
    cache.lock().ok().and_then(|current| current.clone())
}

fn query_playback() -> Option<Playback> {
    let output = playerctl()
        .args([
        "metadata",
        "--format",
            "{{status}}\t{{artist}}\t{{title}}\t{{mpris:artUrl}}\t{{position}}\t{{mpris:length}}\t{{playerName}}",
        ])
        .output()
        .ok()
        .filter(|result| result.status.success())?;
    let fields = String::from_utf8_lossy(&output.stdout);
    let mut fields = fields.trim_end().split('\t');
    let status = fields.next()?.to_owned();
    let artist = fields.next().unwrap_or_default().to_owned();
    let title = fields.next().unwrap_or_default().to_owned();
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    let art_url = fields.next().and_then(resolve_art);
    let position = format_time(fields.next().unwrap_or_default());
    let length = format_time(fields.next().unwrap_or_default());
    let player_name = fields.next().unwrap_or_default();
    Some(Playback {
        app_icon: (!player_name.is_empty())
            .then(|| find_icon(player_name))
            .flatten(),
        title,
        artist,
        status,
        art_url,
        position,
        length,
    })
}

fn resolve_art(value: &str) -> Option<PathBuf> {
    if let Some(path) = value.strip_prefix("file://") {
        return Some(PathBuf::from(path));
    }
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    let path = std::env::temp_dir().join(format!("creamshell-art-{:x}", hasher.finish()));
    if path.is_file() {
        return Some(path);
    }
    let output = Command::new("timeout")
        .args(["4s", "curl", "-LfsS", "-A", "CreamShell/1.0", value])
        .output()
        .ok()
        .filter(|result| result.status.success())?;
    if output.stdout.is_empty() || output.stdout.len() > 8 * 1024 * 1024 {
        return None;
    }
    std::fs::write(&path, output.stdout).ok()?;
    Some(path)
}

fn format_time(value: &str) -> String {
    value
        .parse::<u64>()
        .map(|micros| format!("{}:{:02}", micros / 60_000_000, micros / 1_000_000 % 60))
        .unwrap_or_else(|_| "0:00".into())
}

fn query(args: &[&str]) -> Option<String> {
    kdotool()
        .args(args)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn kdotool() -> Command {
    let cargo_home = std::env::var_os("CARGO_HOME").or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo").into_os_string())
    });
    let local = cargo_home.map(|home| PathBuf::from(home).join("bin/kdotool"));
    let executable = local
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("kdotool"));
    let mut command = Command::new("timeout");
    command.arg("2s").arg(executable);
    command
}

fn window_from_id(id: &str, active: Option<&str>) -> Option<OpenWindow> {
    let title = query(&["getwindowname", id])?;
    let app_name = query(&["getwindowclassname", id])
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| title.clone());
    (!title.is_empty()).then(|| OpenWindow {
        id: id.into(),
        icon_path: find_icon(&app_name),
        app_name,
        title,
        active: active.is_some_and(|active_id| active_id == id),
    })
}

fn find_icon(app_name: &str) -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = app_name.to_ascii_lowercase();
    if let Ok(entries) = cache.lock() {
        if let Some(icon) = entries.get(&key) {
            return icon.clone();
        }
    }
    let mut names = vec![app_name.to_ascii_lowercase()];
    if let Some(last) = app_name.rsplit('.').next() {
        names.push(last.to_ascii_lowercase());
    }
    names.sort();
    names.dedup();

    let icon = icon_roots()
        .into_iter()
        .filter_map(|root| find_icon_in(&root, &names))
        .min_by_key(|path| icon_score(path, &names));
    if let Ok(mut entries) = cache.lock() {
        entries.insert(key, icon.clone());
    }
    icon
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

#[cfg(test)]
mod tests {
    use super::query;

    #[test]
    fn query_returns_none_when_kdotool_is_unavailable() {
        assert!(query(&["__creamshell_test_missing_command__"]).is_none());
    }
}
