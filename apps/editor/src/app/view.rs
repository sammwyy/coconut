use super::*;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, Dialog, DialogContent, DialogFooter,
    DialogHeader, DialogMessage, DialogPlacement, ModalActions,
};

impl State {
    pub fn build(&self, size: Size) -> BoxedWidget {
        let _ = self.revision.get();
        let (id, tab) = self.current();
        let mut children = vec![self.header(size.width)];
        let panel = self.panel.get();
        if matches!(panel, Panel::Find | Panel::Replace) {
            children.push(self.search_bar(panel == Panel::Replace));
        }
        children.push(Box::new(EditorScreen {
            drag_axis: Rc::new(Cell::new(0)),
            revision: self.revision.clone(),
            focused: self.editor_focused.clone(),
            matches: self.matches.clone(),
            on_change: {
                let state = self.clone();
                Rc::new(move || state.refresh())
            },
            tab,
            id,
        }));
        children.push(self.statusbar(size.width));
        let mut root = RawView::new(fixed(size.width, size.height))
            .background(use_theme().colors.surface)
            .child(view(
                Style {
                    size: layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Percent(1.0),
                    },
                    ..column(0.0)
                },
                children,
            ));
        if matches!(panel, Panel::Palette | Panel::Help | Panel::GoTo) {
            root = root.child(self.dialog(panel, size));
        }
        if let Some(popup) = self.popup_layer(size) {
            root = root.child(popup);
        }
        if let Some(id) = self.confirming.get() {
            root = root.child(self.unsaved_dialog(id));
        }
        Box::new(root)
    }
    fn unsaved_dialog(&self, id: u64) -> BoxedWidget {
        let name = self
            .tab(id)
            .and_then(|tab| {
                tab.borrow()
                    .document
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Untitled".into());
        let save = self.clone();
        let discard = self.clone();
        let cancel = self.clone();
        let dismiss = self.clone();
        Box::new(
            Dialog::custom(move || dismiss.resolve_close("cancel"))
                .width(400.0)
                .header(Box::new(DialogHeader::new("Save changes?").compact()))
                .content(Box::new(DialogContent::new().compact().child(Box::new(
                    DialogMessage::new(format!("Your changes to {name} will be lost.")),
                ))))
                .footer(Box::new(
                    DialogFooter::new().compact().child(Box::new(
                        ModalActions::new()
                            .child(Button::styled(
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                                "Cancel",
                                ButtonState::Normal,
                                move || cancel.resolve_close("cancel"),
                            ))
                            .child(Button::styled(
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                                "Don't Save",
                                ButtonState::Normal,
                                move || discard.resolve_close("discard"),
                            ))
                            .child(Button::styled(
                                ButtonVariant::Primary,
                                ButtonSize::Sm,
                                "Save",
                                ButtonState::Normal,
                                move || save.resolve_close("save"),
                            )),
                    )),
                )),
        )
    }
    pub(super) fn search_bar(&self, replace: bool) -> BoxedWidget {
        let state = self.clone();
        let options = self.options.get();
        let mode = self.current().1.borrow().document.view;
        let mut items = vec![Box::new(
            TextInput::controlled_with_style(
                Style {
                    flex_grow: 1.0,
                    ..fixed(220.0, 32.0)
                },
                &self.query,
            )
            .placeholder(match mode {
                ViewMode::Hex => "Find bytes: FF 00 A1",
                ViewMode::Binary => "Find bytes: 11111111 00000000",
                _ => "Find text or regular expression",
            })
            .on_submit(move || state.next_match(false)),
        ) as BoxedWidget];
        let action = |label: &str, width: f32, active: bool, command: Command| {
            let state = self.clone();
            button(label, width, active, move || state.run(command))
        };
        if !mode.bytes() {
            items.extend([
                action("Aa", 34.0, options.case_sensitive, Command::MatchCase),
                action("Word", 48.0, options.whole_word, Command::WholeWord),
                action(".*", 34.0, options.regex, Command::Regex),
            ]);
        }
        items.extend([
            action("↑", 30.0, false, Command::PreviousMatch),
            action("↓", 30.0, false, Command::NextMatch),
        ]);
        let state = self.clone();
        items.push(button("×", 28.0, false, move || state.dismiss()));
        let mut rows = vec![view(row(4.0), items)];
        if replace {
            let state = self.clone();
            rows.push(view(
                row(5.0),
                vec![
                    Box::new(
                        TextInput::controlled_with_style(
                            Style {
                                flex_grow: 1.0,
                                ..fixed(220.0, 32.0)
                            },
                            &self.replacement,
                        )
                        .placeholder(if mode.bytes() {
                            "Replacement byte values"
                        } else {
                            "Replace with ($1 for regex groups)"
                        })
                        .on_submit(move || state.replace_matches(false)),
                    ),
                    action("Replace", 76.0, false, Command::ReplaceOne),
                    action("Replace all", 96.0, false, Command::ReplaceAll),
                ],
            ));
        }
        view(pad(column(4.0), 12.0, 6.0), rows)
    }
    fn palette_items(&self) -> Vec<creamui_widgets::CommandPaletteItem> {
        let mut entries: Vec<_> = commands::ENTRIES.iter().collect();
        entries.sort_by_key(|entry| match entry.command.category() {
            "File" => 0,
            "Edit" => 1,
            "Search" => 2,
            "View" => 3,
            "Tabs" => 4,
            _ => 5,
        });
        entries
            .into_iter()
            .map(|entry| {
                let state = self.clone();
                let command = entry.command;
                creamui_widgets::CommandPaletteItem::new(
                    format!("{command:?}"),
                    entry.label,
                    move || {
                        state.dismiss();
                        state.run(command);
                    },
                )
                .category(command.category())
                .shortcut(entry.binding)
            })
            .collect()
    }
    pub(super) fn dialog(&self, panel: Panel, size: Size) -> BoxedWidget {
        if panel == Panel::GoTo {
            return self.goto_dialog(size);
        }
        let help = panel == Panel::Help;
        let close = self.clone();
        Box::new(
            creamui_widgets::CommandPalette::new(&self.palette, self.palette_items(), move || {
                close.dismiss()
            })
            .viewport(size)
            .placeholder(if help {
                "Search keyboard shortcuts…"
            } else {
                "Search commands…"
            })
            .hint(if help {
                "Ctrl + arrows  Move by word    Shift  Extend selection"
            } else {
                "↑ ↓  Navigate    Enter  Run    Esc  Close"
            })
            .reference(help),
        )
    }
    fn goto_dialog(&self, size: Size) -> BoxedWidget {
        let submit = self.clone();
        let cancel = self.clone();
        let go = self.clone();
        let close = self.clone();
        Box::new(
            Dialog::custom(move || close.dismiss())
                .width((size.width - 32.0).min(380.0))
                .placement(DialogPlacement::TopLeft)
                .header(Box::new(DialogHeader::new("Go to position").compact()))
                .content(Box::new(DialogContent::new().compact().with_children(
                    vec![
                        Box::new(DialogMessage::new(
                            "Enter a line and optional column, or a byte offset.",
                        )),
                        Box::new(
                            TextInput::controlled_with_style(
                                Style {
                                    size: layout::Size {
                                        width: Dimension::Percent(1.0),
                                        height: Dimension::Length(38.0),
                                    },
                                    ..Default::default()
                                },
                                &self.prompt,
                            )
                            .placeholder("Line:column or 0x0000")
                            .on_submit(move || submit.go_to()),
                        ),
                    ],
                )))
                .footer(Box::new(
                    DialogFooter::new().compact().child(Box::new(
                        ModalActions::new()
                            .child(Button::styled(
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                                "Cancel",
                                ButtonState::Normal,
                                move || cancel.dismiss(),
                            ))
                            .child(Button::styled(
                                ButtonVariant::Primary,
                                ButtonSize::Sm,
                                "Go",
                                ButtonState::Normal,
                                move || go.go_to(),
                            )),
                    )),
                )),
        )
    }
}
