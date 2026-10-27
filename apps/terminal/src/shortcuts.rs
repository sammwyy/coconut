use creamui_render::platform::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    NewTab,
    NewWindow,
    CloseTab,
    NextTab,
    PreviousTab,
    SelectTab(usize),
    Copy,
    Paste,
    Search,
    Palette,
    RenameTab,
    SaveOutput,
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    FocusNextPane,
    FocusPreviousPane,
    GrowPane,
    ShrinkPane,
    PreviousCommand,
    NextCommand,
    ToggleBroadcastInput,
    Settings,
    ScrollUp,
    ScrollDown,
}
impl Action {
    pub fn all() -> Vec<Self> {
        use Action::*;
        [NewTab, NewWindow, CloseTab, NextTab, PreviousTab]
            .into_iter()
            .chain((1..=9).map(SelectTab))
            .chain([
                Copy,
                Paste,
                Search,
                Palette,
                RenameTab,
                SaveOutput,
                SplitHorizontal,
                SplitVertical,
                ClosePane,
                FocusNextPane,
                FocusPreviousPane,
                GrowPane,
                ShrinkPane,
                PreviousCommand,
                NextCommand,
                ToggleBroadcastInput,
                Settings,
                ScrollUp,
                ScrollDown,
            ])
            .collect()
    }
    pub fn id(self) -> String {
        match self {
            Self::SelectTab(i) => format!("tab_{i}"),
            a => match a {
                Self::NewTab => "new_tab",
                Self::NewWindow => "new_window",
                Self::CloseTab => "close_tab",
                Self::NextTab => "next_tab",
                Self::PreviousTab => "previous_tab",
                Self::Copy => "copy",
                Self::Paste => "paste",
                Self::Search => "search",
                Self::Palette => "palette",
                Self::RenameTab => "rename_tab",
                Self::SaveOutput => "save_output",
                Self::SplitHorizontal => "split_horizontal",
                Self::SplitVertical => "split_vertical",
                Self::ClosePane => "close_pane",
                Self::FocusNextPane => "focus_next_pane",
                Self::FocusPreviousPane => "focus_previous_pane",
                Self::GrowPane => "grow_pane",
                Self::ShrinkPane => "shrink_pane",
                Self::PreviousCommand => "previous_command",
                Self::NextCommand => "next_command",
                Self::ToggleBroadcastInput => "toggle_broadcast_input",
                Self::Settings => "settings",
                Self::ScrollUp => "scroll_up",
                Self::ScrollDown => "scroll_down",
                Self::SelectTab(_) => unreachable!(),
            }
            .into(),
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::SelectTab(i) => format!("Switch to tab {i}"),
            a => match a {
                Self::NewTab => "New tab",
                Self::NewWindow => "New window",
                Self::CloseTab => "Close tab",
                Self::NextTab => "Next tab",
                Self::PreviousTab => "Previous tab",
                Self::Copy => "Copy selection",
                Self::Paste => "Paste",
                Self::Search => "Search scrollback",
                Self::Palette => "Command palette",
                Self::RenameTab => "Rename tab",
                Self::SaveOutput => "Save terminal output",
                Self::SplitHorizontal => "Split pane horizontally",
                Self::SplitVertical => "Split pane vertically",
                Self::ClosePane => "Close pane",
                Self::FocusNextPane => "Focus next pane",
                Self::FocusPreviousPane => "Focus previous pane",
                Self::GrowPane => "Grow focused pane",
                Self::ShrinkPane => "Shrink focused pane",
                Self::PreviousCommand => "Previous command boundary",
                Self::NextCommand => "Next command boundary",
                Self::ToggleBroadcastInput => "Toggle broadcast input",
                Self::Settings => "Open settings",
                Self::ScrollUp => "Scroll page up",
                Self::ScrollDown => "Scroll page down",
                Self::SelectTab(_) => unreachable!(),
            }
            .into(),
        }
    }
    pub fn default_binding(self) -> String {
        match self {
            Self::SelectTab(i) => format!("Alt+{i}"),
            a => match a {
                Self::NewTab => "Ctrl+Shift+T",
                Self::NewWindow => "Ctrl+Shift+N",
                Self::CloseTab => "Ctrl+Shift+W",
                Self::NextTab => "Ctrl+Tab",
                Self::PreviousTab => "Ctrl+Shift+Tab",
                Self::Copy => "Ctrl+Shift+C",
                Self::Paste => "Ctrl+Shift+V",
                Self::Search => "Ctrl+Shift+F",
                Self::Palette => "Ctrl+Shift+P",
                Self::RenameTab => "Ctrl+Shift+R",
                Self::SaveOutput => "Ctrl+Shift+S",
                Self::SplitHorizontal => "Ctrl+Alt+H",
                Self::SplitVertical => "Ctrl+Alt+V",
                Self::ClosePane => "Ctrl+Alt+W",
                Self::FocusNextPane => "Ctrl+Alt+]",
                Self::FocusPreviousPane => "Ctrl+Alt+[",
                Self::GrowPane => "Ctrl+Alt+Plus",
                Self::ShrinkPane => "Ctrl+Alt+Minus",
                Self::PreviousCommand => "Ctrl+Alt+Up",
                Self::NextCommand => "Ctrl+Alt+Down",
                Self::ToggleBroadcastInput => "Ctrl+Alt+B",
                Self::Settings => "Ctrl+Comma",
                Self::ScrollUp => "Shift+PageUp",
                Self::ScrollDown => "Shift+PageDown",
                Self::SelectTab(_) => unreachable!(),
            }
            .into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key: Key,
    pub modifiers: Modifiers,
}
impl Binding {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for token in text.split('+').map(str::trim) {
            match token.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "super" | "meta" | "logo" => modifiers.logo = true,
                _ => {
                    if key.is_some() {
                        return Err(format!("Expected one key in {text:?}"));
                    }
                    key = Some(parse_key(token).ok_or_else(|| format!("Unknown key {token:?}"))?);
                }
            }
        }
        let binding = Self {
            key: key.ok_or_else(|| "Choose a key as well as modifiers".to_string())?,
            modifiers,
        };
        if !(modifiers.ctrl
            || modifiers.alt
            || modifiers.logo
            || is_function(&binding.key)
            || modifiers.shift
                && matches!(
                    binding.key,
                    Key::PageUp | Key::PageDown | Key::Home | Key::End | Key::Insert | Key::Tab
                ))
        {
            return Err("Use Ctrl, Alt or Super with a key, or a function key".into());
        }
        Ok(binding)
    }
    pub fn from_event(key: &Key, modifiers: Modifiers) -> Option<Self> {
        let key = normalize(key);
        if matches!(key, Key::Other | Key::Back) {
            None
        } else {
            Some(Self { key, modifiers })
        }
    }
    pub fn matches(&self, key: &Key, m: Modifiers) -> bool {
        self.modifiers == m && self.key == normalize(key)
    }
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        let m = self.modifiers;
        if m.ctrl {
            parts.push("Ctrl".into());
        }
        if m.alt {
            parts.push("Alt".into());
        }
        if m.shift {
            parts.push("Shift".into());
        }
        if m.logo {
            parts.push("Super".into());
        }
        parts.push(key_name(&self.key));
        parts.join("+")
    }
}
fn normalize(key: &Key) -> Key {
    match key {
        Key::Character(s) => Key::Character(s.to_lowercase()),
        Key::Space => Key::Character(" ".into()),
        k => k.clone(),
    }
}
fn parse_key(s: &str) -> Option<Key> {
    Some(match s.to_lowercase().as_str() {
        "space" => Key::Character(" ".into()),
        "comma" => Key::Character(",".into()),
        "plus" => Key::Character("+".into()),
        "minus" => Key::Character("-".into()),
        "period" => Key::Character(".".into()),
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "home" => Key::Home,
        "end" => Key::End,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ if s.chars().count() == 1 => Key::Character(s.to_lowercase()),
        _ => return None,
    })
}
fn is_function(key: &Key) -> bool {
    matches!(
        key,
        Key::F1
            | Key::F2
            | Key::F3
            | Key::F4
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12
    )
}
fn key_name(key: &Key) -> String {
    match key {
        Key::Character(s) => match s.as_str() {
            " " => "Space".into(),
            "," => "Comma".into(),
            "+" => "Plus".into(),
            s => s.to_uppercase(),
        },
        k => format!("{k:?}"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_matches_exact_modifiers_and_normalizes_shifted_letters() {
        for a in Action::all() {
            let b = Binding::parse(&a.default_binding()).unwrap();
            assert_eq!(Binding::parse(&b.display()).unwrap(), b);
        }
        let b = Binding::parse("Shift+Control+t").unwrap();
        assert!(b.matches(
            &Key::Character("T".into()),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
        ));
        assert!(!b.matches(
            &Key::Character("t".into()),
            Modifiers {
                ctrl: true,
                ..Default::default()
            }
        ));
        assert!(Binding::parse("t").is_err());
        assert!(Binding::parse("Ctrl+Shift").is_err());
        assert!(Binding::parse("Ctrl+T+N").is_err());
    }
}
