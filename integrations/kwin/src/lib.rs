use creamshell_api::desktop::{DesktopIntegration, OpenWindow, WindowChangeListener};
use creamui_render::WindowHandle;
use dbus::{
    blocking::{Connection as DbusConnection, SyncConnection},
    channel::MatchingReceiver,
    message::MatchRule,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    mpsc::{self, Receiver, SyncSender, TryRecvError},
    Arc, Mutex, OnceLock,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// KWin Wayland exposes its window-control interface over the user D-Bus.
/// kdotool is a small D-Bus client for that interface; using it avoids the X11
/// APIs, which cannot enumerate native Wayland windows.
pub struct KWinDbus {
    windows: Arc<Mutex<WindowCache>>,
    changes: WindowChangeListener,
    shutdown: Option<mpsc::Sender<()>>,
    watcher: Option<JoinHandle<()>>,
}

impl KWinDbus {
    pub fn detect() -> Option<Self> {
        busctl(&["status", "org.kde.KWin"])
            .and_then(|_| kdotool(&["--help"]))
            .map(|_| Self::new())
    }

    fn new() -> Self {
        let windows = Arc::new(Mutex::new(WindowCache::default()));
        let cache = windows.clone();
        let (change_tx, change_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let watcher = thread::spawn(move || {
            if let Err(error) = run_window_event_bridge(cache, change_tx, shutdown_rx) {
                eprintln!("kwin window events: {error}");
            }
        });
        Self {
            windows,
            changes: WindowChangeListener::new(change_rx),
            shutdown: Some(shutdown_tx),
            watcher: Some(watcher),
        }
    }
}

impl Drop for KWinDbus {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}

impl DesktopIntegration for KWinDbus {
    fn prepare_window(&self, window: &WindowHandle) {
        window.set_always_on_top(true);
    }

    fn windows(&self) -> Vec<OpenWindow> {
        self.windows
            .lock()
            .map(|cache| cache.windows.clone())
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

    fn window_changes(&self) -> Option<WindowChangeListener> {
        Some(self.changes.clone())
    }
}

#[derive(Default)]
struct WindowCache {
    revision: u64,
    windows: Vec<OpenWindow>,
}

#[derive(Deserialize)]
struct EventSnapshot {
    revision: u64,
    windows: Vec<EventWindow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventWindow {
    id: String,
    title: String,
    app_name: String,
    #[serde(default)]
    desktop_file: String,
    pid: Option<u32>,
    active: bool,
}

fn run_window_event_bridge(
    cache: Arc<Mutex<WindowCache>>,
    changes: SyncSender<()>,
    shutdown: Receiver<()>,
) -> Result<(), String> {
    let event_connection = SyncConnection::new_session().map_err(|error| error.to_string())?;
    let destination = event_connection.unique_name().to_string();
    let callback_cache = cache.clone();
    let callback_changes = changes.clone();
    let _receiver = event_connection.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |message, connection| {
            if message.member().is_some_and(|member| member == "snapshot") {
                if let Some(payload) = message.get1::<String>() {
                    if apply_event_snapshot(&callback_cache, &callback_changes, &payload) {
                        let _ = callback_changes.try_send(());
                    }
                }
            }
            let _ = connection.channel().send(message.method_return());
            true
        }),
    );

    let kwin_connection = DbusConnection::new_session().map_err(|error| error.to_string())?;
    let scripting =
        kwin_connection.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(2));
    let _: Result<(bool,), _> = scripting.method_call(
        "org.kde.kwin.Scripting",
        "unloadScript",
        (KWIN_EVENT_SCRIPT_NAME,),
    );

    let source = KWIN_EVENT_SCRIPT.replace("__DESTINATION__", &destination);
    let script_file = RuntimeScriptFile::create(&source)?;
    let (script_id,): (i32,) = scripting
        .method_call(
            "org.kde.kwin.Scripting",
            "loadScript",
            (
                script_file.path.to_string_lossy().into_owned(),
                KWIN_EVENT_SCRIPT_NAME,
            ),
        )
        .map_err(|error| error.to_string())?;
    if script_id < 0 {
        return Err("KWin rejected the runtime window-event hook".to_owned());
    }

    let script = kwin_connection.with_proxy(
        "org.kde.KWin",
        format!("/Scripting/Script{script_id}"),
        Duration::from_secs(2),
    );
    let run_result: Result<(), dbus::Error> = script.method_call("org.kde.kwin.Script", "run", ());
    drop(script_file);
    if let Err(error) = run_result {
        let _: Result<(bool,), _> = scripting.method_call(
            "org.kde.kwin.Scripting",
            "unloadScript",
            (KWIN_EVENT_SCRIPT_NAME,),
        );
        return Err(error.to_string());
    }

    let event_result = loop {
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break Ok(()),
            Err(TryRecvError::Empty) => {}
        }
        if let Err(error) = event_connection.process(Duration::from_millis(100)) {
            break Err(error.to_string());
        }
    };

    let _: Result<(), _> = script.method_call("org.kde.kwin.Script", "stop", ());
    let _: Result<(bool,), _> = scripting.method_call(
        "org.kde.kwin.Scripting",
        "unloadScript",
        (KWIN_EVENT_SCRIPT_NAME,),
    );
    event_result
}

