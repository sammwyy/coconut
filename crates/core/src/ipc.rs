//! Public D-Bus contract for live CreamUI theme and Coconut shell updates.

use crate::ShellConfig;
use dbus::blocking::Connection;
use dbus::channel::Sender;
use dbus::message::{MatchRule, Message};
use std::sync::{
    mpsc::{self, Receiver, Sender as EventSender},
    Arc, Condvar, Mutex, OnceLock,
};
use std::time::Duration;

pub const THEME_PATH: &str = "/org/creamui/Theme";
pub const THEME_INTERFACE: &str = "org.creamui.Theme";
pub const THEME_RELOAD: &str = "ReloadTheme";

pub const SHELL_PATH: &str = "/org/coconut/Shell";
pub const SHELL_INTERFACE: &str = "org.coconut.Shell";
pub const SHELL_CONFIG_CHANGED: &str = "ConfigChanged";
pub const USER_PROFILE_CHANGED: &str = "UserProfileChanged";

/// Color pickers can produce a value for every pointer motion.  Keep those
/// updates responsive while ensuring only the final value of a short burst is
/// sent over D-Bus or rendered by the shell.
const SHELL_CONFIG_DEBOUNCE: Duration = Duration::from_millis(120);

static SHELL_CONFIG_PUBLISHER: OnceLock<DebouncedShellConfigPublisher> = OnceLock::new();

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// Reload the selected CreamUI theme from its persisted selection.
    ReloadTheme,
    /// Replace Coconut's in-memory configuration with this validated value.
    ShellConfig(ShellConfig),
    /// The user's saved location changed and location-backed widgets should refresh.
    UserProfileChanged,
}

/// Emits `org.creamui.Theme.ReloadTheme` on the session bus.
pub fn publish_theme_reload() -> Result<(), String> {
    publish_signal(THEME_PATH, THEME_INTERFACE, THEME_RELOAD, ())
}

/// Schedules `org.coconut.Shell.ConfigChanged` with the complete TOML document.
///
/// Consecutive calls made within [`SHELL_CONFIG_DEBOUNCE`] are coalesced, so a
/// high-frequency control such as a color picker emits only its final value.
pub fn publish_shell_config(config: &ShellConfig) -> Result<(), String> {
    SHELL_CONFIG_PUBLISHER
        .get_or_init(DebouncedShellConfigPublisher::new)
        .schedule(config.clone());
    Ok(())
}

/// Emits `org.coconut.Shell.UserProfileChanged` on the session bus.
pub fn publish_user_profile_changed() -> Result<(), String> {
    publish_signal(SHELL_PATH, SHELL_INTERFACE, USER_PROFILE_CHANGED, ())
}

fn publish_shell_config_now(config: &ShellConfig) -> Result<(), String> {
    let contents = toml::to_string(config).map_err(|error| error.to_string())?;
    publish_signal(
        SHELL_PATH,
        SHELL_INTERFACE,
        SHELL_CONFIG_CHANGED,
        (contents,),
    )
}

struct DebouncedShellConfigPublisher {
    pending: Arc<(Mutex<Option<ShellConfig>>, Condvar)>,
}

impl DebouncedShellConfigPublisher {
    fn new() -> Self {
        let pending = Arc::new((Mutex::new(None), Condvar::new()));
        let worker_pending = pending.clone();
        std::thread::spawn(move || publish_debounced_shell_configs(worker_pending));
        Self { pending }
    }

    fn schedule(&self, config: ShellConfig) {
        let (pending, wake) = &*self.pending;
        let mut pending = pending.lock().unwrap_or_else(|error| error.into_inner());
        *pending = Some(config);
        wake.notify_one();
    }
}

fn publish_debounced_shell_configs(pending: Arc<(Mutex<Option<ShellConfig>>, Condvar)>) {
    let (slot, wake) = &*pending;
    loop {
        let config = {
            let mut config = slot.lock().unwrap_or_else(|error| error.into_inner());
            while config.is_none() {
                config = wake.wait(config).unwrap_or_else(|error| error.into_inner());
            }

            loop {
                let (next, timeout) = wake
                    .wait_timeout(config, SHELL_CONFIG_DEBOUNCE)
                    .unwrap_or_else(|error| error.into_inner());
                config = next;
                if timeout.timed_out() {
                    break config.take().expect("pending shell config");
                }
            }
        };

        if let Err(error) = publish_shell_config_now(&config) {
            eprintln!("coconut: failed to publish shell configuration update: {error}");
        }
    }
}

fn publish_signal<A: dbus::arg::AppendAll>(
    path: &str,
    interface: &str,
    member: &str,
    arguments: A,
) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let mut message =
        Message::new_signal(path, interface, member).map_err(|error| error.to_string())?;
    message.append_all(arguments);
    connection
        .send(message)
        .map(|_| ())
        .map_err(|_| "failed to send D-Bus signal".to_owned())
}

/// Receives public runtime events on a worker-owned session-bus connection.
pub fn listen_for_runtime_events() -> Receiver<RuntimeEvent> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || listen(sender));
    receiver
}

fn listen(sender: EventSender<RuntimeEvent>) {
    let Ok(connection) = Connection::new_session() else {
        eprintln!("coconut: D-Bus session bus unavailable; live settings updates are disabled");
        return;
    };

    let theme_sender = sender.clone();
    if let Err(error) = connection.add_match(
        MatchRule::new_signal(THEME_INTERFACE, THEME_RELOAD).with_path(THEME_PATH),
        move |_: (), _, _| theme_sender.send(RuntimeEvent::ReloadTheme).is_ok(),
    ) {
        eprintln!("coconut: failed to listen for theme updates: {error}");
    }

    let (shell_config_sender, shell_config_receiver) = mpsc::channel();
    let shell_event_sender = sender.clone();
    std::thread::spawn(move || {
        forward_debounced_shell_configs(shell_config_receiver, shell_event_sender)
    });

    if let Err(error) = connection.add_match(
        MatchRule::new_signal(SHELL_INTERFACE, SHELL_CONFIG_CHANGED).with_path(SHELL_PATH),
        move |(contents,): (String,), _, _| match toml::from_str::<ShellConfig>(&contents) {
            Ok(config) => shell_config_sender.send(config).is_ok(),
            Err(error) => {
                eprintln!("coconut: ignored invalid D-Bus configuration update: {error}");
                true
            }
        },
    ) {
        eprintln!("coconut: failed to listen for shell updates: {error}");
    }

    let profile_sender = sender.clone();
    if let Err(error) = connection.add_match(
        MatchRule::new_signal(SHELL_INTERFACE, USER_PROFILE_CHANGED).with_path(SHELL_PATH),
        move |_: (), _, _| {
            profile_sender
                .send(RuntimeEvent::UserProfileChanged)
                .is_ok()
        },
    ) {
        eprintln!("coconut: failed to listen for profile updates: {error}");
    }

    loop {
        if let Err(error) = connection.process(Duration::from_secs(1)) {
            eprintln!("coconut: D-Bus listener stopped: {error}");
            return;
        }
    }
}

/// Coalesces an incoming burst before it reaches the shell's UI event queue.
fn forward_debounced_shell_configs(
    receiver: Receiver<ShellConfig>,
    sender: EventSender<RuntimeEvent>,
) {
    while let Ok(mut config) = receiver.recv() {
        loop {
            match receiver.recv_timeout(SHELL_CONFIG_DEBOUNCE) {
                Ok(next) => config = next,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = sender.send(RuntimeEvent::ShellConfig(config));
                    return;
                }
            }
        }

        if sender.send(RuntimeEvent::ShellConfig(config)).is_err() {
            return;
        }
    }
}
