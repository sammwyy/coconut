use super::*;
use crate::document::Encoding;
use creamui_core::{
    layout::{LengthPercentage, LengthPercentageAuto, Position},
    Rect, StateStyle, Widget,
};
use creamui_widgets::RawButton;

struct PopupEntry {
    label: String,
    shortcut: &'static str,
    active: bool,
    action: Rc<dyn Fn()>,
}
impl State {
    pub(super) fn toggle_popup(&self, popup: Popup) {
        self.popup.set(if self.popup.peek() == popup {
            Popup::None
        } else {
            popup
        });
        self.popup_index.set(0);
        self.refresh();
    }
    pub(super) fn popup_key(&self, key: &Key) -> bool {
        let popup = self.popup.peek();
        if popup == Popup::None {
            return false;
        }
        let entries = self.popup_entries(popup);
        let count = entries.len();
        match key {
            Key::Up => self
                .popup_index
                .update(|index| *index = (*index + count - 1) % count),
            Key::Down => self
                .popup_index
                .update(|index| *index = (*index + 1) % count),
            Key::Enter => {
                if let Some(entry) = entries.get(self.popup_index.peek()) {
                    (entry.action)();
                }
            }
            _ => return false,
        }
        self.refresh();
        true
    }
    pub(super) fn header(&self, width: f32) -> BoxedWidget {
        let state = self.clone();
        let menu = anchored(
            Box::new(
                RawButton::new(
                    Style {
                        justify_content: Some(layout::JustifyContent::Center),
                        ..fixed(32.0, 30.0)
                    },
                    move || state.toggle_popup(Popup::Menu),
                )
                .corner_radius(6.0)
                .hover_style(StateStyle::new().background(use_theme().colors.surface_hover))
                .focus_style(StateStyle::new().outline(use_theme().colors.accent, 2.0))
                .child(menu_icon()),
            ),
            self.menu_anchor.clone(),
        );
        let add = self.clone();
        let plus = button("+", 30.0, false, move || add.run(Command::New));
        let titlebar = AppTitlebar::new(width)
            .height(42.0)
            .padding(8.0)
            .center_margin(80.0);
        let leading = titlebar.controls_lead();
        let (start, end) = titlebar.reserved();
        let available = (width - start.max(8.0) - end.max(8.0)).max(1.0);
        let workspace = self.workspace.borrow();
        if workspace.tabs.len() == 1 {
            let title = workspace.tabs[0].1.borrow().document.title();
            let capacity = ((width - 2.0 * (start.max(end) + 80.0)) / 7.0).max(1.0) as usize;
            return Box::new(
                titlebar
                    .start_boxed(view(row(2.0), vec![menu, plus]))
                    .center(Text::new(truncate_title(&title, capacity)).font_size(13.0)),
            );
        }
        let capacity = ((available - 104.0) / 120.0).floor().max(1.0) as usize;
        let paged = workspace.tabs.len() > capacity;
        let first = workspace.active / capacity * capacity;
        let last = (first + capacity).min(workspace.tabs.len());
        let tab_width = ((available - 70.0 - if paged { 56.0 } else { 0.0 })
            / (last - first) as f32)
            .clamp(70.0, 180.0);
        let mut tabs = Vec::new();
        if !leading {
            tabs.push(menu);
        }
        if paged {
            let state = self.clone();
            tabs.push(button("‹", 26.0, false, move || {
                state.run(Command::PreviousTab)
            }));
        }
        for (index, (id, tab)) in workspace.tabs.iter().enumerate().take(last).skip(first) {
            let doc = &tab.borrow().document;
            let name = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into());
            let name = format!(
                "{}{}",
                truncate_title(&name, ((tab_width - 34.0) / 7.0).max(1.0) as usize),
                if doc.dirty() { " *" } else { "" }
            );
            let choose = self.clone();
            let close = self.clone();
            let id = *id;
            tabs.push(Box::new(
                RawView::new(fixed(tab_width, 30.0))
                    .background(if index == workspace.active {
                        use_theme().colors.surface_hover
                    } else {
                        CLEAR
                    })
                    .corner_radius(6.0)
                    .with_children(vec![
                        button(
                            &name,
                            tab_width - 26.0,
                            index == workspace.active,
                            move || choose.activate(index),
                        ),
                        button("×", 26.0, false, move || close.close_tab(id)),
                    ]),
            ) as BoxedWidget);
        }
        if paged {
            let state = self.clone();
            tabs.push(button("›", 26.0, false, move || {
                state.run(Command::NextTab)
            }));
        }
        tabs.push(plus);
        if leading {
            tabs.push(view(
                Style {
                    flex_grow: 1.0,
                    ..row(0.0)
                },
                vec![],
            ));
            let state = self.clone();
            tabs.push(anchored(
                Box::new(
                    RawButton::new(
                        Style {
                            justify_content: Some(layout::JustifyContent::Center),
                            ..fixed(32.0, 30.0)
                        },
                        move || state.toggle_popup(Popup::Menu),
                    )
                    .corner_radius(6.0)
                    .child(menu_icon()),
                ),
                self.menu_anchor.clone(),
            ));
        }
        Box::new(titlebar.body_boxed(view(
            Style {
                size: layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                ..row(3.0)
            },
            tabs,
        )))
    }
    pub(super) fn statusbar(&self, width: f32) -> BoxedWidget {
        let (_, tab) = self.current();
        let editor = tab.borrow();
        let position = if width >= 900.0 {
            format!(
                "Ln {}, Col {}   {} lines   {} chars / {} bytes",
                editor.line() + 1,
                editor.column() + 1,
                editor.line_count(),
                editor.document.text.len_chars(),
                editor.document.byte_len()
            )
        } else {
            format!(
                "Ln {}, Col {}   {}L · {}C",
                editor.line() + 1,
                editor.column() + 1,
                editor.line_count(),
                editor.document.text.len_chars()
            )
        };
        let message = self.message.get();
        let text = if self.busy.get() > 0 {
            "Working…".to_string()
        } else if !self.digits.borrow().is_empty() {
            format!("Byte: {}…", self.digits.borrow())
        } else if message != "Ready"
            && message != "Saved"
            && !message.ends_with(" matches")
            && !message.starts_with("Replaced ")
        {
            truncate_title(&message, if width < 900.0 { 24 } else { 48 })
        } else {
            position
        };
        let goto = self.clone();
        let mut items: Vec<BoxedWidget> = vec![Box::new(
            RawButton::new(
                pad(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        size: layout::Size {
                            width: Dimension::Auto,
                            height: Dimension::Length(30.0),
                        },
                        min_size: layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Length(0.0),
                        },
                        justify_content: Some(layout::JustifyContent::Start),
                        ..row(0.0)
                    },
                    4.0,
                    0.0,
                ),
                move || goto.run(Command::GoTo),
            )
            .corner_radius(4.0)
            .hover_style(StateStyle::new().background(use_theme().colors.surface_hover))
            .child(Box::new(
                Text::new(text)
                    .font_size(12.0)
                    .color(use_theme().colors.text_secondary)
                    .text_align(creamui_core::TextAlign::Start),
            )),
        )];
        let bytes = editor.document.view.bytes();
        let render_label = if bytes {
            "Bytes"
        } else if editor.document.language == Language::Markdown {
            "Markdown"
        } else {
            "Plain"
        };
        let state = self.clone();
        items.push(anchored(
            button(&format!("{render_label} ▾"), 94.0, false, move || {
                state.toggle_popup(Popup::Renderer)
            }),
            self.render_anchor.clone(),
        ));
        if bytes || editor.document.language == Language::Markdown {
            let mode = if bytes {
                editor.document.view.label()
            } else {
                editor.document.markdown.label()
            };
            let state = self.clone();
            items.push(anchored(
                button(&format!("{mode} ▾"), 82.0, false, move || {
                    state.toggle_popup(Popup::Mode)
                }),
                self.mode_anchor.clone(),
            ));
        } else {
            items.push(view(fixed(82.0, 30.0), vec![]));
        }
        if !bytes {
            let encoding = editor.document.encoding.label();
            let state = self.clone();
            items.push(anchored(
                button(&format!("{encoding} ▾"), 104.0, false, move || {
                    state.toggle_popup(Popup::Encoding)
                }),
                self.encoding_anchor.clone(),
            ));
            let state = self.clone();
            items.push(button(
                if editor.document.newline == "\r\n" {
                    "CRLF"
                } else {
                    "LF"
                },
                44.0,
                false,
                move || state.run(Command::LineEnding),
            ));
        } else {
            items.push(view(fixed(104.0, 30.0), vec![]));
            items.push(view(fixed(44.0, 30.0), vec![]));
        }
        if width >= 900.0 {
            if !bytes {
                let state = self.clone();
                items.push(button(
                    if editor.insert_spaces {
                        "Spaces: 4"
                    } else {
                        "Tabs: 4"
                    },
                    74.0,
                    false,
                    move || state.run(Command::Spaces),
                ));
            } else {
                items.push(view(fixed(74.0, 30.0), vec![]));
            }
        }
        view(
            pad(
                Style {
                    flex_shrink: 0.0,
                    ..row(2.0)
                },
                6.0,
                1.0,
            ),
            items,
        )
    }
    fn popup_entries(&self, popup: Popup) -> Vec<PopupEntry> {
        let mut entries = Vec::new();
        let commands: Vec<Command> = match popup {
            Popup::Menu => vec![
                Command::New,
                Command::Open,
                Command::Save,
                Command::SaveAs,
                Command::Find,
                Command::Replace,
                Command::GoTo,
                Command::Palette,
                Command::Help,
                Command::LineNumbers,
                Command::Close,
                Command::Quit,
            ],
            Popup::Renderer => vec![Command::PlainText, Command::Markdown, Command::Bytes],
            Popup::Mode => {
                if self.current().1.borrow().document.view.bytes() {
                    vec![Command::Hex, Command::Binary]
                } else {
                    vec![Command::Raw, Command::Hybrid, Command::Preview]
                }
            }
            Popup::Encoding => vec![Command::Utf8, Command::Ascii],
            Popup::None => Vec::new(),
        };
        let (_, tab) = self.current();
        let editor = tab.borrow();
        for command in commands {
            let entry = commands::ENTRIES
                .iter()
                .find(|entry| entry.command == command)
                .unwrap();
            let active = match command {
                Command::Markdown => {
                    editor.document.language == Language::Markdown && !editor.document.view.bytes()
                }
                Command::Bytes => editor.document.view.bytes(),
                Command::PlainText => {
                    editor.document.language == Language::PlainText && !editor.document.view.bytes()
                }
                Command::Raw => {
                    editor.document.language == Language::Markdown
                        && !editor.document.view.bytes()
                        && editor.document.markdown == MarkdownMode::Raw
                }
                Command::Preview => {
                    editor.document.language == Language::Markdown
                        && !editor.document.view.bytes()
                        && editor.document.markdown == MarkdownMode::Preview
                }
                Command::Hybrid => {
                    editor.document.language == Language::Markdown
                        && !editor.document.view.bytes()
                        && editor.document.markdown == MarkdownMode::Hybrid
                }
                Command::Utf8 => {
                    editor.document.encoding == Encoding::Utf8 && !editor.document.view.bytes()
                }
                Command::Ascii => editor.document.view == ViewMode::Ascii,
                Command::Hex => editor.document.view == ViewMode::Hex,
                Command::Binary => editor.document.view == ViewMode::Binary,
                _ => false,
            };
            let state = self.clone();
            entries.push(PopupEntry {
                label: match command {
                    Command::PlainText => "Plain".into(),
                    Command::Markdown => "Markdown".into(),
                    Command::Bytes => "Bytes".into(),
                    Command::Raw if popup == Popup::Mode => "Raw".into(),
                    Command::Hybrid if popup == Popup::Mode => "Hybrid".into(),
                    Command::Preview if popup == Popup::Mode => "Preview".into(),
                    Command::Hex => "HEX".into(),
                    Command::Binary => "Binary".into(),
                    _ => entry
                        .label
                        .replace(
                            "Markdown: ",
                            if popup == Popup::Mode {
                                ""
                            } else {
                                "Markdown · "
                            },
                        )
                        .replace("Render: ", "")
                        .replace("View: ", "")
                        .replace("Encoding: ", ""),
                },
                shortcut: entry.binding,
                active,
                action: Rc::new(move || state.run(command)),
            });
        }
        if popup == Popup::Encoding {
            for encoding in [Encoding::Utf8Bom, Encoding::Utf16Le, Encoding::Utf16Be] {
                let state = self.clone();
                entries.push(PopupEntry {
                    label: format!("Encoding: {}", encoding.label()),
                    shortcut: "",
                    active: editor.document.encoding == encoding,
                    action: Rc::new(move || {
                        state.popup.set(Popup::None);
                        state.set_encoding(encoding);
                    }),
                });
            }
        }
        entries
    }
    pub(super) fn popup_layer(&self, size: Size) -> Option<BoxedWidget> {
        let popup = self.popup.get();
        if popup == Popup::None {
            return None;
        }
        let entries = self.popup_entries(popup);
        let width = 290.0_f32.min(size.width - 16.0);
        let height = (entries.len() as f32 * 30.0 + 12.0).min(size.height - 60.0);
        let anchor = match popup {
            Popup::Menu => self.menu_anchor.get(),
            Popup::Renderer => self.render_anchor.get(),
            Popup::Mode => self.mode_anchor.get(),
            _ => self.encoding_anchor.get(),
        };
        let x = anchor.x.clamp(8.0, (size.width - width - 8.0).max(8.0));
        let y = if popup == Popup::Menu {
            if anchor.height > 0.0 {
                anchor.y + anchor.height + 6.0
            } else {
                42.0
            }
        } else {
            (if anchor.height > 0.0 {
                anchor.y
            } else {
                size.height - 32.0
            }) - height
                - 6.0
        };
        let mut rows = Vec::new();
        let selected = self.popup_index.get();
        for (index, entry) in entries.into_iter().enumerate() {
            let action = entry.action;
            let text = if entry.active {
                format!("✓ {}", entry.label)
            } else {
                entry.label
            };
            rows.push(Box::new(
                RawButton::new(pad(fixed(width - 12.0, 30.0), 8.0, 0.0), move || action())
                    .background(if index == selected {
                        use_theme().colors.surface_hover
                    } else {
                        CLEAR
                    })
                    .corner_radius(4.0)
                    .hover_style(StateStyle::new().background(use_theme().colors.surface_hover))
                    .with_children(vec![
                        Box::new(Text::new(text).font_size(12.0)),
                        view(
                            Style {
                                flex_grow: 1.0,
                                ..row(0.0)
                            },
                            vec![],
                        ),
                        Box::new(
                            Text::new(entry.shortcut)
                                .font_size(11.0)
                                .color(use_theme().colors.text_secondary),
                        ),
                    ]),
            ) as BoxedWidget);
        }
        let panel = Box::new(
            Popover::new(Style {
                position: Position::Absolute,
                inset: layout::Rect {
                    left: LengthPercentageAuto::Length(x),
                    top: LengthPercentageAuto::Length(y.max(8.0)),
                    right: LengthPercentageAuto::Auto,
                    bottom: LengthPercentageAuto::Auto,
                },
                size: layout::Size {
                    width: Dimension::Length(width),
                    height: Dimension::Length(height),
                },
                padding: layout::Rect {
                    left: LengthPercentage::Length(6.0),
                    right: LengthPercentage::Length(6.0),
                    top: LengthPercentage::Length(6.0),
                    bottom: LengthPercentage::Length(6.0),
                },
                ..column(0.0)
            })
            .with_children(rows),
        );
        let close = self.clone();
        Some(Box::new(PopupLayer {
            child: Some(panel),
            dismiss: Rc::new(move || {
                close.popup.set(Popup::None);
                close.refresh();
            }),
        }))
    }
    pub(super) fn set_render(&self, language: Language, mode: MarkdownMode) {
        let (id, tab) = self.current();
        if !tab.borrow().document.view.bytes() {
            let mut editor = tab.borrow_mut();
            editor.document.language = language;
            editor.document.markdown = mode;
            editor.reveal();
            drop(editor);
            self.refresh();
            return;
        }
        let mut document = tab.borrow().document.clone();
        let state = self.clone();
        self.busy.update(|busy| *busy += 1);
        self.app.spawn_background(
            move || {
                document.set_view(ViewMode::Utf8)?;
                document.language = language;
                document.markdown = mode;
                Ok::<_, String>(document)
            },
            move |result| {
                state.busy.update(|busy| *busy -= 1);
                match result {
                    Ok(document) => {
                        if let Some(tab) = state.tab(id) {
                            let mut editor = tab.borrow_mut();
                            editor.document = document;
                            editor.move_to(0, false);
                        }
                        state.refresh();
                        state.focus_editor();
                    }
                    Err(error) => state.error(error),
                }
            },
        );
    }
    pub(super) fn set_encoding(&self, encoding: Encoding) {
        if self.busy.peek() > 0 {
            return;
        }
        let (id, tab) = self.current();
        let mut document = tab.borrow().document.clone();
        if document.encoding == encoding && !document.view.bytes() {
            self.refresh();
            self.focus_editor();
            return;
        }
        let state = self.clone();
        self.busy.update(|n| *n += 1);
        self.app.spawn_background(
            move || {
                if document.view.bytes() {
                    document.set_view(ViewMode::Utf8)?;
                }
                document.encoding = encoding;
                document.view = ViewMode::Utf8;
                document.touch();
                Ok::<_, String>(document)
            },
            move |result| {
                state.busy.update(|n| *n -= 1);
                match result {
                    Ok(document) => {
                        if let Some(tab) = state.tab(id) {
                            let mut e = tab.borrow_mut();
                            e.checkpoint();
                            e.document = document;
                            e.move_to(0, false);
                        }
                        state.refresh();
                        state.focus_editor();
                    }
                    Err(error) => state.error(error),
                }
            },
        );
    }
}
struct PopupLayer {
    child: Option<BoxedWidget>,
    dismiss: Rc<dyn Fn()>,
}
impl Widget for PopupLayer {
    fn style(&self) -> creamui_core::Style {
        Style {
            position: Position::Absolute,
            inset: layout::Rect {
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                top: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Length(0.0),
            },
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, _: &mut dyn creamui_core::Painter, _: Rect) {}
    fn is_modal(&self) -> bool {
        true
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.dismiss.clone())
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }
}
fn truncate_title(text: &str, capacity: usize) -> String {
    if text.chars().count() <= capacity {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    }
}
