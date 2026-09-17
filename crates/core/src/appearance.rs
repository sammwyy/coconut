use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub icon_theme: String,
    pub sound_theme: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            icon_theme: "hicolor".into(),
            sound_theme: "freedesktop".into(),
        }
    }
}
