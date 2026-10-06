//! Linux service adapters. No settings are changed during discovery. Every
//! write is an explicit UI action and failures (including polkit) are returned.
mod applications;
mod hardware;
use coconut_api::settings::{
    Action, Choice, Entry, Preference, SettingsIntegration, Snapshot, Value,
};
use dbus::blocking::{stdintf::org_freedesktop_dbus::Properties, Connection};
use std::{collections::BTreeMap, fs, process::Command, time::Duration};

pub struct LinuxSettings;
const TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) fn command(program: &str, args: &[&str]) -> Result<String, String> {
    command_timeout(program, args, Duration::from_secs(8))
}
fn command_timeout(program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    use std::{io::Read, os::unix::process::CommandExt, process::Stdio, thread, time::Instant};
    let mut helper = Command::new(program);
    helper
        .args(args)
        .env("LC_ALL", "C")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        helper.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            libc::setrlimit(libc::RLIMIT_CORE, &limit);
            Ok(())
        });
    }
    let mut child = helper
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let capture = |mut pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut chunk = [0; 8192];
            while let Ok(length) = pipe.read(&mut chunk) {
                if length == 0 {
                    break;
                }
                if bytes.len() < 16 * 1024 * 1024 {
                    bytes.extend_from_slice(&chunk[..length]);
                }
            }
            bytes
        })
    };
    let stdout = capture(Box::new(child.stdout.take().unwrap()));
    let stderr = capture(Box::new(child.stderr.take().unwrap()));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            _ => {
                // This process group contains only the helper we just spawned.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                timed_out = true;
                let _ = child.wait();
                break None;
            }
        }
    };
    // A timed-out helper may have left children holding its output pipes open.
    if timed_out {
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    let output = stdout.join().unwrap_or_default();
    let error = stderr.join().unwrap_or_default();
    let status = status.ok_or_else(|| format!("{program}: service did not respond in time"))?;
    if !status.success() {
        let message = String::from_utf8_lossy(&error).trim().to_owned();
        return Err(if message.is_empty() {
            format!("{program} exited with {status}")
        } else {
            message
        });
    }
    Ok(String::from_utf8_lossy(&output).trim().to_owned())
}
pub(crate) fn read(path: impl AsRef<std::path::Path>) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_owned()
}
pub(crate) fn text(value: impl Into<String>, writable: bool) -> Preference {
    Preference {
        value: Some(Value::Text(value.into())),
        writable,
        ..Default::default()
    }
}
pub(crate) fn number(value: f64, writable: bool) -> Preference {
    Preference {
        value: Some(Value::Number(value)),
        writable,
        ..Default::default()
    }
}
pub(crate) fn boolean(value: bool, writable: bool) -> Preference {
    Preference {
        value: Some(Value::Bool(value)),
        writable,
        ..Default::default()
    }
}
pub(crate) fn unavailable(reason: impl Into<String>) -> Preference {
    Preference {
        reason: reason.into(),
        ..Default::default()
    }
}

impl SettingsIntegration for LinuxSettings {
    fn snapshot(&self) -> Snapshot {
        let mut result = Snapshot {
            loaded: true,
            session: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Wayland".into()),
            ..Default::default()
        };
        if blair_available() {
            result.session = "Blair".into();
        }
        hardware::discover(&mut result);
        applications::discover(&mut result);
        discover_system(&mut result);
        discover_preferences(&mut result);
        discover_privacy(&mut result);
        result
    }
    fn apply(&self, action: Action) -> Result<String, String> {
        match action {
            Action::Set { key, value } => set_preference(&key, value)?,
            Action::SetDefaultApp { mime, desktop_id } => {
                applications::set_default(&mime, &desktop_id)?
            }
            Action::SetStartup { id, enabled } => applications::set_startup(&id, enabled)?,
            Action::AddStartup { desktop_id } => applications::add_startup(&desktop_id)?,
            Action::SetCameraPermission { app, allowed } => {
                set_device_permission("camera", &app, allowed)?
            }
            Action::SetDevicePermission {
                device,
                app,
                allowed,
            } => set_device_permission(&device, &app, allowed)?,
            Action::AddPrinter { name, uri } => hardware::add_printer(&name, &uri)?,
            Action::CheckUpdates => {
                return command_timeout(
                    "pkcon",
                    &["--noninteractive", "get-updates"],
                    Duration::from_secs(120),
                )
                .or_else(|first| {
                    command_timeout(
                        "flatpak",
                        &["remote-ls", "--updates", "--columns=application,version"],
                        Duration::from_secs(120),
                    )
                    .map(|output| {
                        if output.is_empty() {
                            "No Flatpak updates available (system packages were not checked)".into()
                        } else {
                            output
                        }
                    })
                    .map_err(|second| format!("PackageKit: {first}\nFlatpak: {second}"))
                });
            }
            Action::ChangePassword { password } => change_password(password)?,
            Action::CreateUser {
                username,
                real_name,
                administrator,
            } => create_user(&username, &real_name, administrator)?,
        }
        Ok("Changes applied".into())
    }
}

