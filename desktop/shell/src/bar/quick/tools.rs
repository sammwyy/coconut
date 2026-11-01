use super::State;
use coconut_core::{BarEdit, BarKind, DockConfig};
use coconut_plugin_kit::bar_editor::{controls, item_label, BAR_ITEMS};
use creamui_core::{BoxedWidget, Size};
use creamui_render::{WindowHandle, WindowOptions};
use creamui_widgets::{
    layout::{Align, Flex},
    AppTitlebar, Button, ButtonSize, ButtonState, ButtonVariant, CommandPalette,
    CommandPaletteItem, ScrollView, SegmentedControl, Text, TextSize,
};
use std::rc::Rc;

impl State {
    pub fn open(
        &self,
        parent: WindowHandle,
        kind: BarKind,
        bar: DockConfig,
        available: Rc<Vec<String>>,
    ) {
        self.begin(&bar);
        if let Some(window) = self
            .tools
            .borrow()
            .as_ref()
            .filter(|window| window.is_open())
        {
            window.show();
            return;
        }
        let ready = self.clone();
        let build = self.clone();
        let theme = parent.theme();
        let keys_available = available.clone();
        let build_parent = parent.clone();
        parent.app().append_window(
            WindowOptions {
                title: format!("{} · Quick Edit", kind.title()),
                width: 420,
                height: 520,
                theme,
                ..Default::default()
            },
            theme.surface,
            move |window| {
                window.set_always_on_top(true);
                window.set_min_size(Some((320, 360)));
                let close = ready.clone();
                window.on_close_requested(move || {
                    if close.persistence.busy.peek() {
                        return false;
                    }
                    close.draft.set(None);
                    close.drag.set(None);
                    close.catalog.set(None);
                    close.appearance.dismiss();
                    close.persistence.dismiss_transients();
                    close.tools.borrow_mut().take();
                    true
                });
                let keys = ready.clone();
                window.on_keyboard_input(move |event, modifiers| {
                    coconut_plugin_kit::bar_editor::key_input(&event.key, modifiers)
                        .is_some_and(|input| keys.handle_key(input, event.pressed, &keys_available))
                });
                let blur = ready.clone();
                window.on_focus_lost(move || {
                    blur.persistence.menu.parent_focus_lost();
                    blur.appearance.dismiss();
                });
                *ready.tools.borrow_mut() = Some(window);
            },
            move |size| build.build_tools(size, &build_parent, kind, &available),
        );
    }

    pub(super) fn catalog_items(&self, available: &[String]) -> Vec<CommandPaletteItem> {
        let Some(group) = self.catalog.get() else {
            return Vec::new();
        };
        let mut items: Vec<_> = available
            .iter()
            .map(|id| {
                let id = id.clone();
                let state = self.clone();
                let category = BAR_ITEMS
                    .iter()
                    .find(|item| item.id == id)
                    .map_or("Other widgets", |item| item.category);
                CommandPaletteItem::new(id.clone(), item_label(&id).to_owned(), move || {
                    let Some(draft) = state.draft.peek() else {
                        return;
                    };
                    let Some(section) = draft.current.sections.get(group) else {
                        return;
                    };
                    state.apply(BarEdit::AddItem {
                        group,
                        at: section.islands.len(),
                        id: id.clone(),
                    });
                    state.catalog.set(None);
                })
                .category(category)
            })
            .collect();
        items.sort_by(|left, right| {
            left.category
                .cmp(&right.category)
                .then_with(|| left.label.cmp(&right.label))
        });
        items
    }

    pub(super) fn build_tools(
        &self,
        size: Size,
        parent: &WindowHandle,
        kind: BarKind,
        available: &[String],
    ) -> BoxedWidget {
        self.tools_tree(size, kind, available, {
            let parent = parent.clone();
            let state = self.clone();
            Rc::new(move || state.save(&parent, kind))
        })
    }

