//! Native desktop integration for the Blair compositor.
//!
//! Blair owns the canonical window list and work areas. This backend talks
//! directly to its org.blair.Compositor1 D-Bus service, without an injected
//! script or compatibility tool.

use coconut_api::desktop::{
    DesktopIntegration, DesktopWorkArea, OpenWindow, WindowChangeListener, Workspace,
    WorkspaceCapabilities,
};
use creamui_render::WindowHandle;
use dbus::{blocking::SyncConnection, channel::MatchingReceiver, message::MatchRule, MessageType};
use delib::{application_icons::ApplicationIconResolver, icons::system_icon_theme};
use std::cell::RefCell;
use std::sync::{mpsc, Arc, Mutex};
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

    fn close_window(&self, id: &str) {
        let Ok(id) = id.parse::<u64>() else { return };
        thread::spawn(move || {
            let Ok(connection) = SyncConnection::new_session() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
            let _: Result<(bool,), _> = proxy.method_call(INTERFACE, "CloseWindow", (id,));
        });
    }

    fn window_changes(&self) -> Option<WindowChangeListener> {
        Some(self.changes.clone())
    }

    fn work_area(&self) -> Option<DesktopWorkArea> {
        self.cache.lock().ok().and_then(|cache| cache.work_area)
    }

    fn workspaces(&self) -> Vec<Workspace> {
        self.cache
            .lock()
            .map(|cache| cache.workspaces.clone())
            .unwrap_or_default()
    }

    fn activate_workspace(&self, id: &str) {
        let Ok(id) = id.parse::<u64>() else {
            return;
        };
        thread::spawn(move || {
            let Ok(connection) = SyncConnection::new_session() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
            let _: Result<(bool,), _> = proxy.method_call(INTERFACE, "SwitchWorkspace", (id,));
        });
    }

    fn create_workspace(&self) -> Option<String> {
        let connection = SyncConnection::new_session().ok()?;
        let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
        let (id,): (u64,) = proxy
            .method_call(INTERFACE, "CreateWorkspace", ("",))
            .ok()?;
        (id > 0).then(|| id.to_string())
    }

    fn destroy_workspace(&self, id: &str) {
        let Ok(id) = id.parse::<u64>() else {
            return;
        };
        thread::spawn(move || {
            let Ok(connection) = SyncConnection::new_session() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
            let _: Result<(bool,), _> = proxy.method_call(INTERFACE, "DestroyWorkspace", (id,));
        });
    }

    fn workspace_capabilities(&self) -> WorkspaceCapabilities {
        self.cache
            .lock()
            .map(|cache| cache.workspace_capabilities)
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct WindowCache {
    windows: Vec<OpenWindow>,
    workspaces: Vec<Workspace>,
    workspace_capabilities: WorkspaceCapabilities,
    work_area: Option<DesktopWorkArea>,
}

type DbusWindow = (u64, String, String, i32, i32, i32, i32, bool, bool, bool);
type DbusWorkspace = (u64, String, bool, String, u32);

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
    if let Ok(workspaces) = list_workspaces(connection) {
        current.workspaces = workspaces
            .into_iter()
            .map(|(id, name, active, output, window_count)| Workspace {
                id: id.to_string(),
                name,
                active,
                window_count: window_count as usize,
                output: (!output.is_empty()).then_some(output),
            })
            .collect();
    }
    if let Ok((create, destroy)) = workspace_capabilities(connection) {
        current.workspace_capabilities = WorkspaceCapabilities { create, destroy };
    }
    true
}

fn list_windows(connection: &SyncConnection) -> Result<Vec<DbusWindow>, dbus::Error> {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (windows,): (Vec<DbusWindow>,) = proxy.method_call(INTERFACE, "ListWindows", ())?;
    Ok(windows)
}

fn list_workspaces(connection: &SyncConnection) -> Result<Vec<DbusWorkspace>, dbus::Error> {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (workspaces,): (Vec<DbusWorkspace>,) =
        proxy.method_call(INTERFACE, "ListWorkspaces", ())?;
    Ok(workspaces)
}

fn workspace_capabilities(connection: &SyncConnection) -> Result<(bool, bool), dbus::Error> {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    proxy.method_call(INTERFACE, "WorkspaceCapabilities", ())
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

fn application_icon(app_id: &str) -> Option<std::path::PathBuf> {
    thread_local! {
        static ICONS: RefCell<ApplicationIconResolver> = RefCell::new(
            ApplicationIconResolver::new(system_icon_theme())
        );
    }
    ICONS.with(|icons| icons.borrow().resolve(app_id, 64, 1))
}
