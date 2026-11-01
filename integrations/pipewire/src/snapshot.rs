use coconut_api::volume::{AudioDevice, AudioDirection, AudioStream};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AudioNode {
    pub global_id: u32,
    pub serial: String,
    pub name: String,
    pub description: String,
    pub direction: AudioDirection,
    pub stream: bool,
    pub level: f32,
    pub muted: bool,
    pub active: bool,
}

#[derive(Clone, Default, PartialEq)]
pub(super) struct Snapshot {
    pub loading: bool,
    pub level: f32,
    pub muted: bool,
    pub nodes: Vec<AudioNode>,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn devices(&self) -> Vec<AudioDevice> {
        self.nodes
            .iter()
            .filter(|node| !node.stream)
            .map(|node| AudioDevice {
                id: node.serial.clone(),
                name: node.name.clone(),
                description: node.description.clone(),
                direction: node.direction,
                level: node.level,
                muted: node.muted,
                active: node.active,
            })
            .collect()
    }
    pub fn streams(&self) -> Vec<AudioStream> {
        self.nodes
            .iter()
            .filter(|node| node.stream)
            .map(|node| AudioStream {
                id: node.serial.clone(),
                name: node.name.clone(),
                description: node.description.clone(),
                direction: node.direction,
                level: node.level,
                muted: node.muted,
            })
            .collect()
    }
}

pub(super) fn read() -> Result<Snapshot, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let raw = super::command("pw-dump", &["-N"])?;
    let mut snapshot = parse(&raw)?;
    for node in &mut snapshot.nodes {
        if std::time::Instant::now() >= deadline {
            return Err("Reading audio controls timed out. Try again.".into());
        }
        if let Ok(output) = super::command("wpctl", &["get-volume", &node.global_id.to_string()]) {
            if let Some((level, muted)) = parse_volume(&output) {
                node.level = level;
                node.muted = muted;
            }
        }
    }
    if let Some((level, muted)) = super::volume_output() {
        snapshot.level = level;
        snapshot.muted = muted;
    }
    snapshot.nodes.sort_by(|a, b| {
        a.stream
            .cmp(&b.stream)
            .then(a.name.cmp(&b.name))
            .then(a.serial.cmp(&b.serial))
    });
    Ok(snapshot)
}

pub(super) fn parse_volume(output: &str) -> Option<(f32, bool)> {
    let value = output
        .split_whitespace()
        .find_map(|part| part.parse::<f32>().ok().filter(|value| value.is_finite()))?;
    Some((value.clamp(0.0, 1.0), output.contains("MUTED")))
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| {
        v.as_str()
            .map(str::to_owned)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    })
}

pub(super) fn parse(raw: &str) -> Result<Snapshot, String> {
    let objects: Vec<Value> = serde_json::from_str(raw)
        .map_err(|error| format!("Could not read audio devices: {error}"))?;
    let default = |key: &str| -> Option<String> {
        objects
            .iter()
            .filter_map(|object| object.get("metadata").and_then(Value::as_array))
            .flatten()
            .find_map(|entry| {
                if entry.get("key").and_then(Value::as_str) != Some(key) {
                    return None;
                }
                let value = entry.get("value")?;
                let value = if let Some(raw) = value.as_str() {
                    serde_json::from_str::<Value>(raw).ok()?
                } else {
                    value.clone()
                };
                text(&value, "name")
            })
    };
    let output = default("default.audio.sink");
    let input = default("default.audio.source");
    let mut nodes = Vec::new();
    for object in &objects {
        let props = &object["info"]["props"];
        let (direction, stream) = match props["media.class"].as_str() {
            Some("Audio/Sink") => (AudioDirection::Output, false),
            Some("Audio/Source") => (AudioDirection::Input, false),
            Some("Stream/Output/Audio") => (AudioDirection::Output, true),
            Some("Stream/Input/Audio") => (AudioDirection::Input, true),
            _ => continue,
        };
        if props["node.name"]
            .as_str()
            .is_some_and(|name| name.ends_with(".monitor"))
        {
            continue;
        }
        let Some(id) = object["id"].as_u64().and_then(|id| u32::try_from(id).ok()) else {
            continue;
        };
        let Some(serial) = text(props, "object.serial") else {
            continue;
        };
        let node_name = text(props, "node.name").unwrap_or_else(|| format!("Audio {id}"));
        let name = if stream {
            text(props, "application.name").or_else(|| text(props, "application.process.binary"))
        } else {
            text(props, "node.description").or_else(|| text(props, "node.nick"))
        }
        .unwrap_or_else(|| node_name.clone());
        let description = if stream {
            text(props, "media.name").unwrap_or_default()
        } else {
            text(props, "device.description").unwrap_or_default()
        };
        let active = !stream
            && match direction {
                AudioDirection::Output => output.as_deref(),
                AudioDirection::Input => input.as_deref(),
            } == Some(node_name.as_str());
        nodes.push(AudioNode {
            global_id: id,
            serial,
            name,
            description,
            direction,
            stream,
            level: 0.0,
            muted: false,
            active,
        });
    }
    Ok(Snapshot {
        nodes,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_uses_serial_identity_and_default_metadata() {
        let snapshot = parse(r#"[
            {"id":1,"metadata":[{"key":"default.audio.sink","value":"{\"name\":\"speakers\"}"},{"key":"default.audio.source","value":{"name":"mic"}}]},
            {"id":20,"info":{"props":{"object.serial":900,"media.class":"Audio/Sink","node.name":"speakers","node.description":"Speakers"}}},
            {"id":21,"info":{"props":{"object.serial":901,"media.class":"Audio/Source","node.name":"mic"}}},
            {"id":22,"info":{"props":{"object.serial":902,"media.class":"Stream/Output/Audio","application.name":"Music","media.name":"Track"}}},
            {"id":23,"info":{"props":{"object.serial":903,"media.class":"Audio/Source","node.name":"speakers.monitor"}}}
        ]"#).unwrap();
        assert_eq!(snapshot.devices().len(), 2);
        assert!(snapshot.devices().iter().all(|device| device.active));
        assert_eq!(snapshot.devices()[0].id, "900");
        assert_eq!(snapshot.streams()[0].name, "Music");
        assert_eq!(snapshot.streams()[0].description, "Track");
    }
    #[test]
    fn volume_parser_rejects_non_finite_values_and_preserves_mute() {
        assert_eq!(parse_volume("Volume: 0.42 [MUTED]"), Some((0.42, true)));
        assert_eq!(parse_volume("Volume: NaN"), None);
        assert_eq!(parse_volume("Volume: 1.50"), Some((1.0, false)));
    }

    #[test]
    #[ignore = "requires a running PipeWire session; reads devices without changing them"]
    fn reads_live_inventory_without_changing_audio() {
        let snapshot = read().expect("a live audio inventory");
        assert!(!snapshot.devices().is_empty());
        assert!(snapshot
            .nodes
            .iter()
            .all(|node| !node.serial.is_empty() && node.level.is_finite()));
        assert!(snapshot.devices().iter().any(|device| device.active));
    }
}