    pub(super) fn tools_tree(
        &self,
        size: Size,
        kind: BarKind,
        available: &[String],
        save: Rc<dyn Fn()>,
    ) -> BoxedWidget {
        let bar = self
            .draft
            .get()
            .map_or_else(DockConfig::default, |draft| draft.current);
        let state = self.clone();
        let change: controls::Change = Rc::new(move |edit| {
            if let Some(draft) = state.draft.peek() {
                let mut current = draft.current;
                edit(&mut current);
                state.replace(current);
            }
        });
        let selected = self.tab.get();
        let state = self.clone();
        let tabs = SegmentedControl::new(selected, move |tab| {
            state.tab.set(tab);
            state.scroll.set(0.0);
            state.appearance.dismiss();
        })
        .option("Layout")
        .option("Style")
        .option("Sections");
        let mut body = Flex::column().gap(4.0).padding(12.0);
        let rows = match selected {
            1 => self.appearance.build(&bar, &change),
            2 => self.sections(&bar),
            _ => controls::layout(&bar, &change),
        };
        for row in rows {
            body = body.child(row);
        }
        let state = self.clone();
        let add = action(
            "+ Add section",
            false,
            self.persistence.busy.get(),
            move || state.apply(BarEdit::AddGroup),
        );
        let state = self.clone();
        let cancel = action("Discard", false, self.persistence.busy.get(), move || {
            state.discard()
        });
        let save = action(
            if self.persistence.busy.get() {
                "Saving…"
            } else {
                "Save"
            },
            true,
            self.persistence.busy.get(),
            move || save(),
        );
        let status = self.persistence.status.get();
        let mut root = Flex::column()
            .size(size.width, size.height)
            .child(Box::new(AppTitlebar::new(size.width).start_boxed(
                Box::new(Text::new(format!("{} · Quick Edit", kind.title())).size(TextSize::Sm)),
            )))
            .child(Box::new(
                Flex::column()
                    .padding_xy(16.0, 10.0)
                    .gap(8.0)
                    .child(Box::new(
                        Text::new("Drag widgets or section grips on the bar.").size(TextSize::Sm),
                    ))
                    .child(Box::new(tabs)),
            ))
            .child(Box::new(
                ScrollView::controlled(
                    creamui_core::layout::Style {
                        flex_grow: 1.0,
                        flex_basis: creamui_core::layout::Dimension::Length(0.0),
                        ..Default::default()
                    },
                    self.scroll.clone(),
                )
                .child(Box::new(body)),
            ))
            .child(Box::new(
                Flex::row()
                    .padding(12.0)
                    .gap(8.0)
                    .align(Align::Center)
                    .child(add)
                    .child(Box::new(Flex::row().grow(1.0)))
                    .child(cancel)
                    .child(save),
            ));
        if !status.is_empty() {
            root = root.child(Box::new(
                Flex::column()
                    .padding_xy(16.0, 8.0)
                    .child(Box::new(Text::new(status).size(TextSize::Sm))),
            ));
        }
        let mut children: Vec<BoxedWidget> = vec![Box::new(root)];
        children.extend(self.persistence.overlays(size));
        if let Some(group) = self.catalog.get() {
            let state = self.clone();
            children.push(Box::new(
                CommandPalette::new(&self.search, self.catalog_items(available), move || {
                    state.catalog.set(None)
                })
                .viewport(size)
                .width((size.width - 24.0).min(480.0))
                .max_results_height((size.height - 140.0).max(80.0))
                .placeholder("Search items to add…")
                .hint(format!("Add to Section {}", group + 1)),
            ));
        }
        Box::new(
            Flex::column()
                .size(size.width, size.height)
                .with_children(children),
        )
    }

    fn sections(&self, bar: &DockConfig) -> Vec<BoxedWidget> {
        bar.sections
            .iter()
            .enumerate()
            .map(|(group, section)| {
                let state = self.clone();
                let add = action("+ Item", false, self.persistence.busy.get(), move || {
                    state.open_catalog(group)
                });
                let state = self.clone();
                let remove = action(
                    "Remove",
                    false,
                    !section.islands.is_empty() || self.persistence.busy.get(),
                    move || state.apply(BarEdit::RemoveGroup(group)),
                );
                let state = self.clone();
                let enabled = !section.enabled;
                let show = creamui_widgets::Switch::new(section.enabled, move || {
                    state.apply(BarEdit::ShowGroup { group, enabled })
                });
                let state = self.clone();
                let spacing = controls::spacing(
                    &section.gap,
                    Rc::new(move |gap| state.apply(BarEdit::GroupGap { group, gap })),
                );
                Box::new(
                    Flex::column()
                        .gap(8.0)
                        .child(controls::row(
                            format!("Section {} · {} items", group + 1, section.islands.len()),
                            Box::new(show),
                        ))
                        .child(spacing)
                        .child(Box::new(Flex::row().gap(8.0).child(add).child(remove))),
                ) as BoxedWidget
            })
            .collect()
    }
}

fn action(label: &str, primary: bool, disabled: bool, action: impl Fn() + 'static) -> BoxedWidget {
    Box::new(
        Button::styled(
            if primary {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            },
            ButtonSize::Sm,
            label,
            ButtonState::Normal,
            action,
        )
        .disabled(disabled),
    )
}
