use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupEdge {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PopupConfig {
    pub enabled: bool,
    pub edge: PopupEdge,
    pub align: PopupAlign,
    pub offset: f32,
    pub duration_ms: u32,
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            edge: PopupEdge::Bottom,
            align: PopupAlign::Center,
            offset: 120.0,
            duration_ms: 1800,
        }
    }
}

impl PopupConfig {
    pub fn notifications() -> Self {
        Self {
            edge: PopupEdge::Top,
            align: PopupAlign::Right,
            offset: 64.0,
            duration_ms: 5000,
            ..Self::default()
        }
    }
}

pub(crate) fn deserialize_notifications<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<PopupConfig, D::Error> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Overrides {
        enabled: Option<bool>,
        edge: Option<PopupEdge>,
        align: Option<PopupAlign>,
        offset: Option<f32>,
        duration_ms: Option<u32>,
    }
    let overrides = Overrides::deserialize(deserializer)?;
    let defaults = PopupConfig::notifications();
    Ok(PopupConfig {
        enabled: overrides.enabled.unwrap_or(defaults.enabled),
        edge: overrides.edge.unwrap_or(defaults.edge),
        align: overrides.align.unwrap_or(defaults.align),
        offset: overrides.offset.unwrap_or(defaults.offset),
        duration_ms: overrides.duration_ms.unwrap_or(defaults.duration_ms),
    })
}
