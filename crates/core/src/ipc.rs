//! Public D-Bus contract for live CreamUI theme and Coconut shell updates.

use crate::ShellConfig;
use dbus::blocking::Connection;
use dbus::channel::Sender;
use dbus::message::{MatchRule, Message};
use std::sync::mpsc::{self, Receiver, Sender as EventSender};
use std::time::Duration;

pub const THEME_PATH: &str = "/org/creamui/Theme";
pub const THEME_INTERFACE: &str = "org.creamui.Theme";
pub const THEME_RELOAD: &str = "ReloadTheme";

pub const SHELL_PATH: &str = "/org/coconut/Shell";
pub const SHELL_INTERFACE: &str = "org.coconut.Shell";
pub const SHELL_CONFIG_CHANGED: &str = "ConfigChanged";

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// Reload the selected CreamUI theme from its persisted selection.
    ReloadTheme,
    /// Replace Coconut's in-memory configuration with this validated value.
    ShellConfig(ShellConfig),
}

/// Emits `org.creamui.Theme.ReloadTheme` on the session bus.
pub fn publish_theme_reload() -> Result<(), String> {
    publish_signal(THEME_PATH, THEME_INTERFACE, THEME_RELOAD, ())
}

/// Emits `org.coconut.Shell.ConfigChanged` with the complete TOML document.
pub fn publish_shell_config(config: &ShellConfig) -> Result<(), String> {
    let contents = toml::to_string(config).map_err(|error| error.to_string())?;
    publish_signal(
        SHELL_PATH,
        SHELL_INTERFACE,
        SHELL_CONFIG_CHANGED,
        (contents,),
    )
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

    if let Err(error) = connection.add_match(
        MatchRule::new_signal(SHELL_INTERFACE, SHELL_CONFIG_CHANGED).with_path(SHELL_PATH),
        move |(contents,): (String,), _, _| match toml::from_str::<ShellConfig>(&contents) {
            Ok(config) => sender.send(RuntimeEvent::ShellConfig(config)).is_ok(),
            Err(error) => {
                eprintln!("coconut: ignored invalid D-Bus configuration update: {error}");
                true
            }
        },
    ) {
        eprintln!("coconut: failed to listen for shell updates: {error}");
    }

    loop {
        if let Err(error) = connection.process(Duration::from_secs(1)) {
            eprintln!("coconut: D-Bus listener stopped: {error}");
            return;
        }
    }
}
