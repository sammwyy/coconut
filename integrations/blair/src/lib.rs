//! Native desktop integration for the Blair compositor.
//!
//! Blair owns the canonical window list and work areas. This backend talks
//! directly to its org.blair.Compositor1 D-Bus service, without an injected
//! script or compatibility tool.

use coconut_api::desktop::{DesktopIntegration, DesktopWorkArea, OpenWindow, WindowChangeListener};
use creamui_render::WindowHandle;
use dbus::{blocking::SyncConnection, channel::MatchingReceiver, message::MatchRule, MessageType};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const SERVICE: &str = "org.blair.Compositor";
const PATH: &str = "/org/blair/Compositor";
const INTERFACE: &str = "org.blair.Compositor1";

/// Uses Blair's native D-Bus API for window activation, state changes,
/// work-area reservations, and live compositor events.
pub struct BlairDbus {
    cache: Arc<Mutex<WindowCache>>,
    changes: WindowChangeListener,
    shutdown: Option<mpsc::Sender<()>>,
    watcher: Option<thread::JoinHandle<()>>,
}

impl BlairDbus {
    /// Detects a running Blair service and verifies its window API. This does
    /// not rely on XDG_CURRENT_DESKTOP, so nested sessions work too.
    pub fn detect() -> Option<Self> {
        let connection = SyncConnection::new_session().ok()?;
        list_windows(&connection).ok()?;
        Some(Self::new())
    }

    fn new() -> Self {
        let cache = Arc::new(Mutex::new(WindowCache::default()));
        let (change_tx, change_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let watcher_cache = cache.clone();
        let watcher = thread::spawn(move || {
            if let Err(error) = run_event_bridge(watcher_cache, change_tx, shutdown_rx) {
                eprintln!("blair window events: {error}");
            }
        });
        Self {
            cache,
            changes: WindowChangeListener::new(change_rx),
            shutdown: Some(shutdown_tx),
            watcher: Some(watcher),
        }
    }
}

impl Drop for BlairDbus {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}

impl DesktopIntegration for BlairDbus {
    fn prepare_window(&self, window: &WindowHandle) {
        // CreamUI's panel roles map to wlr-layer-shell and Blair honors their
        // exclusive zones. This keeps normal popups above toplevel clients.
        window.set_always_on_top(true);
    }

    fn windows(&self) -> Vec<OpenWindow> {
        self.cache
            .lock()
            .map(|cache| cache.windows.clone())
            .unwrap_or_default()
    }

    fn activate_window(&self, id: &str) {
        let Ok(id) = id.parse::<u64>() else {
            return;
        };
        let minimize = self.cache.lock().ok().is_some_and(|cache| {
            cache
                .windows
                .iter()
                .any(|window| window.id == id.to_string() && window.active)
        });
        thread::spawn(move || {
            let Ok(connection) = SyncConnection::new_session() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
            if minimize {
                let _: Result<(bool,), _> = proxy.method_call(INTERFACE, "MinimizeWindow", (id,));
            } else {
                let _: Result<(bool,), _> = proxy.method_call(INTERFACE, "FocusWindow", (id,));
            }
        });
    }

    fn window_changes(&self) -> Option<WindowChangeListener> {
        Some(self.changes.clone())
    }

