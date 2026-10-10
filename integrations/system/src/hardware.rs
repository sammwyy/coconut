use super::*;
use serde_json::Value as Json;

// org.blair.Compositor1.DisplayInfo: physical mode, advertised modes and preview state.
type BlairDisplay = (String, i32, i32, i32, Vec<(i32, i32, i32)>, bool, bool, u32);

fn display_mode_id(width: i32, height: i32, refresh: i32) -> String {
    format!("{width}x{height}@{refresh}")
}

fn display_mode_label(width: i32, height: i32, refresh: i32) -> String {
    format!("{width} × {height} · {:.2} Hz", refresh as f64 / 1000.0)
}

fn accept_blair_displays(snapshot: &mut Snapshot, displays: Vec<BlairDisplay>) -> Vec<Entry> {
    displays
        .into_iter()
        .map(
            |(name, width, height, refresh, modes, writable, pending, seconds)| {
                let mut choices: Vec<_> = modes
                    .into_iter()
                    .map(|(width, height, refresh)| Choice {
                        id: display_mode_id(width, height, refresh),
                        label: display_mode_label(width, height, refresh),
                    })
                    .collect();
                choices.dedup_by(|a, b| a.id == b.id);
                snapshot.preferences.insert(
                    format!("display:{name}"),
                    Preference {
                        value: Some(Value::Text(display_mode_id(width, height, refresh))),
                        writable: writable && !pending,
                        choices,
                        reason: if pending {
                            "Confirm or revert the preview before trying another mode".into()
                        } else if !writable {
                            "This output follows the host window size".into()
                        } else {
                            String::new()
                        },
                    },
                );
                Entry {
                    id: name.clone(),
                    name,
                    description: display_mode_label(width, height, refresh),
                    enabled: true,
                    properties: [
                        ("pending-confirmation".into(), pending.to_string()),
                        ("confirmation-seconds".into(), seconds.to_string()),
                    ]
                    .into(),
                }
            },
        )
        .collect()
}

pub fn display_action(method: &str, output: &str) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let _: () = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call("org.blair.Compositor1", method, (output,))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn parse_display_mode(mode: &str) -> Result<(i32, i32, i32), String> {
    let parsed = mode
        .split_once('@')
        .and_then(|(size, refresh)| {
            let (width, height) = size.split_once('x')?;
            Some((
                width.parse::<i32>().ok()?,
                height.parse::<i32>().ok()?,
                refresh.parse::<i32>().ok()?,
            ))
        })
        .filter(|(width, height, refresh)| *width > 0 && *height > 0 && *refresh > 0);
    parsed.ok_or_else(|| "Choose a supported display mode".into())
}