fn blair_available() -> bool {
    Connection::new_session().ok().is_some_and(|connection| {
        let result: Result<(bool,), _> = connection
            .with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", TIMEOUT)
            .method_call(
                "org.freedesktop.DBus",
                "NameHasOwner",
                ("org.blair.Compositor",),
            );
        result.is_ok_and(|(owned,)| owned)
    })
}

fn system_property<T: for<'a> dbus::arg::Get<'a> + 'static>(
    service: &str,
    property: &str,
) -> Option<T> {
    let connection = Connection::new_system().ok()?;
    let path = format!("/{}", service.replace('.', "/"));
    connection
        .with_proxy(service, path, TIMEOUT)
        .get(service, property)
        .ok()
}
fn set_system(service: &str, method: &str, value: &str) -> Result<(), String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let path = format!("/{}", service.replace('.', "/"));
    let _: () = connection
        .with_proxy(service, path, Duration::from_secs(120))
        .method_call(service, method, (value, true))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn discover_system(snapshot: &mut Snapshot) {
    if let Ok(fonts) = command("fc-list", &["--format=%{family}\n"]) {
        let mut families: std::collections::BTreeSet<String> = fonts
            .lines()
            .flat_map(|line| line.split(','))
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
        families.insert("Plus Jakarta Sans".into());
        snapshot.collections.insert(
            "fonts".into(),
            families
                .into_iter()
                .map(|name| Entry {
                    id: name.clone(),
                    name,
                    enabled: true,
                    ..Default::default()
                })
                .collect(),
        );
    }
    let facts = &mut snapshot.facts;
    let os = read("/etc/os-release");
    facts.insert(
        "os".into(),
        os.lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME="))
            .unwrap_or("Linux")
            .trim_matches('"')
            .into(),
    );
    facts.insert("hostname".into(), read("/proc/sys/kernel/hostname"));
    facts.insert("kernel".into(), read("/proc/sys/kernel/osrelease"));
    let cpu = read("/proc/cpuinfo");
    facts.insert(
        "processor".into(),
        cpu.lines()
            .find_map(|line| {
                line.strip_prefix("model name")
                    .and_then(|part| part.split_once(':').map(|(_, value)| value.trim()))
            })
            .unwrap_or("Unavailable")
            .into(),
    );
    facts.insert(
        "cores".into(),
        cpu.lines()
            .filter(|line| line.starts_with("processor\t"))
            .count()
            .to_string(),
    );
    let memory = read("/proc/meminfo");
    if let Some(total) = memory.lines().find_map(|line| {
        line.strip_prefix("MemTotal:")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
    }) {
        facts.insert(
            "memory".into(),
            format!("{:.1} GiB", total as f64 / 1048576.0),
        );
    }
    facts.insert(
        "uptime".into(),
        read("/proc/uptime")
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|secs| format!("{:.0} hours", secs / 3600.0))
            .unwrap_or_default(),
    );
    if let Ok(pci) = command("lspci", &[]) {
        facts.insert(
            "graphics".into(),
            pci.lines()
                .filter(|line| {
                    line.contains("VGA")
                        || line.contains("3D controller")
                        || line.contains("Display controller")
                })
                .map(|line| line.split_once(':').map(|(_, v)| v.trim()).unwrap_or(line))
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
    if let Ok(storage) = command(
        "df",
        &["-B1", "--output=source,fstype,size,used,avail,target"],
    ) {
        let entries = storage
            .lines()
            .skip(1)
            .filter_map(|line| {
                let columns: Vec<_> = line.split_whitespace().collect();
                if columns.len() < 6 || matches!(columns[1], "tmpfs" | "devtmpfs") {
                    return None;
                }
                let total: f64 = columns[2].parse().ok()?;
                let used: f64 = columns[3].parse().ok()?;
                Some(Entry {
                    id: columns[5..].join(" "),
                    name: columns[5..].join(" "),
                    description: format!(
                        "{:.1} / {:.1} GiB used · {} · {}",
                        used / 1073741824.0,
                        total / 1073741824.0,
                        columns[1],
                        columns[0]
                    ),
                    enabled: true,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        if let Some(root) = entries.iter().find(|entry| entry.id == "/") {
            facts.insert("storage".into(), root.description.clone());
        }
        snapshot.collections.insert("storage".into(), entries);
    }
    if let Ok(processes) = command("ps", &["-eo", "pid,pcpu,pmem,comm", "--sort=-pcpu"]) {
        snapshot.collections.insert(
            "activity".into(),
            processes
                .lines()
                .skip(1)
                .take(50)
                .filter_map(|line| {
                    let v: Vec<_> = line.split_whitespace().collect();
                    (v.len() >= 4).then(|| Entry {
                        id: v[0].into(),
                        name: v[3..].join(" "),
                        description: format!("PID {} · CPU {}% · Memory {}%", v[0], v[1], v[2]),
                        enabled: true,
                        ..Default::default()
                    })
                })
                .collect(),
        );
    }
    let timezone = system_property::<String>("org.freedesktop.timedate1", "Timezone");
    snapshot.preferences.insert(
        "timezone".into(),
        timezone
            .map(|v| text(v, true))
            .unwrap_or_else(|| unavailable("systemd-timedated is not available")),
    );
    let ntp = system_property::<bool>("org.freedesktop.timedate1", "NTP");
    snapshot.preferences.insert(
        "ntp".into(),
        ntp.map(|v| boolean(v, true))
            .unwrap_or_else(|| unavailable("systemd-timedated is not available")),
    );
    let locales = command("localectl", &["list-locales"]).unwrap_or_default();
    let locale =
        system_property::<Vec<String>>("org.freedesktop.locale1", "Locale").and_then(|items| {
            items
                .into_iter()
                .find_map(|v| v.strip_prefix("LANG=").map(str::to_owned))
        });
    let mut pref = locale
        .map(|v| text(v, true))
        .unwrap_or_else(|| unavailable("systemd-localed is not available"));
    pref.choices = locales
        .lines()
        .map(|v| Choice {
            id: v.into(),
            label: v.into(),
        })
        .collect();
    snapshot.preferences.insert("locale".into(), pref);
    let host = system_property::<String>("org.freedesktop.hostname1", "StaticHostname");
    snapshot.preferences.insert(
        "hostname".into(),
        host.map(|v| text(v, true))
            .unwrap_or_else(|| text(read("/proc/sys/kernel/hostname"), false)),
    );
}

// These settings are only advertised when the desktop that consumes them is
// actually running. Installing a schema alone does not make Blair implement it.
fn gnome_mapping(key: &str) -> Option<(&'static str, &'static str, bool)> {
    Some(match key {
        "contrast" => ("org.gnome.desktop.a11y.interface", "high-contrast", false),
        "zoom" => (
            "org.gnome.desktop.a11y.applications",
            "screen-magnifier-enabled",
            false,
        ),
        "text-scale" => ("org.gnome.desktop.interface", "text-scaling-factor", false),
        "reduce-motion" => ("org.gnome.desktop.interface", "enable-animations", true),
        "visual-alerts" => ("org.gnome.desktop.wm.preferences", "visual-bell", false),
        "sticky-keys" => (
            "org.gnome.desktop.a11y.keyboard",
            "stickykeys-enable",
            false,
        ),
        "lock-after-sleep" => ("org.gnome.desktop.screensaver", "lock-enabled", false),
        "lock-delay" => ("org.gnome.desktop.screensaver", "lock-delay", false),
        "location" => ("org.gnome.system.location", "enabled", false),
        "notifications" => ("org.gnome.desktop.notifications", "show-banners", false),
        "idle-dim" => ("org.gnome.settings-daemon.plugins.power", "idle-dim", false),
        _ => return None,
    })
}
fn discover_preferences(snapshot: &mut Snapshot) {
    let gnome = snapshot.session.to_lowercase().contains("gnome");
    for key in [
        "contrast",
        "zoom",
        "text-scale",
        "reduce-motion",
        "visual-alerts",
        "sticky-keys",
        "lock-after-sleep",
        "lock-delay",
        "location",
        "notifications",
        "idle-dim",
    ] {
        let (schema, property, invert) = gnome_mapping(key).unwrap();
        let pref = if gnome {
            command("gsettings", &["get", schema, property])
                .ok()
                .and_then(|output| {
                    let value = match output.as_str() {
                        "true" => Value::Bool(!invert),
                        "false" => Value::Bool(invert),
                        _ => Value::Number(output.split_whitespace().last()?.parse().ok()?),
                    };
                    Some(Preference {
                        value: Some(value),
                        writable: command("gsettings", &["writable", schema, property])
                            .is_ok_and(|v| v == "true"),
                        ..Default::default()
                    })
                })
                .unwrap_or_else(|| {
                    unavailable("This setting is not provided by the active desktop")
                })
        } else {
            unavailable(format!(
                "Not supported by the active compositor ({})",
                snapshot.session
            ))
        };
        snapshot.preferences.insert(key.into(), pref);
    }
}

fn set_preference(key: &str, value: Value) -> Result<(), String> {
    if key.starts_with("display:") {
        return Err("Display mode changes require safe apply/revert support".into());
    }
    match (key, &value) {
        ("hostname", Value::Text(name)) => {
            if name.is_empty()
                || name.len() > 253
                || !name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
            {
                return Err("Use a valid hostname (letters, digits, hyphens and dots)".into());
            }
            set_system("org.freedesktop.hostname1", "SetStaticHostname", name)
        }
        ("timezone", Value::Text(zone)) => {
            set_system("org.freedesktop.timedate1", "SetTimezone", zone)
        }
        ("ntp", Value::Bool(enabled)) => {
            let connection = Connection::new_system().map_err(|e| e.to_string())?;
            let _: () = connection
                .with_proxy(
                    "org.freedesktop.timedate1",
                    "/org/freedesktop/timedate1",
                    Duration::from_secs(120),
                )
                .method_call("org.freedesktop.timedate1", "SetNTP", (*enabled, true))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        ("locale", Value::Text(locale)) => {
            if !command("localectl", &["list-locales"])?
                .lines()
                .any(|v| v == locale)
            {
                return Err("Choose an installed locale".into());
            }
            let connection = Connection::new_system().map_err(|e| e.to_string())?;
            let _: () = connection
                .with_proxy(
                    "org.freedesktop.locale1",
                    "/org/freedesktop/locale1",
                    Duration::from_secs(120),
                )
                .method_call(
                    "org.freedesktop.locale1",
                    "SetLocale",
                    (vec![format!("LANG={locale}")], true),
                )
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        (
            "volume" | "brightness" | "microphone-muted" | "sound-output" | "sound-input"
            | "power-mode",
            _,
        ) => hardware::set_preference(key, value),
        _ => {
            let (schema, property, invert) =
                gnome_mapping(key).ok_or("This setting is not supported")?;
            if blair_available()
                || !std::env::var("XDG_CURRENT_DESKTOP")
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains("gnome")
            {
                return Err("The active desktop does not consume this preference".into());
            }
            let formatted = match value {
                Value::Bool(value) => (value ^ invert).to_string(),
                Value::Number(value) if value.is_finite() => value.to_string(),
                _ => return Err("Invalid preference value".into()),
            };
            command("gsettings", &["set", schema, property, &formatted]).map(|_| ())
        }
    }
}

fn discover_privacy(snapshot: &mut Snapshot) {
    for device in ["camera", "microphone"] {
        match device_permissions(device) {
            Ok(entries) => {
                snapshot
                    .collections
                    .insert(format!("{device}-permissions"), entries);
                snapshot
                    .facts
                    .insert(format!("{device}-service"), "Desktop portal".into());
            }
            Err(error) => {
                snapshot.facts.insert(format!("{device}-service"), error);
            }
        }
    }
    discover_online_accounts(snapshot);
    if let Ok(connection) = Connection::new_system() {
        let device: Result<(dbus::Path<'static>,), _> = connection
            .with_proxy(
                "net.reactivated.Fprint",
                "/net/reactivated/Fprint/Manager",
                TIMEOUT,
            )
            .method_call("net.reactivated.Fprint.Manager", "GetDefaultDevice", ());
        if let Ok((path,)) = device {
            let proxy = connection.with_proxy("net.reactivated.Fprint", path, TIMEOUT);
            if let Ok(name) = proxy.get::<String>("net.reactivated.Fprint.Device", "name") {
                snapshot.facts.insert("fingerprint".into(), name);
            }
            let user = std::env::var("USER").unwrap_or_default();
            let enrolled: Result<(Vec<String>,), _> = proxy.method_call(
                "net.reactivated.Fprint.Device",
                "ListEnrolledFingers",
                (user,),
            );
            if let Ok((fingers,)) = enrolled {
                snapshot.collections.insert(
                    "fingerprints".into(),
                    fingers
                        .into_iter()
                        .map(|name| Entry {
                            id: name.clone(),
                            name,
                            ..Default::default()
                        })
                        .collect(),
                );
            }
        }
    }
    snapshot.facts.insert(
        "camera-devices".into(),
        fs::read_dir("/sys/class/video4linux")
            .map(|items| {
                items
                    .flatten()
                    .map(|item| read(item.path().join("name")))
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default(),
    );
}
type PortalPermissions = BTreeMap<String, Vec<String>>;
fn device_permissions(device: &str) -> Result<Vec<Entry>, String> {
    if !matches!(device, "camera" | "microphone") {
        return Err("Unsupported permission device".into());
    }
    let connection = Connection::new_session().map_err(|e| e.to_string())?;
    let proxy = connection.with_proxy(
        "org.freedesktop.impl.portal.PermissionStore",
        "/org/freedesktop/impl/portal/PermissionStore",
        TIMEOUT,
    );
    let result: Result<
        (
            PortalPermissions,
            dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>,
        ),
        _,
    > = proxy.method_call(
        "org.freedesktop.impl.portal.PermissionStore",
        "Lookup",
        ("devices", device),
    );
    let (permissions, _) = match result {
        Ok(value) => value,
        Err(error) if error.name() == Some("org.freedesktop.portal.Error.NotFound") => {
            return Ok(Vec::new())
        }
        Err(error) => return Err(error.to_string()),
    };
    Ok(permissions
        .into_iter()
        .map(|(app, permissions)| Entry {
            id: app.clone(),
            name: app,
            description: permissions.join(", "),
            enabled: permissions.iter().any(|v| v == "yes"),
            ..Default::default()
        })
        .collect())
}
fn set_device_permission(device: &str, app: &str, allowed: bool) -> Result<(), String> {
    if !device_permissions(device)?
        .iter()
        .any(|entry| entry.id == app)
    {
        return Err("This application has no saved portal permission".into());
    }
    let connection = Connection::new_session().map_err(|e| e.to_string())?;
    let _: () = connection
        .with_proxy(
            "org.freedesktop.impl.portal.PermissionStore",
            "/org/freedesktop/impl/portal/PermissionStore",
            TIMEOUT,
        )
        .method_call(
            "org.freedesktop.impl.portal.PermissionStore",
            "SetPermission",
            (
                "devices",
                false,
                device,
                app,
                vec![if allowed { "yes" } else { "no" }],
            ),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn change_password(password: String) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;
    if password.len() < 8 || password.contains('\0') || password.contains('\n') {
        return Err("Use a password of at least 8 characters without newlines".into());
    }
    let mut child = Command::new("openssl")
        .args(["passwd", "-6", "-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("Password input is unavailable")?
        .write_all(password.as_bytes())
        .map_err(|e| e.to_string())?;
    let result = child.wait_with_output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err("Password hashing failed".into());
    }
    let hash = String::from_utf8(result.stdout).map_err(|_| "Password hashing failed")?;
    if !hash.trim().starts_with("$6$") {
        return Err("Unsupported password hash".into());
    }
    let connection = Connection::new_system().map_err(|e| e.to_string())?;
    let uid = unsafe { libc::geteuid() } as i64;
    let (path,): (dbus::Path<'static>,) = connection
        .with_proxy(
            "org.freedesktop.Accounts",
            "/org/freedesktop/Accounts",
            TIMEOUT,
        )
        .method_call("org.freedesktop.Accounts", "FindUserById", (uid,))
        .map_err(|e| e.to_string())?;
    let _: () = connection
        .with_proxy("org.freedesktop.Accounts", path, Duration::from_secs(120))
        .method_call(
            "org.freedesktop.Accounts.User",
            "SetPassword",
            (hash.trim(), ""),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn create_user(username: &str, name: &str, administrator: bool) -> Result<(), String> {
    if username.is_empty()
        || username.len() > 32
        || !username.starts_with(|ch: char| ch.is_ascii_lowercase() || ch == '_')
        || !username
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err("Use a valid lowercase system username".into());
    }
    if name.contains('\0') || name.contains('\n') {
        return Err("Invalid display name".into());
    }
    let connection = Connection::new_system().map_err(|e| e.to_string())?;
    let (_path,): (dbus::Path<'static>,) = connection
        .with_proxy(
            "org.freedesktop.Accounts",
            "/org/freedesktop/Accounts",
            Duration::from_secs(120),
        )
        .method_call(
            "org.freedesktop.Accounts",
            "CreateUser",
            (username, name, i32::from(administrator)),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn discover_online_accounts(snapshot: &mut Snapshot) {
    type Objects = BTreeMap<dbus::Path<'static>, BTreeMap<String, dbus::arg::PropMap>>;
    let accounts = Connection::new_session()
        .map_err(|e| e.to_string())
        .and_then(|connection| {
            let result: Result<(Objects,), _> = connection
                .with_proxy(
                    "org.gnome.OnlineAccounts",
                    "/org/gnome/OnlineAccounts",
                    TIMEOUT,
                )
                .method_call(
                    "org.freedesktop.DBus.ObjectManager",
                    "GetManagedObjects",
                    (),
                );
            result.map(|(objects,)| objects).map_err(|e| e.to_string())
        });
    match accounts {
        Ok(objects) => {
            let entries = objects
                .into_iter()
                .filter_map(|(path, mut interfaces)| {
                    let properties = interfaces.remove("org.gnome.OnlineAccounts.Account")?;
                    let get = |key: &str| {
                        properties
                            .get(key)
                            .and_then(|v| v.0.as_str())
                            .unwrap_or_default()
                            .to_owned()
                    };
                    Some(Entry {
                        id: path.to_string(),
                        name: get("PresentationIdentity"),
                        description: get("ProviderName"),
                        enabled: true,
                        ..Default::default()
                    })
                })
                .collect();
            snapshot
                .collections
                .insert("online-accounts".into(), entries);
            snapshot.facts.insert(
                "online-accounts-service".into(),
                "GNOME Online Accounts · account changes are managed by the provider".into(),
            );
        }
        Err(error) => {
            snapshot.facts.insert(
                "online-accounts-service".into(),
                format!("No online-account service available: {error}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hung_helpers_are_bounded_and_reaped() {
        let start = std::time::Instant::now();
        let result = command_timeout("sleep", &["5"], Duration::from_millis(50));
        assert!(result.unwrap_err().contains("did not respond"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn helper_failures_are_not_reported_as_success() {
        assert!(command("false", &[]).is_err());
        assert!(command("coconut-missing-test-helper", &[]).is_err());
        assert_eq!(
            command("printf", &["fixture output"]).unwrap(),
            "fixture output"
        );
    }
    #[test]
    fn unsupported_or_invalid_writes_fail_before_touching_services() {
        for (key, value) in [
            ("invented-option", Value::Bool(true)),
            ("volume", Value::Number(f64::NAN)),
            ("brightness", Value::Number(2.0)),
            ("hostname", Value::Text("bad/name".into())),
            ("display:1", Value::Text("99".into())),
        ] {
            assert!(set_preference(key, value).is_err());
        }
        assert!(create_user("../../bad", "", false).is_err());
        assert!(hardware::add_printer("bad/name", "ipp://test").is_err());
        assert!(hardware::add_printer("good-name", "file:///tmp/test").is_err());
        assert!(change_password("short".into()).is_err());
        assert!(device_permissions("invented-device").is_err());
    }
    #[test]
    #[ignore = "Requires real Linux services; read-only opt-in smoke test"]
    fn discovers_real_services_without_writes() {
        let snapshot = LinuxSettings.snapshot();
        assert!(snapshot.loaded);
        assert!(!snapshot.fact("hostname").is_empty());
        assert!(!snapshot.fact("os").is_empty());
        println!(
            "session={} applications={} devices={} displays={} audio={:?}",
            snapshot.session,
            snapshot.entries("applications").len(),
            snapshot.entries("devices").len(),
            snapshot.entries("displays").len(),
            snapshot.preferences.get("volume")
        );
    }
}
