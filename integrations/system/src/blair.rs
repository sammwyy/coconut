use super::*;
use std::os::unix::fs::PermissionsExt;

pub fn configuration() -> Result<toml::Value, String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let (document,): (String,) = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call("org.blair.Compositor1", "Configuration", ())
        .map_err(|error| error.to_string())?;
    toml::from_str(&document).map_err(|error| error.to_string())
}

pub fn save(configuration: &toml::Value) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let document = toml::to_string_pretty(configuration).map_err(|error| error.to_string())?;
    let (saved,): (bool,) = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call("org.blair.Compositor1", "SetConfiguration", (document,))
        .map_err(|error| error.to_string())?;
    if saved {
        Ok(())
    } else {
        Err("Blair rejected the configuration".into())
    }
}

fn path(key: &str) -> Option<(&'static str, &'static str)> {
    match key {
        "cursor-theme" => Some(("cursor", "theme")),
        "icon-theme" => Some(("appearance", "icon_theme")),
        "sticky-keys" => Some(("accessibility", "sticky_keys")),
        "visual-alerts" => Some(("accessibility", "visual_alerts")),
        "zoom" => Some(("accessibility", "zoom")),
        "zoom-factor" => Some(("accessibility", "zoom_factor")),
        "lock-after-sleep" => Some(("session", "lock_after_sleep")),
        "idle-dim" => Some(("power", "idle_dim")),
        _ => None,
    }
}

pub fn supports(key: &str) -> bool {
    path(key).is_some() || matches!(key, "contrast" | "text-scale")
}

fn compatible_locker() -> Option<String> {
    let paths = std::env::var_os("PATH")?;
    for name in ["swaylock", "gtklock", "waylock"] {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join(name);
            if candidate.is_file()
                && candidate
                    .metadata()
                    .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
            {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn native_backlight_available() -> bool {
    std::fs::read_dir("/sys/class/backlight")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("brightness"))
        .any(|brightness| {
            brightness.is_file()
                && std::fs::OpenOptions::new()
                    .write(true)
                    .open(brightness)
                    .is_ok()
        })
}

pub fn discover(snapshot: &mut Snapshot) {
    match creamui_theme_loader::read_system_accessibility() {
        Ok(preferences) => {
            snapshot
                .preferences
                .insert("contrast".into(), boolean(preferences.high_contrast, true));
            snapshot.preferences.insert(
                "text-scale".into(),
                number(f64::from(preferences.text_scale), true),
            );
        }
        Err(error) => snapshot.errors.push(error.to_string()),
    }
    match configuration() {
        Ok(config) => {
            for key in [
                "cursor-theme",
                "icon-theme",
                "sticky-keys",
                "visual-alerts",
                "zoom",
                "zoom-factor",
                "lock-after-sleep",
                "idle-dim",
            ] {
                let (section, property) = path(key).unwrap();
                let value = config
                    .get(section)
                    .and_then(|section| section.get(property));
                let (writable, reason) = if key == "lock-after-sleep"
                    && compatible_locker().is_none()
                {
                    (
                        false,
                        "Install swaylock, gtklock, or waylock to enable session locking".into(),
                    )
                } else if key == "idle-dim" && !native_backlight_available() {
                    (
                        false,
                        "No writable kernel backlight is available for Blair to dim".into(),
                    )
                } else {
                    (true, String::new())
                };
                let preference = Preference {
                    value: value.and_then(|value| match value {
                        toml::Value::String(value) => Some(Value::Text(value.clone())),
                        toml::Value::Boolean(value) => Some(Value::Bool(*value)),
                        toml::Value::Float(value) => Some(Value::Number(*value)),
                        _ => None,
                    }),
                    writable,
                    reason,
                    ..Default::default()
                };
                snapshot.preferences.insert(key.into(), preference);
            }
        }
        Err(error) => snapshot
            .errors
            .push(format!("Blair configuration: {error}")),
    }
}

pub fn set(key: &str, value: Value) -> Result<(), String> {
    if matches!(key, "contrast" | "text-scale") {
        let mut preferences =
            creamui_theme_loader::read_system_accessibility().map_err(|error| error.to_string())?;
        match (key, value) {
            ("contrast", Value::Bool(value)) => preferences.high_contrast = value,
            ("text-scale", Value::Number(value))
                if value.is_finite() && (0.75..=2.0).contains(&value) =>
            {
                preferences.text_scale = value as f32
            }
            _ => return Err("Invalid accessibility preference".into()),
        }
        creamui_theme_loader::write_system_accessibility(preferences)
            .map_err(|error| error.to_string())?;
        return creamui_theme_watch::publish().map_err(|error| error.to_string());
    }
    if key == "lock-after-sleep" {
        let Value::Bool(enabled) = value else {
            return Err("Invalid session lock preference".into());
        };
        let mut config = configuration()?;
        if enabled {
            let locker = compatible_locker()
                .ok_or("No compatible Wayland locker was found (swaylock, gtklock, or waylock)")?;
            put(
                &mut config,
                "session",
                "locker",
                toml::Value::String(locker),
            )?;
        }
        put(
            &mut config,
            "session",
            "lock_after_sleep",
            toml::Value::Boolean(enabled),
        )?;
        return save(&config);
    }
    let (section, property) = path(key).ok_or("Unknown Blair preference")?;
    let value = match (key, value) {
        ("cursor-theme" | "icon-theme", Value::Text(value)) if !value.trim().is_empty() => {
            toml::Value::String(value)
        }
        ("sticky-keys" | "visual-alerts" | "zoom" | "idle-dim", Value::Bool(value)) => {
            toml::Value::Boolean(value)
        }
        ("zoom-factor", Value::Number(value))
            if value.is_finite() && (1.0..=8.0).contains(&value) =>
        {
            toml::Value::Float(value)
        }
        _ => return Err("Invalid Blair preference value".into()),
    };
    let mut config = configuration()?;
    put(&mut config, section, property, value)?;
    save(&config)
}

fn put(
    config: &mut toml::Value,
    section: &str,
    property: &str,
    value: toml::Value,
) -> Result<(), String> {
    let root = config.as_table_mut().ok_or("Invalid Blair configuration")?;
    let table = root
        .entry(section)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or("Invalid Blair configuration section")?;
    table.insert(property.into(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_a_theme_preserves_other_configuration_sections() {
        let mut config: toml::Value =
            toml::from_str("[cursor]\nsize = 32\n[window]\nlayout = 'tiling'\n").unwrap();
        put(
            &mut config,
            "cursor",
            "theme",
            toml::Value::String("Adwaita".into()),
        )
        .unwrap();
        put(
            &mut config,
            "appearance",
            "icon_theme",
            toml::Value::String("Papirus".into()),
        )
        .unwrap();
        assert_eq!(config["cursor"]["size"].as_integer(), Some(32));
        assert_eq!(config["window"]["layout"].as_str(), Some("tiling"));
        assert_eq!(config["appearance"]["icon_theme"].as_str(), Some("Papirus"));
    }
}
