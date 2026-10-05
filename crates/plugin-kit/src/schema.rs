//! Declarative per-island config fields for [`Island::config_schema`],
//! auto-rendered by Settings instead of a hand-rolled page per plugin id.

use crate::config::ConfigValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberPresentation {
    Slider,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum FieldKind {
    Toggle,
    Number {
        range: NumberRange,
        presentation: NumberPresentation,
        /// Appended after the displayed value, e.g. `"%"` or `" px"`.
        suffix: &'static str,
    },
}

/// `key` matches what [`crate::IslandRenderContext::config`] and
/// `modules/<island id>.toml` use for this same setting.
#[derive(Debug, Clone)]
pub struct ConfigField {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub default: ConfigValue,
}

impl ConfigField {
    pub fn toggle(key: &'static str, label: &'static str, default: bool) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Toggle,
            default: ConfigValue::Bool(default),
        }
    }

    pub fn slider(
        key: &'static str,
        label: &'static str,
        default: f64,
        range: NumberRange,
        suffix: &'static str,
    ) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Number {
                range,
                presentation: NumberPresentation::Slider,
                suffix,
            },
            default: ConfigValue::Float(default),
        }
    }

    pub fn number_input(
        key: &'static str,
        label: &'static str,
        default: f64,
        range: NumberRange,
        suffix: &'static str,
    ) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Number {
                range,
                presentation: NumberPresentation::Input,
                suffix,
            },
            default: ConfigValue::Float(default),
        }
    }
}