    fn work_area(&self) -> Option<DesktopWorkArea> {
        self.cache.lock().ok().and_then(|cache| cache.work_area)
    }
}

#[derive(Default)]
struct WindowCache {
    windows: Vec<OpenWindow>,
    work_area: Option<DesktopWorkArea>,
}

type DbusWindow = (u64, String, String, i32, i32, i32, i32, bool, bool, bool);

fn run_event_bridge(
    cache: Arc<Mutex<WindowCache>>,
    changes: mpsc::SyncSender<()>,
    shutdown: mpsc::Receiver<()>,
) -> Result<(), String> {
    let connection = SyncConnection::new_session().map_err(|error| error.to_string())?;
    let rule = MatchRule::new()
        .with_type(MessageType::Signal)
        .with_interface(INTERFACE)
        .with_path(PATH);
    connection
        .add_match_no_cb(&rule.match_str())
        .map_err(|error| error.to_string())?;
    let signal_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_signal_seen = signal_seen.clone();
    let _receiver = connection.start_receive(
        rule.static_clone(),
        Box::new(move |_message, _connection| {
            callback_signal_seen.store(true, std::sync::atomic::Ordering::Release);
            true
        }),
    );

    refresh_snapshot(&connection, &cache);
    let _ = changes.try_send(());
    loop {
        match shutdown.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        connection
            .process(Duration::from_millis(100))
            .map_err(|error| error.to_string())?;
        if signal_seen.swap(false, std::sync::atomic::Ordering::AcqRel)
            && refresh_snapshot(&connection, &cache)
        {
            let _ = changes.try_send(());
        }
    }
}

fn refresh_snapshot(connection: &SyncConnection, cache: &Arc<Mutex<WindowCache>>) -> bool {
    let Ok(windows) = list_windows(connection) else {
        return false;
    };
    let work_area = query_work_area(connection);
    let windows = windows
        .into_iter()
        .filter(|(_, title, app_id, _, _, _, _, _, _, _)| !title.is_empty() && app_id != "coconut")
        .map(
            |(id, title, app_id, _, _, _, _, focused, minimized, _)| OpenWindow {
                id: id.to_string(),
                app_name: app_id.clone(),
                title,
                icon_path: application_icon(&app_id),
                active: focused && !minimized,
            },
        )
        .collect();
    let Ok(mut current) = cache.lock() else {
        return false;
    };
    current.windows = windows;
    current.work_area = work_area;
    true
}

fn list_windows(connection: &SyncConnection) -> Result<Vec<DbusWindow>, dbus::Error> {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (windows,): (Vec<DbusWindow>,) = proxy.method_call(INTERFACE, "ListWindows", ())?;
    Ok(windows)
}

fn query_work_area(connection: &SyncConnection) -> Option<DesktopWorkArea> {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (x, y, width, height): (i32, i32, i32, i32) =
        proxy.method_call(INTERFACE, "WorkArea", ("",)).ok()?;
    (width > 0 && height > 0).then_some(DesktopWorkArea {
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    })
}

fn application_icon(app_id: &str) -> Option<PathBuf> {
    static ICONS: OnceLock<HashMap<String, PathBuf>> = OnceLock::new();
    let icon_name = desktop_icon_name(app_id)?;
    let icons = ICONS.get_or_init(build_icon_index);
    resolve_icon(&icon_name, icons)
}

fn desktop_icon_name(app_id: &str) -> Option<String> {
    let file_name = if app_id.ends_with(".desktop") {
        app_id.to_owned()
    } else {
        format!("{app_id}.desktop")
    };
    xdg_data_directories()
        .into_iter()
        .map(|directory| directory.join("applications").join(&file_name))
        .find(|path| path.is_file())
        .and_then(|path| desktop_entry_icon(&path))
}

fn desktop_entry_icon(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry {
            if let Some(value) = line.strip_prefix("Icon=") {
                return (!value.is_empty()).then_some(value.to_owned());
            }
        }
    }
    None
}

fn resolve_icon(icon: &str, index: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    let path = PathBuf::from(icon);
    if is_icon(&path) {
        return Some(path);
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(icon);
    index.get(&name.to_ascii_lowercase()).cloned()
}

fn build_icon_index() -> HashMap<String, PathBuf> {
    let mut icons = HashMap::new();
    for directory in xdg_data_directories() {
        collect_icons(&directory.join("icons"), &mut icons);
        collect_icons(&directory.join("pixmaps"), &mut icons);
        collect_icons(&directory.join("flatpak/appstream"), &mut icons);
    }
    collect_icons(Path::new("/var/lib/flatpak/appstream"), &mut icons);
    icons
}

fn collect_icons(directory: &Path, icons: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_icons(&path, icons);
        } else if is_icon(&path) {
            if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                icons.entry(name.to_ascii_lowercase()).or_insert(path);
            }
        }
    }
}

fn xdg_data_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        directories.push(PathBuf::from(home));
    } else if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share"));
    }
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    directories.extend(std::env::split_paths(&data_dirs));
    directories
}

fn is_icon(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
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
    use super::desktop_entry_icon;
    use std::fs;

    #[test]
    fn reads_icon_only_from_the_desktop_entry_group() {
        let path =
            std::env::temp_dir().join(format!("coconut-blair-icon-{}.desktop", std::process::id()));
        fs::write(
            &path,
            "Icon=wrong\n[Desktop Entry]\nName=Example\nIcon=example-icon\n",
        )
        .unwrap();
        assert_eq!(desktop_entry_icon(&path), Some("example-icon".to_owned()));
        fs::remove_file(path).unwrap();
    }
}