fn apply_event_snapshot(
    cache: &Arc<Mutex<WindowCache>>,
    changes: &SyncSender<()>,
    payload: &str,
) -> bool {
    let Ok(snapshot) = serde_json::from_str::<EventSnapshot>(payload) else {
        return false;
    };
    let Ok(mut current) = cache.lock() else {
        return false;
    };
    if snapshot.revision <= current.revision {
        return false;
    }
    current.revision = snapshot.revision;
    current.windows = snapshot
        .windows
        .into_iter()
        .filter(|window| !window.title.is_empty() && !window.title.starts_with("CreamShell"))
        .map(|window| OpenWindow {
            icon_path: find_icon(
                &window.id,
                &window.desktop_file,
                &window.app_name,
                window.pid,
                cache.clone(),
                changes.clone(),
            ),
            id: window.id,
            app_name: window.app_name,
            title: window.title,
            active: window.active,
        })
        .collect();
    true
}

struct RuntimeScriptFile {
    path: PathBuf,
}

impl RuntimeScriptFile {
    fn create(contents: &str) -> Result<Self, String> {
        let directory = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = directory.join(format!("creamshell-kwin-events-{}.js", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| error.to_string())?;
        file.write_all(contents.as_bytes())
            .map_err(|error| error.to_string())?;
        Ok(Self { path })
    }
}

impl Drop for RuntimeScriptFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

const KWIN_EVENT_SCRIPT_NAME: &str = "creamshell-window-events";
const KWIN_EVENT_SCRIPT: &str = r#"
const destination = "__DESTINATION__";
let revision = 0;

function sendSnapshot() {
    revision += 1;
    const windows = workspace.windowList()
        .filter(window => !window.specialWindow && !window.skipTaskbar)
        .map(window => ({
            id: window.internalId.toString(),
            title: String(window.caption || ""),
            appName: String(window.resourceClass || window.desktopFileName || window.resourceName || ""),
            desktopFile: String(window.desktopFileName || ""),
            pid: Number(window.pid) || null,
            active: Boolean(window.active)
        }));
    callDBus(destination, "/", "", "snapshot", JSON.stringify({ revision, windows }));
}

function watchWindow(window) {
    ["captionChanged", "resourceClassChanged", "skipTaskbarChanged"].forEach(name => {
        if (window[name]) {
            window[name].connect(sendSnapshot);
        }
    });
}

workspace.windowList().forEach(watchWindow);
workspace.windowAdded.connect(window => {
    watchWindow(window);
    sendSnapshot();
});
workspace.windowRemoved.connect(sendSnapshot);
workspace.windowActivated.connect(sendSnapshot);
sendSnapshot();
"#;

/// KWin exposes an application identifier but not a ready-to-paint raster
/// icon. Resolve it once in the background worker and retain the result.
fn find_icon(
    window_id: &str,
    desktop_file: &str,
    app_name: &str,
    pid: Option<u32>,
    windows: Arc<Mutex<WindowCache>>,
    changes: SyncSender<()>,
) -> Option<PathBuf> {
    let key = format!(
        "{}:{}",
        desktop_file.to_ascii_lowercase(),
        app_name.to_ascii_lowercase()
    );
    if let Ok(mut cache) = icon_cache().lock() {
        if let Some(icon) = cache.get(&key) {
            return icon.clone();
        }
        cache.insert(key.clone(), None);
    }
    let lookup_window_id = window_id.to_owned();
    let lookup_desktop_file = desktop_file.to_owned();
    let lookup_app_name = app_name.to_owned();
    thread::spawn(move || {
        let icon = compositor_declared_icon(&lookup_desktop_file)
            .or_else(|| kde_icon(&lookup_desktop_file))
            .or_else(|| pid.and_then(declared_process_icon))
            .or_else(|| kde_icon(&lookup_app_name));
        if let Ok(mut cache) = icon_cache().lock() {
            cache.insert(key, icon.clone());
        }
        if let Some(icon) = icon {
            if let Ok(mut cache) = windows.lock() {
                if let Some(window) = cache
                    .windows
                    .iter_mut()
                    .find(|window| window.id == lookup_window_id)
                {
                    window.icon_path = Some(icon);
                    let _ = changes.try_send(());
                }
            }
        }
    });
    None
}

fn compositor_declared_icon(desktop_file: &str) -> Option<PathBuf> {
    if desktop_file.is_empty() {
        return None;
    }
    let supplied = PathBuf::from(desktop_file);
    let path = if supplied.is_absolute() {
        supplied
    } else {
        let file_name = if desktop_file.ends_with(".desktop") {
            desktop_file.to_owned()
        } else {
            format!("{desktop_file}.desktop")
        };
        desktop_entry_dirs()
            .into_iter()
            .map(|directory| directory.join(&file_name))
            .find(|path| path.is_file())?
    };
    parse_desktop_entry(&path).and_then(|entry| resolve_declared_icon(&entry.icon))
}

fn kde_icon(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    ["kiconfinder6", "kiconfinder5"]
        .into_iter()
        .find_map(|program| {
            Command::new(program)
                .arg(name.trim_end_matches(".desktop"))
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| {
                    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
                    supported_image(&path).then_some(path)
                })
        })
}

