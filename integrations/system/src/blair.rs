use super::*;

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
        _ => None,
    }
}

pub fn supports(key: &str) -> bool {
    path(key).is_some()
}

pub fn discover(snapshot: &mut Snapshot) {
    match configuration() {
        Ok(config) => {
            for key in ["cursor-theme", "icon-theme"] {
                let (section, property) = path(key).unwrap();
                let value = config
                    .get(section)
                    .and_then(|section| section.get(property))
                    .and_then(toml::Value::as_str);
                let preference = Preference {
                    value: value.map(|value| Value::Text(value.into())),
                    writable: true,
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
    let (section, property) = path(key).ok_or("Unknown Blair preference")?;
    let Value::Text(value) = value else {
        return Err("Choose a theme name".into());
    };
    if value.trim().is_empty() {
        return Err("Theme name cannot be empty".into());
    }
    let mut config = configuration()?;
    put(&mut config, section, property, toml::Value::String(value))?;
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
