use super::*;

impl State {
    pub(super) fn dismiss(&self) {
        self.popup.set(Popup::None);
        self.panel.set(Panel::None);
        self.digits.borrow_mut().clear();
        self.editor_focused.set(true);
        self.refresh();
        if let Some(window) = self.window.borrow().as_ref() {
            window.focus_text_input();
        }
    }
    pub(super) fn go_to(&self) {
        let value = self.prompt.peek();
        let (_, tab) = self.current();
        let mut e = tab.borrow_mut();
        let target = if e.document.view.bytes() {
            usize::from_str_radix(
                value.trim_start_matches("0x"),
                if value.starts_with("0x") { 16 } else { 10 },
            )
            .ok()
            .filter(|n| *n <= e.document.text.len_chars())
        } else {
            let mut parts = value.split(':');
            let line = parts.next().and_then(|n| n.parse::<usize>().ok());
            let column = parts
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(1);
            line.filter(|line| *line > 0 && *line <= e.document.text.len_lines())
                .map(|line| {
                    (e.document.text.line_to_char(line - 1) + column.saturating_sub(1))
                        .min(e.document.line_end(line - 1))
                })
        };
        if let Some(target) = target {
            e.move_to(target, false);
            drop(e);
            self.dismiss();
        } else {
            drop(e);
            self.error("Enter a valid line[:column] or byte offset (decimal / 0xHEX).".into());
        }
    }
    pub fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !event.pressed {
            return false;
        }
        if self.confirming.peek().is_some() {
            match &event.key {
                Key::Escape => self.resolve_close("cancel"),
                Key::Enter => self.resolve_close("save"),
                Key::Character(key) if modifiers.alt && key.eq_ignore_ascii_case("d") => {
                    self.resolve_close("discard")
                }
                Key::Character(key) if modifiers.alt && key.eq_ignore_ascii_case("s") => {
                    self.resolve_close("save")
                }
                Key::Tab => return false,
                _ => {}
            }
            return true;
        }
        if event.key == Key::Escape {
            if self.popup.peek() != Popup::None {
                self.popup.set(Popup::None);
                self.refresh();
                return true;
            }
            self.dismiss();
            return true;
        }
        if self.popup_key(&event.key) {
            return true;
        }
        let editing = self.popup.peek() == Popup::None
            && self.editor_focused.get()
            && !matches!(
                self.panel.peek(),
                Panel::Palette | Panel::GoTo | Panel::Help
            );
        if let Some(command) = commands::shortcut(&event.key, modifiers, editing) {
            if !editing
                && matches!(
                    command,
                    Command::Undo
                        | Command::Redo
                        | Command::Copy
                        | Command::Cut
                        | Command::Paste
                        | Command::SelectAll
                )
            {
                return false;
            }
            self.run(command);
            return true;
        }
        if !editing {
            return false;
        }
        if modifiers.alt || modifiers.logo {
            return false;
        }
        let shift = modifiers.shift;
        let ctrl = modifiers.ctrl;
        match event.key {
            Key::Left | Key::Right => self.edit(|e| {
                e.horizontal(event.key == Key::Right, ctrl, shift);
                Ok(())
            }),
            Key::Up | Key::Down => self.edit(|e| {
                e.vertical(if event.key == Key::Up { -1 } else { 1 }, shift);
                Ok(())
            }),
            Key::Home | Key::End => self.edit(|e| {
                e.home_end(event.key == Key::End, ctrl, shift);
                Ok(())
            }),
            Key::PageUp | Key::PageDown => self.edit(|e| {
                e.vertical(
                    e.rows as isize * if event.key == Key::PageUp { -1 } else { 1 },
                    shift,
                );
                Ok(())
            }),
            Key::Backspace | Key::Delete => {
                self.edit(|e| e.delete(event.key == Key::Backspace, ctrl))
            }
            Key::Enter => self.edit(Editor::newline),
            Key::Space if !ctrl => {
                if !self.current().1.borrow().document.view.bytes() {
                    self.edit(|e| e.type_text(" "));
                }
            }
            Key::Character(text) if !ctrl => {
                if self.current().1.borrow().document.view.bytes() {
                    self.byte_input(&text);
                } else {
                    self.edit(|e| e.type_text(&text));
                }
            }
            _ => return false,
        }
        true
    }
    pub(super) fn byte_input(&self, text: &str) {
        let mode = self.current().1.borrow().document.view;
        let radix = if mode == ViewMode::Hex { 16 } else { 2 };
        let width = if radix == 16 { 2 } else { 8 };
        for c in text.chars() {
            if c.is_whitespace() {
                continue;
            }
            if c.to_digit(radix).is_none() {
                self.error(format!(
                    "Enter {} digits, or paste space-separated byte values.",
                    if radix == 16 { "hexadecimal" } else { "binary" }
                ));
                return;
            }
            let value = {
                let mut digits = self.digits.borrow_mut();
                digits.push(c);
                if digits.len() == width {
                    let value = u8::from_str_radix(&digits, radix).unwrap();
                    digits.clear();
                    Some(value)
                } else {
                    None
                }
            };
            if let Some(value) = value {
                self.edit(|e| {
                    let mut range = e.selection();
                    if range.is_empty() {
                        range.end = (range.start + 1).min(e.document.text.len_chars());
                    }
                    e.replace(range, &(value as char).to_string())
                });
            }
        }
        self.refresh();
    }
}
