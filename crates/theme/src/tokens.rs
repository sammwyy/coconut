use crate::{Color, Scheme};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tokens {
    pub colors: Colors,
    pub typography: Typography,
    pub geometry: Geometry,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub custom: std::collections::BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub surface: Color,
    pub surface_elevated: Color,
    pub surface_hover: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub on_accent: Color,
    pub selection_background: Color,
    pub selection_text: Color,
    pub border: Color,
    pub border_strong: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typography {
    pub family: String,
    pub caption_size: f32,
    pub body_size: f32,
    pub section_size: f32,
    pub title_size: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub radius_small: f32,
    pub radius_medium: f32,
    pub radius_large: f32,
    pub control_radius: f32,
    pub input_radius: f32,
    pub card_radius: f32,
    pub menu_radius: f32,
    pub menu_item_radius: f32,
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
    pub border_width: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellTokens {
    pub dock_background: Color,
    pub island_background: Color,
    pub dock_opacity: f32,
    pub island_opacity: f32,
    pub dock_radius: f32,
    pub island_radius: f32,
    pub blur: bool,
}

impl Tokens {
    pub fn validate(&self) -> Result<(), String> {
        let value = toml::Value::try_from(self).map_err(|e| e.to_string())?;
        for section in ["geometry", "typography"] {
            for (name, value) in value[section].as_table().unwrap() {
                if name == "family" {
                    continue;
                }
                let number = value
                    .as_float()
                    .or_else(|| value.as_integer().map(|v| v as f64))
                    .unwrap_or(f64::NAN);
                if !number.is_finite()
                    || !(0.0..=512.0).contains(&number)
                    || (section == "typography" && number == 0.0)
                {
                    return Err(format!(
                        "tokens.{section}.{name} is outside its valid range"
                    ));
                }
            }
        }
        if self.typography.family.trim().is_empty()
            || self.typography.family.chars().any(char::is_control)
        {
            return Err("invalid font family".into());
        }
        Ok(())
    }

    pub(crate) fn customize(
        &mut self,
        overrides: &crate::Overrides,
        accessibility: &crate::Accessibility,
        scheme: Scheme,
    ) {
        if let Some(accent) = overrides.accent {
            self.colors.accent = accent;
            self.colors.accent_hover = accent.mix(Color::rgb(255, 255, 255), 0.15);
            self.colors.accent_pressed = accent.mix(Color::rgb(0, 0, 0), 0.15);
            self.colors.on_accent = accent.foreground();
            self.colors.selection_background.r = accent.r;
            self.colors.selection_background.g = accent.g;
            self.colors.selection_background.b = accent.b;
            self.colors.selection_text = if self.colors.selection_background.a == 255 {
                accent.foreground()
            } else {
                self.colors.text_primary
            };
        }
        if let Some(family) = &overrides.font_family {
            self.typography.family = family.clone();
        }
        if let Some(corners) = overrides.corners {
            let factor = match corners {
                crate::Corners::Square => 0.,
                crate::Corners::Soft => 0.5,
                crate::Corners::Round => 1.,
            };
            for radius in [
                &mut self.geometry.radius_small,
                &mut self.geometry.radius_medium,
                &mut self.geometry.radius_large,
                &mut self.geometry.control_radius,
                &mut self.geometry.input_radius,
                &mut self.geometry.card_radius,
                &mut self.geometry.menu_radius,
                &mut self.geometry.menu_item_radius,
            ] {
                *radius = (*radius * factor).round();
            }
        }
        for size in [
            &mut self.typography.caption_size,
            &mut self.typography.body_size,
            &mut self.typography.section_size,
            &mut self.typography.title_size,
        ] {
            *size *= accessibility.text_scale;
        }
        if accessibility.high_contrast {
            let (background, foreground, accent) = match scheme {
                Scheme::Dark => (
                    Color::rgb(0, 0, 0),
                    Color::rgb(255, 255, 255),
                    Color::rgb(255, 255, 0),
                ),
                Scheme::Light => (
                    Color::rgb(255, 255, 255),
                    Color::rgb(0, 0, 0),
                    Color::rgb(0, 0, 128),
                ),
            };
            let c = &mut self.colors;
            c.surface = background;
            c.surface_elevated = background;
            c.surface_hover = background.mix(foreground, 0.15);
            c.text_primary = foreground;
            c.text_secondary = foreground;
            c.text_disabled = background.mix(foreground, 0.65);
            c.border = foreground;
            c.border_strong = foreground;
            c.accent = accent;
            c.accent_hover = accent;
            c.accent_pressed = accent;
            c.on_accent = background;
            c.selection_background = accent;
            c.selection_text = background;
            self.geometry.border_width = self.geometry.border_width.max(2.0);
        }
    }
}

impl ShellTokens {
    pub fn validate(&self) -> Result<(), String> {
        if [self.dock_opacity, self.island_opacity]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || [self.dock_radius, self.island_radius]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=512.0).contains(v))
        {
            return Err("invalid shell opacity or radius".into());
        }
        Ok(())
    }
}