pub fn preview_display_mode(output: &str, mode: &str) -> Result<(), String> {
    let (width, height, refresh) = parse_display_mode(mode)?;
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let _: () = connection
        .with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT)
        .method_call(
            "org.blair.Compositor1",
            "ApplyDisplayMode",
            (output, width, height, refresh),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn discover_displays(snapshot: &mut Snapshot) {
    let displays = if snapshot.session.to_ascii_lowercase().contains("kde")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some())
    {
        command("kscreen-doctor", &["-j"])
            .ok()
            .and_then(|output| serde_json::from_str::<Json>(&output).ok())
    } else {
        None
    };
    let mut outputs = Vec::new();
    if snapshot.session == "Blair" {
        if let Ok(connection) = Connection::new_session() {
            let proxy =
                connection.with_proxy("org.blair.Compositor", "/org/blair/Compositor", TIMEOUT);
            let info: Result<(Vec<BlairDisplay>,), _> =
                proxy.method_call("org.blair.Compositor1", "DisplayInfo", ());
            if let Ok((info,)) = info {
                outputs = accept_blair_displays(snapshot, info);
            }
            let names: Result<(Vec<String>,), _> =
                proxy.method_call("org.blair.Compositor1", "Outputs", ());
            if let Ok((names,)) = names {
                for name in names
                    .into_iter()
                    .filter(|name| !outputs.iter().any(|output| &output.id == name))
                    .collect::<Vec<_>>()
                {
                    let area: Result<(i32, i32, i32, i32), _> =
                        proxy.method_call("org.blair.Compositor1", "WorkArea", (name.clone(),));
                    let description = area
                        .ok()
                        .map(|(_, _, width, height)| {
                            format!("Usable area {width} × {height} · Blair")
                        })
                        .unwrap_or_else(|| "Connected · Blair".into());
                    outputs.push(Entry {
                        id: name.clone(),
                        name,
                        description,
                        enabled: true,
                        ..Default::default()
                    });
                }
            }
        }
    }
    if let Some(json) = displays {
        for output in json["outputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|o| o["connected"].as_bool() == Some(true))
        {
            let id = output["id"].to_string();
            let name = output["name"].as_str().unwrap_or("Display").to_owned();
            let current = output["currentModeId"].as_str().unwrap_or_default();
            let choices: Vec<_> = output["modes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|mode| {
                    Some(Choice {
                        id: mode["id"].as_str()?.into(),
                        label: format!(
                            "{} × {} · {:.2} Hz",
                            mode["size"]["width"],
                            mode["size"]["height"],
                            mode["refreshRate"].as_f64().unwrap_or(0.0)
                        ),
                    })
                })
                .collect();
            let description = choices
                .iter()
                .find(|choice| choice.id == current)
                .map(|choice| choice.label.clone())
                .unwrap_or_else(|| "Connected · disabled".into());
            snapshot.preferences.insert(
                format!("display:{id}"),
                Preference {
                    value: Some(Value::Text(current.into())),
                    writable: false,
                    choices,
                    reason: "Safe mode-change confirmation is not yet supported".into(),
                },
            );
            outputs.push(Entry {
                id,
                name,
                description,
                enabled: output["enabled"].as_bool() == Some(true),
                ..Default::default()
            });
        }
    }
    if outputs.is_empty() {
        if let Ok(connectors) = fs::read_dir("/sys/class/drm") {
            for connector in connectors.flatten() {
                if read(connector.path().join("status")) != "connected" {
                    continue;
                }
                let modes = read(connector.path().join("modes"));
                outputs.push(Entry {
                    id: connector.file_name().to_string_lossy().into(),
                    name: connector.file_name().to_string_lossy().into(),
                    description: format!(
                        "Connected · supported modes: {}",
                        modes.lines().take(3).collect::<Vec<_>>().join(", ")
                    ),
                    enabled: true,
                    ..Default::default()
                });
            }
        }
    }
    snapshot.collections.insert("displays".into(), outputs);
}

pub fn discover(snapshot: &mut Snapshot) {
    discover_displays(snapshot);
    let mut usb = Vec::new();
    if let Ok(devices) = fs::read_dir("/sys/bus/usb/devices") {
        for device in devices.flatten() {
            let name = read(device.path().join("product"));
            if name.is_empty() {
                continue;
            }
            usb.push(Entry {
                id: device.file_name().to_string_lossy().into(),
                name,
                description: format!(
                    "{} · USB {}:{}",
                    read(device.path().join("manufacturer")),
                    read(device.path().join("idVendor")),
                    read(device.path().join("idProduct"))
                ),
                enabled: true,
                ..Default::default()
            });
        }
    }
    snapshot.collections.insert("devices".into(), usb);
    let inputs = fs::read_dir("/sys/class/input")
        .map(|devices| {
            devices
                .flatten()
                .filter(|device| device.file_name().to_string_lossy().starts_with("event"))
                .map(|device| Entry {
                    id: device.file_name().to_string_lossy().into(),
                    name: read(device.path().join("device/name")),
                    description: read(device.path().join("device/phys")),
                    enabled: true,
                    ..Default::default()
                })
                .collect()
        })
        .unwrap_or_default();
    snapshot.collections.insert("input-devices".into(), inputs);
    match command("lpstat", &["-p"]) {
        Ok(printers) => {
            snapshot
                .facts
                .insert("printer-service".into(), "CUPS".into());
            snapshot.collections.insert(
                "printers".into(),
                printers
                    .lines()
                    .filter_map(|line| {
                        let remainder = line.strip_prefix("printer ")?;
                        let (name, state) = remainder.split_once(' ')?;
                        Some(Entry {
                            id: name.into(),
                            name: name.into(),
                            description: state.into(),
                            enabled: true,
                            ..Default::default()
                        })
                    })
                    .collect(),
            );
        }
        Err(error) => {
            snapshot.facts.insert("printer-service".into(), error);
        }
    }
    audio(snapshot);
    snapshot.preferences.insert(
        "brightness".into(),
        command("brightnessctl", &["-m"])
            .ok()
            .and_then(|output| {
                output
                    .split(',')
                    .nth(3)
                    .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
            })
            .map(|value| number(value / 100.0, true))
            .unwrap_or_else(|| unavailable("No controllable backlight is available")),
    );
    if let Ok(connection) = Connection::new_system() {
        let proxy = connection.with_proxy(
            "net.hadess.PowerProfiles",
            "/net/hadess/PowerProfiles",
            TIMEOUT,
        );
        let current: Result<String, _> = proxy.get("net.hadess.PowerProfiles", "ActiveProfile");
        let profiles: Result<Vec<dbus::arg::PropMap>, _> =
            proxy.get("net.hadess.PowerProfiles", "Profiles");
        if let (Ok(current), Ok(profiles)) = (current, profiles) {
            let choices = profiles
                .iter()
                .filter_map(|profile| profile.get("Profile").and_then(|v| v.0.as_str()))
                .map(|id| Choice {
                    id: id.into(),
                    label: id.replace('-', " "),
                })
                .collect();
            snapshot.preferences.insert(
                "power-mode".into(),
                Preference {
                    value: Some(Value::Text(current)),
                    writable: true,
                    choices,
                    ..Default::default()
                },
            );
        }
    }
    snapshot
        .preferences
        .entry("power-mode".into())
        .or_insert_with(|| unavailable("power-profiles-daemon is not available"));
}

