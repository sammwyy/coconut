use dbus::blocking::Connection;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const KDE_AGENT_BUS_NAME: &str = "org.kde.polkit-kde-authentication-agent-1";

pub fn ensure_kde_agent() {
    if !is_kde_session() || agent_is_running() {
        return;
    }
    let Some(program) = kde_agent_program() else {
        return;
    };
    if let Err(error) = Command::new(program).spawn() {
        eprintln!("settings: could not start KDE's authorization agent: {error}");
    }
}

fn is_kde_session() -> bool {
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "KDE_FULL_SESSION",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok())
    .any(|value| {
        let value = value.to_ascii_lowercase();
        value.contains("kde") || value.contains("plasma") || value.contains("kwin")
    })
}

fn agent_is_running() -> bool {
    let Ok(connection) = Connection::new_session() else {
        return false;
    };
    connection
        .with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            Duration::from_secs(1),
        )
        .method_call(
            "org.freedesktop.DBus",
            "NameHasOwner",
            (KDE_AGENT_BUS_NAME,),
        )
        .map(|(running,): (bool,)| running)
        .unwrap_or(false)
}

fn kde_agent_program() -> Option<PathBuf> {
    [
        "/usr/lib/polkit-kde-authentication-agent-1",
        "/usr/libexec/polkit-kde-authentication-agent-1",
        "/usr/lib64/polkit-kde-authentication-agent-1",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .or_else(|| find_in_path("polkit-kde-authentication-agent-1"))
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(program))
        .find(|path| path.is_file())
}