#[derive(Clone)]
struct DesktopEntry {
    program: String,
    icon: String,
}

fn declared_process_icon(pid: u32) -> Option<PathBuf> {
    let process = process_program(pid)?;
    desktop_entries()
        .iter()
        .find(|entry| program_matches(&entry.program, &process))
        .and_then(|entry| resolve_declared_icon(&entry.icon))
}

fn process_program(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

fn desktop_entries() -> &'static Vec<DesktopEntry> {
    static ENTRIES: OnceLock<Vec<DesktopEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        desktop_entry_dirs()
            .into_iter()
            .filter_map(|directory| fs::read_dir(directory).ok())
            .flat_map(|entries| entries.flatten())
            .filter_map(|entry| parse_desktop_entry(&entry.path()))
            .collect()
    })
}

fn desktop_entry_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        directories.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    directories
        .extend(std::env::split_paths(&data_dirs).map(|directory| directory.join("applications")));
    if let Some(home) = std::env::var_os("HOME") {
        directories
            .push(PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"));
    }
    directories.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    directories
}

fn parse_desktop_entry(path: &Path) -> Option<DesktopEntry> {
    let contents = fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut program = None;
    let mut icon = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if in_desktop_entry {
                break;
            }
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Exec=") {
            program = desktop_exec_program(value);
        } else if let Some(value) = line.strip_prefix("Icon=") {
            icon = Some(value.to_owned());
        }
    }
    Some(DesktopEntry {
        program: program?,
        icon: icon?,
    })
}

fn desktop_exec_program(exec: &str) -> Option<String> {
    let mut program = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for character in exec.chars() {
        if escaped {
            program.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if character.is_whitespace() && !quoted {
            break;
        } else {
            program.push(character);
        }
    }
    (!program.is_empty()).then_some(program)
}

fn program_matches(declared: &str, process: &Path) -> bool {
    let declared = Path::new(declared);
    if matches!(
        declared.file_name().and_then(|name| name.to_str()),
        Some("env" | "flatpak" | "sh" | "bash" | "gtk-launch" | "snap")
    ) {
        return false;
    }
    if declared.is_absolute() {
        return declared == process;
    }
    declared.file_name() == process.file_name()
}

fn resolve_declared_icon(icon: &str) -> Option<PathBuf> {
    let path = PathBuf::from(icon);
    if path.is_absolute() {
        return supported_image(&path).then_some(path);
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(icon);
    kde_icon(name)
}

fn supported_image(path: &Path) -> bool {
    path.is_file()
        && path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("png")
                || extension.eq_ignore_ascii_case("jpg")
                || extension.eq_ignore_ascii_case("jpeg")
                || extension.eq_ignore_ascii_case("webp")
                || extension.eq_ignore_ascii_case("svg")
        })
}

fn icon_cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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

#[cfg(test)]
mod tests {
    use super::{desktop_exec_program, program_matches, DesktopIntegration, KWinDbus};
    use std::path::Path;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn desktop_exec_program_handles_quotes_and_arguments() {
        assert_eq!(
            desktop_exec_program("\"/opt/Example App/example\" --new-window %U"),
            Some("/opt/Example App/example".to_owned())
        );
    }

    #[test]
    fn desktop_exec_program_stops_before_arguments() {
        assert_eq!(
            desktop_exec_program("firefox %U"),
            Some("firefox".to_owned())
        );
    }

    #[test]
    fn program_matching_rejects_shared_launchers() {
        assert!(program_matches("firefox", Path::new("/usr/bin/firefox")));
        assert!(!program_matches("flatpak", Path::new("/usr/bin/flatpak")));
    }

    #[test]
    #[ignore = "requires a running KDE Plasma 6 session"]
    fn runtime_hook_delivers_an_initial_window_snapshot() {
        let integration = KWinDbus::new();
        let listener = integration.window_changes().expect("event listener");
        let (result_tx, result_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = result_tx.send(listener.wait());
        });

        assert_eq!(result_rx.recv_timeout(Duration::from_secs(5)), Ok(true));
        drop(integration);
    }
}