fn audio(snapshot: &mut Snapshot) {
    let nodes = command("pw-dump", &[])
        .ok()
        .and_then(|output| serde_json::from_str::<Json>(&output).ok());
    if let Some(nodes) = nodes {
        for (collection, class, key, target) in [
            (
                "sound-outputs",
                "Audio/Sink",
                "sound-output",
                "@DEFAULT_AUDIO_SINK@",
            ),
            (
                "sound-inputs",
                "Audio/Source",
                "sound-input",
                "@DEFAULT_AUDIO_SOURCE@",
            ),
        ] {
            let current = command("wpctl", &["inspect", target])
                .ok()
                .and_then(|output| {
                    output
                        .lines()
                        .next()
                        .and_then(|line| line.strip_prefix("id "))
                        .and_then(|s| s.split(',').next())
                        .map(str::to_owned)
                });
            let entries: Vec<_> = nodes
                .as_array()
                .into_iter()
                .flatten()
                .filter(|node| node["info"]["props"]["media.class"].as_str() == Some(class))
                .filter_map(|node| {
                    let props = &node["info"]["props"];
                    let id = node["id"].as_u64()?.to_string();
                    let name = props["node.description"]
                        .as_str()
                        .or(props["node.nick"].as_str())
                        .or(props["node.name"].as_str())
                        .unwrap_or("Audio device")
                        .to_owned();
                    Some(Entry {
                        enabled: current.as_ref() == Some(&id),
                        id,
                        name,
                        description: props["node.name"].as_str().unwrap_or_default().into(),
                        ..Default::default()
                    })
                })
                .collect();
            snapshot.preferences.insert(
                key.into(),
                Preference {
                    value: current.map(Value::Text),
                    writable: !entries.is_empty(),
                    choices: entries
                        .iter()
                        .map(|e| Choice {
                            id: e.id.clone(),
                            label: e.name.clone(),
                        })
                        .collect(),
                    reason: "No default device is configured".into(),
                },
            );
            snapshot.collections.insert(collection.into(), entries);
        }
    }
    if let Ok(volume) = command("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
        if let Some(level) = volume
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<f64>().ok())
        {
            snapshot
                .preferences
                .insert("volume".into(), number(level, true));
        }
    }
    if let Ok(volume) = command("wpctl", &["get-volume", "@DEFAULT_AUDIO_SOURCE@"]) {
        snapshot.preferences.insert(
            "microphone-muted".into(),
            boolean(volume.contains("[MUTED]"), true),
        );
    }
    for key in ["volume", "sound-output", "sound-input", "microphone-muted"] {
        snapshot.preferences.entry(key.into()).or_insert_with(|| {
            unavailable("PipeWire/WirePlumber or an audio device is unavailable")
        });
    }
}

