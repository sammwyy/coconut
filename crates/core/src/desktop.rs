use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for DesktopColor {
    fn default() -> Self {
        Self {
            r: 29,
            g: 37,
            b: 48,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopConfig {
    pub icons: DesktopIconsConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum IconShape {
    Square,
    #[default]
    Rounded,
    Circle,
}

/// What a single click on a desktop icon does. Either way, double-clicking
/// always opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ClickAction {
    #[default]
    Select,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopIconsConfig {
    /// Side length of the icon's background square, in pixels.
    pub size: f32,
    /// Extra gap added between grid slots, on top of `size`.
    pub spacing: f32,
    pub background: bool,
    pub background_color: DesktopColor,
    pub shape: IconShape,
    /// Gap between the background edge and the icon glyph. At `0.0` the
    /// glyph fills the background and gets clipped to its shape.
    pub padding: f32,
    pub click: ClickAction,
}

impl Default for DesktopIconsConfig {
    fn default() -> Self {
        Self {
            size: 58.0,
            spacing: 16.0,
            background: true,
            background_color: DesktopColor {
                r: 59,
                g: 126,
                b: 193,
            },
            shape: IconShape::Rounded,
            padding: 12.0,
            click: ClickAction::Select,
        }
    }
}
