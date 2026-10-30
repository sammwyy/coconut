use super::*;

impl State {
    pub(super) fn view_mode(&self, mode: ViewMode) {
        let (id, tab) = self.current();
        let mut document = tab.borrow().document.clone();
        let state = self.clone();
        self.busy.update(|n| *n += 1);
        self.app.spawn_background(
            move || {
                document.set_view(mode)?;
                Ok::<_, String>(document)
            },
            move |result| {
                state.busy.update(|n| *n -= 1);
                match result {
                    Ok(document) => {
                        if let Some(tab) = state.tab(id) {
                            let mut editor = tab.borrow_mut();
                            if editor.document.revision != document.revision {
                                editor.checkpoint();
                            }
                            editor.document = document;
                            editor.move_to(0, false);
                            editor.top = 0;
                            editor.left = 0;
                        }
                        state.digits.borrow_mut().clear();
                        state.search(state.query.peek());
                        state.refresh();
                        state.focus_editor();
                    }
                    Err(error) => state.error(error),
                }
            },
        );
    }
    pub(super) fn clipboard(&self, write: Option<String>) -> Result<String, String> {
        let mut clipboard = self.clipboard.borrow_mut();
        if clipboard.is_none() {
            *clipboard =
                Some(arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?);
        }
        let clipboard = clipboard.as_mut().unwrap();
        if let Some(text) = write {
            clipboard.set_text(text).map_err(|e| e.to_string())?;
            Ok(String::new())
        } else {
            clipboard.get_text().map_err(|e| e.to_string())
        }
    }
    pub fn run(&self, command: Command) {
        use Command::*;
        self.popup.set(Popup::None);
        let (_, tab) = self.current();
        if self.busy.peek() > 0
            && !matches!(
                command,
                Help | Palette | Find | GoTo | NextTab | PreviousTab | Open
            )
        {
            return;
        }
        self.digits.borrow_mut().clear();
        match command {
            Menu => self.toggle_popup(Popup::Menu),
            New => self.add(Document::default(), false),
            Open => self.choose_open(),
            Save | SaveAs => self.save(self.current().0, command == SaveAs, false),
            Close => self.close_tab(self.current().0),
            Quit => {
                if self.request_quit() {
                    if let Some(window) = self.window.borrow().as_ref() {
                        window.close();
                    }
                }
            }
            NextTab | PreviousTab => {
                let (index, len) = {
                    let w = self.workspace.borrow();
                    (w.active, w.tabs.len())
                };
                self.activate(if command == NextTab {
                    (index + 1) % len
                } else {
                    (index + len - 1) % len
                });
            }
            Undo => self.edit(|e| {
                e.undo();
                Ok(())
            }),
            Redo => self.edit(|e| {
                e.redo();
                Ok(())
            }),
            Copy | Cut => {
                let mut editor = tab.borrow_mut();
                if editor.selection().is_empty() && !editor.document.view.bytes() {
                    let range = editor.selected_lines();
                    editor.select(range);
                }
                let text = if editor.document.view.bytes() {
                    editor
                        .document
                        .text
                        .slice(editor.selection())
                        .chars()
                        .map(|c| {
                            if editor.document.view == ViewMode::Hex {
                                format!("{:02X}", c as u8)
                            } else {
                                format!("{:08b}", c as u8)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    editor.selected_text()
                };
                drop(editor);
                match self.clipboard(Some(text)) {
                    Ok(_) => {
                        if command == Cut {
                            self.edit(|e| e.insert(""));
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
            Paste => match self.clipboard(None) {
                Ok(text) => {
                    if tab.borrow().document.view.bytes() {
                        let radix = if tab.borrow().document.view == ViewMode::Hex {
                            16
                        } else {
                            2
                        };
                        match document::parse_bytes(&text, radix) {
                            Ok(text) => self.edit(|e| e.insert(&text)),
                            Err(error) => self.error(error),
                        }
                    } else {
                        self.edit(|e| e.insert(&text));
                    }
                }
                Err(error) => self.error(error),
            },
            SelectAll => {
                let mut editor = tab.borrow_mut();
                let len = editor.document.text.len_chars();
                editor.select(0..len);
            }
            Find | Replace | GoTo | Palette | Help => {
                self.panel.set(match command {
                    Find => Panel::Find,
                    Replace => Panel::Replace,
                    GoTo => Panel::GoTo,
                    Palette => Panel::Palette,
                    _ => Panel::Help,
                });
                if matches!(command, Palette | Help | GoTo) {
                    self.prompt.set_value("");
                }
                self.editor_focused.set(false);
                self.refresh();
                if let Some(window) = self.window.borrow().as_ref() {
                    window.focus_text_input();
                }
            }
            NextMatch | PreviousMatch => self.next_match(command == PreviousMatch),
            ReplaceOne | ReplaceAll => self.replace_matches(command == ReplaceAll),
            Indent | Outdent => self.edit(|e| e.indent(command == Outdent)),
            DuplicateLine => self.edit(Editor::duplicate_line),
            DeleteLine => self.edit(Editor::delete_line),
            MoveLineUp | MoveLineDown => self.edit(|e| e.move_line(command == MoveLineDown)),
            Bold | Italic | InlineCode => self.edit(|e| {
                e.wrap_selection(match command {
                    Bold => "**",
                    Italic => "*",
                    _ => "`",
                })
            }),
            Markdown => {
                let mode = tab.borrow().document.markdown;
                self.set_render(Language::Markdown, mode);
            }
            Bytes => {
                let mode = tab.borrow().document.view;
                if !mode.bytes() {
                    self.view_mode(ViewMode::Hex);
                }
            }
            PlainText => self.set_render(Language::PlainText, MarkdownMode::Raw),
            Raw | Preview | Hybrid => self.set_render(
                Language::Markdown,
                match command {
                    Raw => MarkdownMode::Raw,
                    Preview => MarkdownMode::Preview,
                    _ => MarkdownMode::Hybrid,
                },
            ),
            Utf8 => self.set_encoding(document::Encoding::Utf8),
            Ascii | Hex | Binary => self.view_mode(match command {
                Ascii => ViewMode::Ascii,
                Hex => ViewMode::Hex,
                _ => ViewMode::Binary,
            }),
            ZoomIn | ZoomOut | ResetZoom => {
                let mut e = tab.borrow_mut();
                e.font_size = match command {
                    ZoomIn => (e.font_size + 1.0).min(36.0),
                    ZoomOut => (e.font_size - 1.0).max(9.0),
                    _ => 15.0,
                };
            }
            LineNumbers => {
                let mut e = tab.borrow_mut();
                e.line_numbers = !e.line_numbers;
            }
            Overwrite => {
                let mut e = tab.borrow_mut();
                e.overwrite = !e.overwrite;
            }
            Spaces => {
                let mut e = tab.borrow_mut();
                e.insert_spaces = !e.insert_spaces;
            }
            LineEnding => self.convert_newlines(),
            MatchCase | WholeWord | Regex => {
                self.options.update(|o| match command {
                    MatchCase => o.case_sensitive = !o.case_sensitive,
                    WholeWord => o.whole_word = !o.whole_word,
                    _ => o.regex = !o.regex,
                });
                self.search(self.query.peek());
            }
        }
        self.refresh();
        if self.popup.peek() == Popup::None && self.panel.peek() == Panel::None {
            self.focus_editor();
        }
    }
    pub(super) fn convert_newlines(&self) {
        let (id, tab) = self.current();
        if tab.borrow().document.view.bytes() || !tab.borrow().editable() {
            return;
        }
        let source = tab.borrow().document.text.clone();
        let newline = if tab.borrow().document.newline == "\n" {
            "\r\n"
        } else {
            "\n"
        };
        let state = self.clone();
        self.busy.update(|n| *n += 1);
        self.app.spawn_background(
            move || {
                let mut builder = ropey::RopeBuilder::new();
                let mut chars = source.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                        builder.append(newline);
                    } else if c == '\n' {
                        builder.append(newline);
                    } else {
                        builder.append(c.encode_utf8(&mut [0; 4]));
                    }
                }
                builder.finish()
            },
            move |text| {
                state.busy.update(|n| *n -= 1);
                if let Some(tab) = state.tab(id) {
                    let mut e = tab.borrow_mut();
                    e.checkpoint();
                    e.document.text = text;
                    e.document.newline = newline;
                    e.document.touch();
                    e.move_to(0, false);
                }
                state.search(state.query.peek());
                state.refresh();
            },
        );
    }
}
