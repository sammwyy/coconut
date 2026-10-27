//! Configurable application shortcuts shared by the browser and Settings.

use creamui_render::platform::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Search,
}

impl Action {
    pub(crate) const fn id(self) -> &'static str {
        "search"
    }
    pub(crate) const fn label(self) -> &'static str {
        "Search files"
    }
    pub(crate) const fn default_binding(self) -> &'static str {
        "Ctrl+Shift+F"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) key: Key,
    pub(crate) modifiers: Modifiers,
}

impl Binding {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for token in text.split('+').map(str::trim) {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "super" | "meta" | "logo" => modifiers.logo = true,
                value if value.chars().count() == 1 => {
                    if key.is_some() {
                        return Err("Choose one key.".into());
                    }
                    key = Some(Key::Character(value.into()));
                }
                _ => return Err(format!("Unknown key {token:?}")),
            }
        }
        if !(modifiers.ctrl || modifiers.alt || modifiers.logo) {
            return Err("Use Ctrl, Alt or Super with a key.".into());
        }
        Ok(Self {
            key: key.ok_or_else(|| "Choose a key as well as modifiers.".to_string())?,
            modifiers,
        })
    }

    pub(crate) fn from_event(key: &Key, modifiers: Modifiers) -> Option<Self> {
        let Key::Character(value) = key else {
            return None;
        };
        if !(modifiers.ctrl || modifiers.alt || modifiers.logo) || value.chars().count() != 1 {
            return None;
        }
        Some(Self {
            key: Key::Character(value.to_ascii_lowercase()),
            modifiers,
        })
    }

    pub(crate) fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        self.modifiers == modifiers
            && matches!((&self.key, key), (Key::Character(left), Key::Character(right)) if left.eq_ignore_ascii_case(right))
    }

    pub(crate) fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.logo {
            parts.push("Super".to_string());
        }
        if let Key::Character(value) = &self.key {
            parts.push(value.to_ascii_uppercase());
        }
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_binding_matches_case_insensitively() {
        let binding = Binding::parse("Ctrl+Shift+F").unwrap();
        assert!(binding.matches(
            &Key::Character("f".into()),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
        ));
        assert_eq!(binding.display(), "Ctrl+Shift+F");
    }
}
