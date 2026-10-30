use super::*;

impl State {
    pub(super) fn catalog(&self) -> Vec<CommandPaletteItem> {
        let mut actions = vec![
            (
                "standard",
                "Standard mode",
                "Alt+1",
                Action::Navigate(Mode::Standard),
            ),
            (
                "scientific",
                "Scientific mode",
                "Alt+2",
                Action::Navigate(Mode::Scientific),
            ),
            ("degrees", "Use degrees", "", Action::Angle(Angle::Degrees)),
            ("radians", "Use radians", "", Action::Angle(Angle::Radians)),
            ("history", "Show or hide history", "Ctrl+H", Action::History),
            ("clear-history", "Clear history", "", Action::ClearHistory),
            ("copy", "Copy result", "Ctrl+C", Action::Copy),
            (
                "drawer",
                "Show or hide navigation",
                "Ctrl+B",
                Action::Drawer,
            ),
            ("swap", "Swap conversion units", "Ctrl+U", Action::SwapUnits),
            ("paste", "Paste expression", "Ctrl+V", Action::Paste),
            ("clear", "Clear expression", "Escape", Action::Clear),
            (
                "store",
                "Store in memory (MS)",
                "Ctrl+M",
                Action::Memory(Memory::Store),
            ),
            ("add", "Add to memory (M+)", "", Action::Memory(Memory::Add)),
            (
                "subtract",
                "Subtract from memory (M−)",
                "",
                Action::Memory(Memory::Subtract),
            ),
            (
                "clear-memory",
                "Clear memory (MC)",
                "Ctrl+Shift+M",
                Action::Memory(Memory::Clear),
            ),
        ];
        if self.session.borrow().memory.is_some() {
            actions.push(("recall", "Recall memory (MR)", "Ctrl+R", Action::Recall));
        }
        for category in Category::ALL {
            actions.push((
                category.slug(),
                category.name(),
                "",
                Action::Navigate(Mode::Converter(category)),
            ));
        }
        let mut items: Vec<_> = actions
            .into_iter()
            .map(|(id, label, key, action)| {
                let state = self.clone();
                CommandPaletteItem::new(id, label, move || state.run(action.clone())).shortcut(key)
            })
            .collect();
        for name in [
            "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh",
            "atanh", "sqrt", "cbrt", "ln", "log", "log2", "exp", "abs", "floor", "ceil", "round",
            "fact", "sum", "mean", "min", "max",
        ] {
            let state = self.clone();
            items.push(
                CommandPaletteItem::new(name, format!("Insert {name}(x)"), move || {
                    state.run(Action::Function(name.into()))
                })
                .category("Functions"),
            );
        }
        items
    }
    pub fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !event.pressed {
            return false;
        }
        if self.palette_open.peek() {
            let key = match event.key {
                Key::Escape => creamui_core::Key::Escape,
                Key::Enter => creamui_core::Key::Enter,
                Key::Up => creamui_core::Key::Up,
                Key::Down => creamui_core::Key::Down,
                Key::Tab => creamui_core::Key::Tab,
                Key::Backspace => creamui_core::Key::Backspace,
                Key::Delete => creamui_core::Key::Delete,
                Key::Left => creamui_core::Key::Left,
                Key::Right => creamui_core::Key::Right,
                Key::Home => creamui_core::Key::Home,
                Key::End => creamui_core::Key::End,
                Key::Character(ref text) if text.chars().count() == 1 => {
                    creamui_core::Key::Char(text.chars().next().unwrap())
                }
                _ => return false,
            };
            return self.palette.handle_key(
                creamui_core::KeyInput {
                    key,
                    modifiers: creamui_core::Modifiers {
                        ctrl: modifiers.ctrl,
                        shift: modifiers.shift,
                        alt: modifiers.alt,
                        logo: modifiers.logo,
                    },
                },
                &self.catalog(),
                || self.run(Action::Dismiss),
            );
        }
        if self.units_open() {
            return false;
        }
        if self.drawer_open.peek() {
            if event.key == Key::Escape {
                self.run(Action::Dismiss);
                return true;
            }
            if event.key == Key::Enter {
                return false;
            }
        }
        let action = match &event.key {
            Key::Escape => {
                if self.history_open.peek() {
                    Action::Dismiss
                } else {
                    Action::Clear
                }
            }
            Key::Enter => Action::Evaluate,
            Key::Backspace => Action::Backspace,
            Key::Delete => Action::Clear,
            Key::Character(text) if modifiers.alt && text == "1" => {
                Action::Navigate(Mode::Standard)
            }
            Key::Character(text) if modifiers.alt && text == "2" => {
                Action::Navigate(Mode::Scientific)
            }
            Key::Character(text) if modifiers.ctrl || modifiers.logo => {
                match text.to_ascii_lowercase().as_str() {
                    "h" => Action::History,
                    "b" => Action::Drawer,
                    "c" => Action::Copy,
                    "v" => Action::Paste,
                    "p" if modifiers.shift => Action::Commands,
                    "l" => Action::Clear,
                    "u" => Action::SwapUnits,
                    "m" if modifiers.shift => Action::Memory(Memory::Clear),
                    "m" => Action::Memory(Memory::Store),
                    "r" => Action::Recall,
                    _ => return false,
                }
            }
            Key::Character(text) if !modifiers.alt => {
                if text == "=" {
                    Action::Evaluate
                } else if text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-−*/×÷^%!().,πτ".contains(c))
                {
                    Action::Insert(text.clone())
                } else {
                    return false;
                }
            }
            Key::F1 => Action::Commands,
            _ => return false,
        };
        self.run(action);
        true
    }
}
