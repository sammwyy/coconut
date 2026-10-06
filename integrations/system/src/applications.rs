use super::*;
use std::path::{Path, PathBuf};

fn config_dir() -> Result<PathBuf, String> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or("User configuration directory is unavailable".into())
}
fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    {
        dirs.push(dir.join("applications"));
        dirs.push(dir.join("flatpak/exports/share/applications"));
    }
    dirs.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| PathBuf::from(dir).join("applications")),
    );
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}
fn desktop_values(source: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut in_entry = false;
    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.into(), value.into());
        }
    }
    values
}
fn entry(path: &Path, id: String) -> Option<Entry> {
    let values = desktop_values(&fs::read_to_string(path).ok()?);
    if values.get("Type").is_some_and(|v| v != "Application") {
        return None;
    }
    let name = values.get("Name")?.clone();
    let mut properties = values;
    properties.insert("path".into(), path.to_string_lossy().into());
    Some(Entry {
        id,
        name,
        description: properties.get("Comment").cloned().unwrap_or_default(),
        enabled: properties.get("Hidden").is_none_or(|v| v != "true"),
        properties,
    })
}
fn scan_desktops(dir: &Path, prefix: &str, depth: usize, entries: &mut BTreeMap<String, Entry>) {
    if depth > 3 {
        return;
    }
    let Ok(files) = fs::read_dir(dir) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name().to_string_lossy().into_owned();
        if file.file_type().is_ok_and(|kind| kind.is_dir()) {
            scan_desktops(
                &file.path(),
                &format!("{prefix}{name}-"),
                depth + 1,
                entries,
            );
        } else if name.ends_with(".desktop") {
            let id = format!("{prefix}{name}");
            if !entries.contains_key(&id) {
                if let Some(entry) = entry(&file.path(), id.clone()) {
                    entries.insert(id, entry);
                }
            }
        }
    }
}
fn installed() -> Vec<Entry> {
    let mut entries = BTreeMap::new();
    for dir in data_dirs() {
        scan_desktops(&dir, "", 0, &mut entries);
    }
    let mut entries: Vec<_> = entries
        .into_values()
        .filter(|entry| {
            entry.enabled
                && entry
                    .properties
                    .get("NoDisplay")
                    .is_none_or(|v| v != "true")
        })
        .collect();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    entries
}
fn startup() -> Vec<Entry> {
    let mut entries = BTreeMap::new();
    if let Ok(config) = config_dir() {
        scan_desktops(&config.join("autostart"), "", 0, &mut entries);
    }
    for dir in std::env::var("XDG_CONFIG_DIRS")
        .unwrap_or_else(|_| "/etc/xdg".into())
        .split(':')
        .filter(|dir| !dir.is_empty())
    {
        scan_desktops(&PathBuf::from(dir).join("autostart"), "", 0, &mut entries);
    }
    entries.into_values().collect()
}
pub fn discover(snapshot: &mut Snapshot) {
    let apps = installed();
    let current = command("xdg-mime", &["query", "default", "x-scheme-handler/https"])
        .ok()
        .filter(|value| !value.is_empty());
    snapshot.preferences.insert(
        "browser".into(),
        Preference {
            value: current.map(Value::Text),
            writable: true,
            choices: apps
                .iter()
                .filter(|entry| {
                    entry.properties.get("MimeType").is_some_and(|types| {
                        types
                            .split(';')
                            .any(|mime| mime == "x-scheme-handler/https" || mime == "text/html")
                    })
                })
                .map(|entry| Choice {
                    id: entry.id.clone(),
                    label: entry.name.clone(),
                })
                .collect(),
            reason: "No default browser is configured".into(),
        },
    );
    if snapshot.session == "Blair" {
        match blair_configuration() {
            Ok(configuration) => {
                let entries = configuration
                    .get("autostart")
                    .and_then(toml::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        let command = entry.get("command")?.as_str()?;
                        let app = apps.iter().find(|app| {
                            app.properties
                                .get("path")
                                .is_some_and(|path| desktop_launch_command(path) == command)
                        });
                        Some(Entry {
                            id: format!("blair:{command}"),
                            name: app
                                .map(|app| app.name.clone())
                                .unwrap_or_else(|| command.into()),
                            description: format!(
                                "Blair startup · {}",
                                if entry
                                    .get("restart")
                                    .and_then(toml::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    "restart on exit"
                                } else {
                                    "once per session"
                                }
                            ),
                            enabled: true,
                            ..Default::default()
                        })
                    })
                    .collect();
                snapshot.collections.insert("startup".into(), entries);
                snapshot.facts.insert("startup-service".into(), "Blair startup commands. Turning an entry off removes it from startup, without stopping the running application. Changes apply next session.".into());
            }
            Err(error) => {
                snapshot.facts.insert("startup-service".into(), error);
            }
        }
    } else {
        snapshot.collections.insert("startup".into(), startup());
        snapshot.facts.insert("startup-service".into(), "XDG autostart entries, consumed by the session manager. Desktop-specific restrictions still apply. Changes create a per-user override and preserve system entries.".into());
    }
    let mut defaults = BTreeMap::new();
    let paths = config_dir()
        .ok()
        .into_iter()
        .map(|dir| dir.join("mimeapps.list"))
        .chain(data_dirs().into_iter().map(|dir| dir.join("mimeapps.list")));
    for path in paths {
        let content = read(path);
        let mut in_defaults = false;
        for line in content.lines().map(str::trim) {
            if line.starts_with('[') {
                in_defaults = line == "[Default Applications]";
            } else if in_defaults {
                if let Some((mime, ids)) = line.split_once('=') {
                    defaults
                        .entry(mime.to_owned())
                        .or_insert_with(|| ids.split(';').next().unwrap_or_default().to_owned());
                }
            }
        }
    }
    snapshot.collections.insert(
        "file-types".into(),
        defaults
            .into_iter()
            .map(|(mime, id)| Entry {
                name: mime.clone(),
                id: mime,
                description: apps
                    .iter()
                    .find(|app| app.id == id)
                    .map(|app| app.name.clone())
                    .unwrap_or_else(|| id.clone()),
                enabled: true,
                properties: BTreeMap::from([("desktop-id".into(), id)]),
            })
            .collect(),
    );
    snapshot.collections.insert("applications".into(), apps);
}
fn validate_id(id: &str) -> Result<(), String> {
    if id.ends_with(".desktop")
        && !id.starts_with(['.', '-'])
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
    {
        Ok(())
    } else {
        Err("Invalid desktop application identifier".into())
    }
}
pub fn set_default(mime: &str, id: &str) -> Result<(), String> {
    validate_id(id)?;
    if !mime.contains('/') || mime.starts_with('-') || mime.chars().any(char::is_whitespace) {
        return Err("Invalid MIME type".into());
    }
    if !installed().iter().any(|entry| entry.id == id) {
        return Err("This application is no longer installed".into());
    }
    command("xdg-mime", &["default", id, mime])?;
    if mime == "x-scheme-handler/https" {
        command("xdg-mime", &["default", id, "x-scheme-handler/http"])?;
    }
    Ok(())
}
fn with_hidden(source: &str, hidden: bool) -> String {
    let mut lines = Vec::new();
    let mut in_entry = false;
    let mut wrote = false;
    for line in source.lines() {
        if line.starts_with('[') {
            if in_entry && !wrote {
                lines.push(format!("Hidden={hidden}"));
                wrote = true;
            }
            in_entry = line == "[Desktop Entry]";
        }
        if in_entry && line.starts_with("Hidden=") {
            if !wrote {
                lines.push(format!("Hidden={hidden}"));
                wrote = true;
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    if in_entry && !wrote {
        lines.push(format!("Hidden={hidden}"));
    }
    format!("{}\n", lines.join("\n"))
}
fn write_startup(entry: Entry, enabled: bool) -> Result<(), String> {
    validate_id(&entry.id)?;
    let source = entry
        .properties
        .get("path")
        .ok_or("The application entry is unavailable")?;
    let source = fs::read_to_string(source).map_err(|e| e.to_string())?;
    let directory = config_dir()?.join("autostart");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let target = directory.join(&entry.id);
    if fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("Refusing to replace a symbolic link in autostart".into());
    }
    use std::io::Write;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    if target.exists() {
        let backup = directory.join(format!("{}.backup-{stamp}", entry.id));
        let bytes = fs::read(&target).map_err(|e| e.to_string())?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(backup)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    let temporary = directory.join(format!(".{}.{stamp}.tmp", entry.id));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(with_hidden(&source, !enabled).as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, target).map_err(|e| e.to_string())
}
pub fn set_startup(id: &str, enabled: bool) -> Result<(), String> {
    if let Some(command) = id.strip_prefix("blair:") {
        let mut config = blair_configuration()?;
        let entries = config
            .get_mut("autostart")
            .and_then(toml::Value::as_array_mut)
            .ok_or("No Blair startup entries are configured")?;
        let index = entries
            .iter()
            .position(|entry| entry.get("command").and_then(toml::Value::as_str) == Some(command))
            .ok_or("The startup command is no longer configured")?;
        if !enabled {
            entries.remove(index);
        }
        return save_blair_configuration(&config);
    }
    let entry = startup()
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or("The startup entry is no longer available")?;
    write_startup(entry, enabled)
}
pub fn add_startup(id: &str) -> Result<(), String> {
    let entry = installed()
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or("The application is no longer installed")?;
    if blair_available() {
        command("gio", &["help", "launch"])?;
        let path = entry
            .properties
            .get("path")
            .ok_or("The desktop entry is unavailable")?;
        let launch = desktop_launch_command(path);
        let mut config = blair_configuration()?;
        let table = config.as_table_mut().ok_or("Invalid Blair configuration")?;
        let entries = table
            .entry("autostart")
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or("Invalid startup configuration")?;
        if entries
            .iter()
            .any(|entry| entry.get("command").and_then(toml::Value::as_str) == Some(&launch))
        {
            return Err("This application is already configured for startup".into());
        }
        entries.push(toml::Value::Table(toml::map::Map::from_iter([
            ("command".into(), toml::Value::String(launch)),
            ("restart".into(), toml::Value::Boolean(false)),
        ])));
        return save_blair_configuration(&config);
    }
    write_startup(entry, true)
}
fn desktop_launch_command(path: &str) -> String {
    // gio parses the Desktop Entry Exec field according to its own spec;
    // never reinterpret field codes or executable arguments as shell syntax.
    format!("gio launch '{}'", path.replace('\'', "'\"'\"'"))
}
fn blair_configuration() -> Result<toml::Value, String> {
    let connection = Connection::new_session().map_err(|e| e.to_string())?;
    let (document,): (String,) = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call("org.blair.Compositor1", "Configuration", ())
        .map_err(|e| e.to_string())?;
    toml::from_str(&document).map_err(|e| e.to_string())
}
fn save_blair_configuration(configuration: &toml::Value) -> Result<(), String> {
    let document = toml::to_string_pretty(configuration).map_err(|e| e.to_string())?;
    let connection = Connection::new_session().map_err(|e| e.to_string())?;
    let (saved,): (bool,) = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call("org.blair.Compositor1", "SetConfiguration", (document,))
        .map_err(|e| e.to_string())?;
    if saved {
        Ok(())
    } else {
        Err("Blair rejected the startup configuration".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_parser_ignores_action_sections() {
        let values = desktop_values(
            "[Desktop Entry]\nName=Editor\nExec=editor %F\n[Desktop Action New]\nName=New file\n",
        );
        assert_eq!(values.get("Name").unwrap(), "Editor");
    }
    #[test]
    fn startup_override_preserves_other_sections_and_replaces_hidden() {
        let source = "[Desktop Entry]\nName=Editor\nHidden=false\n[Desktop Action New]\nName=New\n";
        let result = with_hidden(source, true);
        assert!(result.contains("Name=Editor\nHidden=true\n[Desktop Action New]"));
        assert_eq!(result.matches("Hidden=").count(), 1);
    }
    #[test]
    fn application_ids_cannot_escape_the_autostart_directory() {
        for id in [
            "../../victim.desktop",
            "/tmp/victim.desktop",
            "--evil.desktop",
            "evil.desktop\n",
        ] {
            assert!(validate_id(id).is_err(), "{id}");
        }
        assert!(validate_id("org.mozilla.firefox.desktop").is_ok());
    }
}