pub fn set_preference(key: &str, value: Value) -> Result<(), String> {
    match (key, value) {
        ("volume", Value::Number(value)) if value.is_finite() && (0.0..=1.0).contains(&value) => {
            command(
                "wpctl",
                &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{value:.3}")],
            )
            .map(|_| ())
        }
        ("brightness", Value::Number(value))
            if value.is_finite() && (0.0..=1.0).contains(&value) =>
        {
            command("brightnessctl", &["set", &format!("{:.0}%", value * 100.0)]).map(|_| ())
        }
        ("microphone-muted", Value::Bool(value)) => command(
            "wpctl",
            &[
                "set-mute",
                "@DEFAULT_AUDIO_SOURCE@",
                if value { "1" } else { "0" },
            ],
        )
        .map(|_| ()),
        ("sound-output" | "sound-input", Value::Text(id)) => {
            let _: u64 = id.parse().map_err(|_| "Invalid audio device identifier")?;
            let mut snapshot = Snapshot::default();
            audio(&mut snapshot);
            if !snapshot
                .preferences
                .get(key)
                .is_some_and(|pref| pref.choices.iter().any(|choice| choice.id == id))
            {
                return Err("The audio device is no longer connected".into());
            }
            command("wpctl", &["set-default", &id]).map(|_| ())
        }
        ("power-mode", Value::Text(profile)) => {
            if !["balanced", "performance", "power-saver"].contains(&profile.as_str()) {
                return Err("Invalid power profile".into());
            }
            let connection = Connection::new_system().map_err(|e| e.to_string())?;
            connection
                .with_proxy(
                    "net.hadess.PowerProfiles",
                    "/net/hadess/PowerProfiles",
                    TIMEOUT,
                )
                .set("net.hadess.PowerProfiles", "ActiveProfile", profile)
                .map_err(|e| e.to_string())
        }
        _ => Err("Invalid hardware preference".into()),
    }
}
pub fn add_printer(name: &str, uri: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("Use letters, digits, hyphens or underscores for the printer name".into());
    }
    if !uri.starts_with("ipp://") && !uri.starts_with("ipps://") {
        return Err("Use an ipp:// or ipps:// printer address".into());
    }
    command(
        "lpadmin",
        &["-p", name, "-E", "-v", uri, "-m", "everywhere"],
    )
    .map(|_| ())
}

#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn blair_displays_report_physical_mode_and_writable_choices() {
        let mut snapshot = Snapshot::default();
        let displays = accept_blair_displays(
            &mut snapshot,
            vec![(
                "Virtual-1".into(),
                1366,
                768,
                75000,
                vec![(1366, 768, 75000), (1920, 1080, 60000)],
                true,
                false,
                0,
            )],
        );
        assert_eq!(displays[0].description, "1366 × 768 · 75.00 Hz");
        let pref = &snapshot.preferences["display:Virtual-1"];
        assert!(pref.writable);
        assert_eq!(pref.value, Some(Value::Text("1366x768@75000".into())));
        assert_eq!(
            parse_display_mode(&pref.choices[1].id).unwrap(),
            (1920, 1080, 60000)
        );
    }

    #[test]
    fn preview_and_nested_modes_are_read_only() {
        for (writable, pending) in [(true, true), (false, false)] {
            let mut snapshot = Snapshot::default();
            let displays = accept_blair_displays(
                &mut snapshot,
                vec![(
                    "Virtual-1".into(),
                    1920,
                    1080,
                    60000,
                    vec![(1920, 1080, 60000)],
                    writable,
                    pending,
                    if pending { 12 } else { 0 },
                )],
            );
            assert!(!snapshot.preferences["display:Virtual-1"].writable);
            assert_eq!(
                displays[0].properties["pending-confirmation"],
                pending.to_string()
            );
        }
    }

    #[test]
    fn malformed_modes_are_rejected_before_dbus() {
        for mode in [
            "",
            "1920x1080",
            "1920x1080@0",
            "0x1080@60000",
            "1920x-1@60000",
            "1920x1080@NaN",
        ] {
            assert!(parse_display_mode(mode).is_err(), "{mode}");
        }
    }
}
