//! Coordinate Coconut theme adapters and publish standard desktop preferences.
use coconut_integration_gtk as gtk;
mod portal;

use coconut_theme::{ResolvedTheme, Store};
use dbus::{
    blocking::{stdintf::org_freedesktop_dbus::RequestNameReply, Connection},
    channel::{MatchingReceiver, Sender},
    message::MatchRule,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

pub const SERVICE: &str = "org.freedesktop.impl.portal.desktop.coconut";
pub const PATH: &str = "/org/freedesktop/portal/desktop";
pub const INTERFACE: &str = "org.freedesktop.impl.portal.Settings";

/// Validate every adapter before committing a selection from Settings. Native
/// CreamUI output is available before the UI emits its normal reload signal;
/// the session worker activates GTK and Qt away from the UI thread.
pub fn apply_selection(next: &creamui_theme::AppearanceSelection) -> Result<(), String> {
    let store = Store::default();
    coconut_integration_cream::import_legacy(&store)?;
    let selection = coconut_integration_cream::selection_for(&store, next)?;
    let resolved = store.package(&selection.theme)?.resolve(&selection)?;
    coconut_integration_qt::generate(&resolved)?;
    let resolved = store.apply(&selection)?;
    coconut_integration_cream::publish_native(&store, &resolved)?;
    Ok(())
}

/// Start inside the shell, including when it was launched without a session
/// script. A D-Bus name prevents a second bridge after portal activation.
pub fn start() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(|| {
            if let Err(error) = run() {
                eprintln!("coconut-appearance: {error}");
            }
        });
    });
}

/// Generate every framework representation before notifying applications.
pub fn sync_once() -> Result<(), String> {
    let store = Store::default();
    coconut_integration_cream::import_legacy(&store)?;
    let appearance = store.ensure_selection()?;
    // Validate native outputs before changing any active adapter.
    coconut_integration_qt::generate(&appearance)?;
    let mut bridge = gtk::Bridge::new(store.paths.clone());
    publish_outputs(&store, &mut bridge, &appearance)?;
    Ok(())
}

fn publish_outputs(
    store: &Store,
    bridge: &mut gtk::Bridge,
    appearance: &ResolvedTheme,
) -> Result<String, String> {
    coconut_integration_qt::generate(appearance)?;
    let theme_name = bridge.sync(appearance)?;
    coconut_integration_qt::publish(store, appearance)?;
    coconut_integration_cream::publish_native(store, appearance)?;
    let shell = toml::to_string_pretty(&appearance.shell).map_err(|e| e.to_string())?;
    coconut_theme::atomic_write(&store.generated_dir().join("shell.toml"), &shell)?;
    Ok(theme_name)
}

fn announce(connection: &Connection) -> Result<(), String> {
    let signal = dbus::Message::new_signal(
        "/org/coconut/Appearance",
        "org.coconut.Appearance",
        "Changed",
    )?;
    connection
        .send(signal)
        .map_err(|_| "could not publish Coconut appearance".to_owned())?;
    creamui_theme_watch::publish().map_err(|e| e.to_string())
}

/// Serve the backend until the session bus disconnects. Also detects direct
/// edits to appearance, accessibility, and the selected theme's definition.
pub fn run() -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    if connection
        .request_name(SERVICE, false, false, true)
        .map_err(|error| error.to_string())?
        != RequestNameReply::PrimaryOwner
    {
        return Ok(());
    }
    let dirty = Arc::new(AtomicBool::new(false));
    let reload = dirty.clone();
    connection
        .add_match(
            MatchRule::new_signal(creamui_theme_watch::INTERFACE, creamui_theme_watch::SIGNAL)
                .with_path(creamui_theme_watch::OBJECT_PATH),
            move |_: (), _, _| {
                reload.store(true, Ordering::Relaxed);
                true
            },
        )
        .map_err(|error| error.to_string())?;

    let store = Store::default();
    let mut appearance = match coconut_integration_cream::import_legacy(&store)
        .and_then(|()| store.ensure_selection())
    {
        Ok(appearance) => appearance,
        Err(error) => {
            eprintln!("coconut-appearance: using the bundled theme until the saved selection is valid: {error}");
            coconut_theme::Package::builtin()?.resolve(&coconut_theme::Selection::default())?
        }
    };
    let mut bridge = gtk::Bridge::new(store.paths.clone());
    let theme_name = publish_outputs(&store, &mut bridge, &appearance)?;
    let settings = Arc::new(Mutex::new(portal::settings(&appearance, &theme_name)));
    let values = settings.clone();
    let mut rule = MatchRule::new_method_call();
    rule.path = Some(PATH.into());
    connection.start_receive(
        rule,
        Box::new(move |message, connection| {
            let reply = portal::dispatch(&message, &values.lock().unwrap());
            let _ = connection.send(reply);
            true
        }),
    );
    portal::forward_gtk_changes(&connection, settings.clone())?;
    // Re-activation must also refresh apps that stayed open while the shell
    // was restarted and only follow standard portal notifications.
    portal::publish_changes(
        &connection,
        &portal::Settings::new(),
        &settings.lock().unwrap(),
    )?;
    announce(&connection)?;

    let mut last_poll = Instant::now();
    let mut changed_at = None;
    let mut last_error = None;
    loop {
        connection
            .process(Duration::from_millis(50))
            .map_err(|error| error.to_string())?;
        if dirty.swap(false, Ordering::Relaxed) {
            changed_at = Some(Instant::now());
        }
        let signaled = changed_at.is_some_and(|time| time.elapsed() >= Duration::from_millis(120));
        if signaled || last_poll.elapsed() >= Duration::from_secs(1) {
            changed_at = None;
            last_poll = Instant::now();
            match store.load() {
                Ok(next) if next != appearance => {
                    let theme_name = match publish_outputs(&store, &mut bridge, &next) {
                        Ok(name) => name,
                        Err(error) => {
                            report_error(&mut last_error, error);
                            continue;
                        }
                    };
                    let next_settings = portal::settings(&next, &theme_name);
                    // Publish the new snapshot before clients can react to signals.
                    let mut values = settings.lock().unwrap();
                    let previous = std::mem::replace(&mut *values, next_settings);
                    portal::publish_changes(&connection, &previous, &values)?;
                    appearance = next;
                    last_error = None;
                    announce(&connection)?;
                }
                Ok(_) => {
                    last_error = None;
                    bridge.maintain_xsettings();
                }
                Err(error) => report_error(&mut last_error, error),
            }
        }
    }
}

fn report_error(previous: &mut Option<String>, error: String) {
    if previous.as_ref() != Some(&error) {
        eprintln!("coconut-appearance: keeping the last valid appearance: {error}");
        *previous = Some(error);
    }
}
