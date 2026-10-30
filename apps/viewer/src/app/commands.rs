use super::*;
use creamui_core::{Key as UiKey, KeyInput};
use creamui_render::platform::{Key, KeyEvent, Modifiers};
use creamui_widgets::CommandPaletteItem;
impl State {
    pub(super) fn catalog(&self) -> Vec<CommandPaletteItem> {
        let loaded = self.model.borrow().asset.is_some();
        let mut commands = vec![("open", "Open images", "Ctrl+O", Action::Open)];
        if loaded {
            commands.extend([
                ("fit", "Fit image", "0", Action::Fit),
                ("actual", "Actual size", "1", Action::ActualSize),
                ("zoom-in", "Zoom in", "+", Action::ZoomIn),
                ("zoom-out", "Zoom out", "−", Action::ZoomOut),
                ("rotate", "Rotate clockwise", "R", Action::Rotate(true)),
                (
                    "rotate-left",
                    "Rotate counterclockwise",
                    "Shift+R",
                    Action::Rotate(false),
                ),
                (
                    "adjust",
                    "Adjust image",
                    "E",
                    Action::Panel(Panel::Adjustments),
                ),
                (
                    "information",
                    "Image information",
                    "I",
                    Action::Panel(Panel::Information),
                ),
                ("grayscale", "Black and white", "G", Action::Grayscale),
                ("invert", "Invert colors", "N", Action::Invert),
                (
                    "reset-adjustments",
                    "Reset adjustments",
                    "",
                    Action::ResetAdjustments,
                ),
                (
                    "reset-view",
                    "Reset view and adjustments",
                    "Ctrl+0",
                    Action::ResetView,
                ),
                ("close", "Close image", "Ctrl+W", Action::Close),
            ]);
        }
        let model = self.model.borrow();
        if model.index > 0 {
            commands.push(("previous", "Previous image", "Left", Action::Previous));
        }
        if model.index + 1 < model.paths.len() {
            commands.push(("next", "Next image", "Right", Action::Next));
        }
        drop(model);
        commands
            .into_iter()
            .map(|(id, label, shortcut, action)| {
                let state = self.clone();
                CommandPaletteItem::new(id, label, move || state.run(action)).shortcut(shortcut)
            })
            .collect()
    }
    pub fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !event.pressed {
            return false;
        }
        if self.palette_open.peek() {
            let key = match &event.key {
                Key::Escape => UiKey::Escape,
                Key::Enter => UiKey::Enter,
                Key::Up => UiKey::Up,
                Key::Down => UiKey::Down,
                Key::Left => UiKey::Left,
                Key::Right => UiKey::Right,
                Key::Home => UiKey::Home,
                Key::End => UiKey::End,
                Key::Tab => UiKey::Tab,
                Key::Backspace => UiKey::Backspace,
                Key::Delete => UiKey::Delete,
                Key::Space => UiKey::Char(' '),
                Key::Character(text) if text.chars().count() == 1 => {
                    UiKey::Char(text.chars().next().unwrap())
                }
                _ => return false,
            };
            return self.palette.handle_key(
                KeyInput {
                    key,
                    modifiers: creamui_core::Modifiers {
                        ctrl: modifiers.ctrl,
                        alt: modifiers.alt,
                        shift: modifiers.shift,
                        logo: modifiers.logo,
                    },
                },
                &self.catalog(),
                || self.run(Action::Dismiss),
            );
        }
        let modal = {
            let model = self.model.borrow();
            model.asset.is_some() && model.error.is_some()
        };
        if modal {
            if matches!(event.key, Key::Escape | Key::Enter) {
                self.run(Action::Dismiss);
                return true;
            }
            return false;
        }
        let command = modifiers.ctrl || modifiers.logo;
        let action = match &event.key {
            Key::Escape => Action::Dismiss,
            Key::F1 => Action::Commands,
            Key::Character(text) if command => match text.to_ascii_lowercase().as_str() {
                "o" => Action::Open,
                "w" => Action::Close,
                "0" => Action::ResetView,
                "p" if modifiers.shift => Action::Commands,
                _ => return false,
            },
            Key::Character(text) if !modifiers.alt => match text.to_ascii_lowercase().as_str() {
                "+" | "=" => Action::ZoomIn,
                "-" | "−" => Action::ZoomOut,
                "0" => Action::Fit,
                "1" => Action::ActualSize,
                "r" => Action::Rotate(!modifiers.shift),
                "e" => Action::Panel(Panel::Adjustments),
                "i" => Action::Panel(Panel::Information),
                "g" => Action::Grayscale,
                "n" => Action::Invert,
                "w" | "a" | "s" | "d" => {
                    let delta = match text.to_ascii_lowercase().as_str() {
                        "w" => Point { x: 0.0, y: 64.0 },
                        "s" => Point { x: 0.0, y: -64.0 },
                        "a" => Point { x: 64.0, y: 0.0 },
                        _ => Point { x: -64.0, y: 0.0 },
                    };
                    self.edit_view(|view, viewport, image| {
                        let pan = view.layout(viewport, image).pan;
                        view.set_pan(
                            Point {
                                x: pan.x + delta.x,
                                y: pan.y + delta.y,
                            },
                            viewport,
                            image,
                        );
                    });
                    return true;
                }
                _ => return false,
            },
            Key::Left | Key::Right if self.panel.peek() == Panel::Adjustments => return false,
            Key::Left | Key::PageUp => Action::Previous,
            Key::Right | Key::PageDown => Action::Next,
            Key::Home => Action::First,
            Key::End => Action::Last,
            _ => return false,
        };
        self.run(action);
        true
    }
}
