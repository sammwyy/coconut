use super::bar_drag::{self, Destination, Region};
use crate::components::group;
use crate::views::context::ViewContext;
use coconut_core::{ordered_islands, BarEdit, BarKind, DockConfig};
use coconut_plugin_kit::bar_editor::{self, BarEditor, EditHandler, Target, BAR_ITEMS};
use creamui_core::{BoxedWidget, KeyInput, Point, Size, Styled};
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex, Justify, StyleExt, Wrap};
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, CommandPalette, CommandPaletteController,
    CommandPaletteItem, Icon, RawButton, Select, Switch, Text, TextSize,
};
use std::rc::Rc;

#[derive(Clone)]
struct Catalog {
    group: usize,
    at: usize,
    apply: EditHandler,
    available: Vec<String>,
}

#[derive(Clone)]
pub struct State {
    pub editor: BarEditor,
    selected: Signal<usize>,
    catalog: Signal<Option<Catalog>>,
    search: CommandPaletteController,
    available: Vec<String>,
    pub(super) drag: bar_drag::State,
    route: Rc<std::cell::RefCell<String>>,
    #[cfg(test)]
    edit_override: Rc<std::cell::RefCell<Option<EditHandler>>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            editor: BarEditor::default(),
            selected: Signal::new(0),
            catalog: Signal::new(None),
            search: CommandPaletteController::new(),
            available: available_items(),
            drag: Default::default(),
            route: Default::default(),
            #[cfg(test)]
            edit_override: Default::default(),
        }
    }
}

impl State {
    fn open_catalog(&self, group: usize, at: usize, apply: EditHandler) {
        self.search.reset();
        self.catalog.set(Some(Catalog {
            group,
            at,
            apply,
            available: self.available.clone(),
        }));
    }

    #[cfg(test)]
    pub(crate) fn preview_catalog(&self) {
        self.open_catalog(0, 0, Rc::new(|_| {}));
    }

    #[cfg(test)]
    pub(crate) fn preview_edits(&self, config: Signal<coconut_core::ShellConfig>, kind: BarKind) {
        *self.edit_override.borrow_mut() = Some(Rc::new(move |edit| {
            config.update(|config| edit.apply(kind.get_mut(config)).unwrap())
        }));
    }

    #[cfg(test)]
    pub(crate) fn preview_rect(&self, region: Region) -> creamui_core::Rect {
        self.drag.rect(region).expect("visible editor region")
    }

    pub fn focus_lost(&self) {
        self.drag.cancel();
        self.editor.menu.parent_focus_lost();
    }

    pub fn dismiss(&self) {
        self.drag.cancel();
        self.editor.menu.close();
        self.catalog.set(None);
    }

    pub fn handle_key(&self, input: KeyInput, pressed: bool) -> bool {
        if self.drag.is_active() && pressed {
            if input.key == creamui_core::Key::Escape {
                self.drag.cancel();
            }
            return true;
        }
        if self.catalog.peek().is_some() && pressed {
            return self
                .search
                .handle_key(input, &self.catalog_items(), || self.catalog.set(None));
        }
        self.editor.handle_key(&input.key, pressed)
    }

    fn catalog_items(&self) -> Vec<CommandPaletteItem> {
        let Some(catalog) = self.catalog.get() else {
            return Vec::new();
        };
        let mut items = catalog
            .available
            .iter()
            .map(|id| {
                let category = BAR_ITEMS
                    .iter()
                    .find(|item| item.id == id)
                    .map_or("Other widgets", |item| item.category);
                let state = self.clone();
                let apply = catalog.apply.clone();
                let group = catalog.group;
                let at = catalog.at;
                let id = id.clone();
                CommandPaletteItem::new(
                    id.clone(),
                    bar_editor::item_label(&id).to_owned(),
                    move || {
                        state.catalog.set(None);
                        apply(BarEdit::AddItem {
                            group,
                            at,
                            id: id.clone(),
                        });
                    },
                )
                .category(category)
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.category.cmp(&b.category));
        items
    }

    pub fn overlays(&self, size: Size) -> Vec<BoxedWidget> {
        let mut overlays = self.editor.overlays(size);
        if let Some(catalog) = self.catalog.get() {
            let state = self.clone();
            overlays.push(Box::new(
                CommandPalette::new(&self.search, self.catalog_items(), move || {
                    state.catalog.set(None)
                })
                .viewport(size)
                .width((size.width - 40.0).min(480.0))
                .max_results_height((size.height - 180.0).clamp(100.0, 360.0))
                .placeholder("Search items to add…")
                .hint(format!(
                    "Add to Group {} · Enter to add · Esc to cancel",
                    catalog.group + 1
                )),
            ));
        }
        if let Some((pointer, label)) = self.drag.preview() {
            let theme = creamui_theme::use_theme();
            let mut background = theme.colors.surface_elevated;
            background.a = 255;
            let mut style = creamui_core::layout::Style {
                position: creamui_core::layout::Position::Absolute,
                ..Default::default()
            };
            style.inset.left = creamui_core::layout::LengthPercentageAuto::Length(
                (pointer.x + 14.0).min((size.width - 220.0).max(0.0)),
            );
            style.inset.top = creamui_core::layout::LengthPercentageAuto::Length(
                (pointer.y + 14.0).min((size.height - 48.0).max(0.0)),
            );
            overlays.push(Box::new(
                creamui_widgets::RawView::new(style)
                    .padding(10.0)
                    .max_width(220.0)
                    .background(background)
                    .border(theme.colors.accent, 1.0)
                    .corner_radius(10.0)
                    .child(Box::new(Text::new(label).size(TextSize::Sm))),
            ));
        }
        overlays
    }
}

pub fn available_items() -> Vec<String> {
    use coconut_plugin_tray::{TrayConfig, TrayMode};
    let tray: TrayConfig = coconut_core::modules::load_module("tray");
    BAR_ITEMS
        .iter()
        .filter(|item| match item.id {
            "control_center" => tray.mode == TrayMode::Grouped,
            "tray.wifi" => tray.mode == TrayMode::Individual && tray.wifi.shows_in_bar(),
            "tray.bluetooth" => tray.mode == TrayMode::Individual && tray.bluetooth.shows_in_bar(),
            "tray.battery" => tray.mode == TrayMode::Individual && tray.battery.shows_in_bar(),
            "tray.volume" => tray.mode == TrayMode::Individual && tray.volume.shows_in_bar(),
            "tray.brightness" => {
                tray.mode == TrayMode::Individual && tray.brightness.shows_in_bar()
            }
            _ => true,
        })
        .map(|item| item.id.into())
        .collect()
}

fn handler(context: &ViewContext, kind: BarKind, bar: &DockConfig) -> Option<EditHandler> {
    #[cfg(test)]
    if let Some(apply) = context.bar_content.edit_override.borrow().clone() {
        return Some(apply);
    }
    let window = context.window.borrow().clone()?;
    let config = context.config.clone();
    let selected = context.bar_content.selected.clone();
    let groups = bar.sections.len();
    Some(context.bar_content.editor.handler(
        window,
        kind,
        bar.clone(),
        Rc::new(move |next| {
            if kind.get(&next).sections.len() > groups {
                selected.set(kind.get(&next).sections.len() - 1);
            }
            config.set(next);
        }),
    ))
}

fn menu(context: &ViewContext, kind: BarKind, bar: &DockConfig, target: Target, point: Point) {
    context.bar_content.drag.cancel();
    let Some(window) = context.window.borrow().clone() else {
        return;
    };
    let Some(apply) = handler(context, kind, bar) else {
        return;
    };
    let router = context.view.clone();
    let mut items = bar_editor::content_menu(
        kind,
        bar,
        target,
        apply,
        Rc::new(move |route| crate::routes::open(&router, &route)),
    );
    items.retain(|item| item.id != "settings" && item.id != "edit-group");
    if items.last().is_some_and(|item| item.id.is_empty()) {
        items.pop();
    }
    context.bar_content.editor.open(window, point, items);
}

fn menu_button(
    label: &str,
    context: &ViewContext,
    kind: BarKind,
    bar: &DockConfig,
    target: Target,
) -> BoxedWidget {
    let context = context.clone();
    let bar = bar.clone();
    let theme = creamui_theme::use_theme();
    let keyboard_context = context.clone();
    let keyboard_bar = bar.clone();
    let anchor = Point {
        x: (context.size.width - 180.0).max(16.0),
        y: 180.0,
    };
    Box::new(
        RawButton::new(
            creamui_core::Style::new()
                .height(32.0)
                .padding_xy(12.0, 6.0),
            move || menu(&keyboard_context, kind, &keyboard_bar, target, anchor),
        )
        .disabled(context.bar_content.editor.busy.get())
        .background(theme.colors.surface_elevated)
        .border(theme.colors.border, 1.0)
        .corner_radius(8.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .focus_style(creamui_core::StateStyle::new().outline(theme.colors.accent, 2.0))
        .with_click_position(move |point| menu(&context, kind, &bar, target, point))
        .child(Box::new(Text::new(label).size(TextSize::Sm))),
    )
}

fn draggable(
    button: RawButton,
    context: &ViewContext,
    source: Region,
    bar: &DockConfig,
    apply: &Option<EditHandler>,
) -> RawButton {
    let start = context.bar_content.drag.clone();
    let update = start.clone();
    let finish = start.clone();
    let select = context.bar_content.selected.clone();
    let keyboard_select = select.clone();
    let apply_start = apply.clone();
    let keyboard = apply.clone();
    let current = bar.clone();
    let keys_bar = bar.clone();
    let scroll = context.dock_scroll.clone();
    let window = context.window.clone();
    button
        .disabled(context.bar_content.editor.busy.get())
        .with_drag_start(move |local, rect| {
            if let Some(apply) = &apply_start {
                start.start(
                    source,
                    Point {
                        x: rect.x + local.x,
                        y: rect.y + local.y,
                    },
                    current.clone(),
                    apply.clone(),
                );
                if let Some(window) = window.borrow().as_ref() {
                    start.autoscroll(window.app(), scroll.clone());
                }
            }
        })
        .with_drag(move |local, rect| {
            update.update(Point {
                x: rect.x + local.x,
                y: rect.y + local.y,
            });
        })
        .with_drag_end(move || {
            if let Some(group) = finish.finish() {
                select.set(group);
            }
        })
        .on_key_press(move |input| {
            if !input.modifiers.alt {
                return;
            }
            let Some(apply) = &keyboard else {
                return;
            };
            use creamui_core::Key;
            let edit = match (source, input.key) {
                (Region::Item { group, item }, Key::Up) if item > 0 => Some(BarEdit::MoveItem {
                    group,
                    item,
                    to_group: group,
                    at: item - 1,
                }),
                (Region::Item { group, item }, Key::Down)
                    if item + 1 < keys_bar.sections[group].islands.len() =>
                {
                    Some(BarEdit::MoveItem {
                        group,
                        item,
                        to_group: group,
                        at: item + 2,
                    })
                }
                (Region::Item { group, item }, Key::Left) if group > 0 => Some(BarEdit::MoveItem {
                    group,
                    item,
                    to_group: group - 1,
                    at: keys_bar.sections[group - 1].islands.len(),
                }),
                (Region::Item { group, item }, Key::Right)
                    if group + 1 < keys_bar.sections.len() =>
                {
                    Some(BarEdit::MoveItem {
                        group,
                        item,
                        to_group: group + 1,
                        at: keys_bar.sections[group + 1].islands.len(),
                    })
                }
                (Region::Group(from), Key::Left | Key::Up) if from > 0 => {
                    Some(BarEdit::MoveGroup { from, to: from - 1 })
                }
                (Region::Group(from), Key::Right | Key::Down)
                    if from + 1 < keys_bar.sections.len() =>
                {
                    Some(BarEdit::MoveGroup { from, to: from + 1 })
                }
                _ => None,
            };
            if let Some(edit) = edit {
                let selected = match &edit {
                    BarEdit::MoveItem { to_group, .. } => Some(*to_group),
                    BarEdit::MoveGroup { to, .. } => Some(*to),
                    _ => None,
                };
                apply(edit);
                if let Some(selected) = selected {
                    keyboard_select.set(selected);
                }
            }
        })
}

fn drag_handle(
    context: &ViewContext,
    source: Region,
    bar: &DockConfig,
    apply: &Option<EditHandler>,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    Box::new(draggable(
        RawButton::new(
            creamui_core::Style::new()
                .width(28.0)
                .height(34.0)
                .padding(6.0),
            || {},
        )
        .corner_radius(6.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .focus_style(creamui_core::StateStyle::new().outline(theme.colors.accent, 2.0))
        .child(Box::new(
            Icon::new(context.icons.drag.clone(), theme.colors.text_secondary).size(16.0),
        )),
        context,
        source,
        bar,
        apply,
    ))
}

fn placement_key(
    identity: String,
    counts: &mut std::collections::HashMap<String, usize>,
) -> String {
    let count = counts.entry(identity.clone()).or_default();
    let key = format!("{identity}-{count}");
    *count += 1;
    key
}

pub fn build(context: &ViewContext, kind: BarKind, bar: &DockConfig) -> BoxedWidget {
    let state = &context.bar_content;
    state.drag.begin_frame();
    let url = context.view.url();
    if *state.route.borrow() != url {
        if let Some(group) = context
            .view
            .location()
            .query("group")
            .and_then(|group| group.parse::<usize>().ok())
        {
            state
                .selected
                .set(group.min(bar.sections.len().saturating_sub(1)));
        }
        *state.route.borrow_mut() = url;
    }
    let compact = context.size.width < 880.0;
    let selected = state
        .selected
        .get()
        .min(bar.sections.len().saturating_sub(1));
    let busy = state.editor.busy.get();
    let theme = creamui_theme::use_theme();
    let apply = handler(context, kind, bar);
    let mut content = Flex::column().gap(18.0).full_width();
    let create = apply.clone();
    let undo_context = context.clone();
    let current = bar.clone();
    let actions = Flex::row()
        .gap(8.0)
        .child(Box::new(
            Button::styled(
                ButtonVariant::Secondary,
                ButtonSize::Sm,
                "Undo",
                ButtonState::Normal,
                move || {
                    if let Some(window) = undo_context.window.borrow().clone() {
                        let config = undo_context.config.clone();
                        undo_context.bar_content.editor.undo(
                            &window,
                            kind,
                            &current,
                            Rc::new(move |next| config.set(next)),
                        );
                    }
                },
            )
            .disabled(busy || !state.editor.can_undo(kind, bar)),
        ))
        .child(Box::new(
            Button::styled(
                ButtonVariant::Secondary,
                ButtonSize::Sm,
                "Add group",
                ButtonState::Normal,
                move || {
                    if let Some(apply) = &create {
                        apply(BarEdit::AddGroup);
                    }
                },
            )
            .disabled(busy),
        ));
    content = content.child(Box::new(
        Flex::row()
            .full_width()
            .align(Align::Center)
            .justify(Justify::Between)
            .wrap(Wrap::Wrap)
            .gap(12.0)
            .child(Box::new(Text::new("Your layout").bold(true)))
            .child(Box::new(actions)),
    ));
    if !bar.enabled {
        content = content.child(Box::new(
            Text::secondary("This bar is hidden. You can still edit its content here.")
                .size(TextSize::Sm),
        ));
    }
    if bar.sections.is_empty() {
        return Box::new(content.child(group(vec![Box::new(
                Flex::column()
                    .gap(8.0)
                    .padding(20.0)
                    .child(Box::new(Text::new("Start with a group").bold(true)))
                    .child(Box::new(
                        Text::secondary("Add a group, then choose the items that go inside.")
                            .size(TextSize::Sm),
                    )),
            )])));
    }
    let mut overview = Flex::row()
        .full_width()
        .wrap(Wrap::Wrap)
        .gap(8.0)
        .padding(12.0);
    let mut group_keys = std::collections::HashMap::<String, usize>::new();
    for (index, section) in bar.sections.iter().enumerate() {
        let select = state.selected.clone();
        let label = format!(
            "Group {}{}",
            index + 1,
            if section.enabled { "" } else { " · Hidden" }
        );
        let drop_here = matches!(state.drag.destination(), Some(Destination::Item { group, .. }) if group == index);
        let summary = if compact {
            format!(
                "{} item{}",
                section.islands.len(),
                if section.islands.len() == 1 { "" } else { "s" }
            )
        } else if section.islands.is_empty() {
            "Empty group".into()
        } else {
            ordered_islands(&section.islands)
                .iter()
                .map(|entry| bar_editor::item_label(&entry.id))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let menu_context = context.clone();
        let current = bar.clone();
        let tile = RawButton::new(
            creamui_core::Style::new()
                .width(creamui_core::LengthValue::Percent(100.0))
                .padding(if compact { 8.0 } else { 12.0 }),
            move || select.set(index),
        )
        .background(if selected == index || drop_here {
            theme.colors.selection_background
        } else {
            theme.colors.surface
        })
        .border(
            if selected == index || drop_here {
                theme.colors.accent
            } else {
                theme.colors.border
            },
            1.0,
        )
        .corner_radius(10.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .focus_style(creamui_core::StateStyle::new().outline(theme.colors.accent, 2.0))
        .with_context_menu(move |point| {
            menu(&menu_context, kind, &current, Target::Group(index), point)
        })
        .child(Box::new(
            Flex::column()
                .full_width()
                .gap(5.0)
                .child(Box::new(
                    Flex::row()
                        .full_width()
                        .align(Align::Center)
                        .gap(4.0)
                        .child(Box::new(Flex::column().grow(1.0).min_width(0.0).child(
                            Box::new(Text::new(label).bold(true).size(TextSize::Sm)),
                        )))
                        .child(Box::new(
                            Icon::new(context.icons.drag.clone(), theme.colors.text_secondary)
                                .size(12.0),
                        )),
                ))
                .child(Box::new(Text::secondary(summary).size(TextSize::Xs))),
        ));
        let tile = draggable(tile, context, Region::Group(index), bar, &apply);
        overview = overview.child(bar_drag::measured(
            &state.drag,
            Region::Group(index),
            creamui_core::Style::new()
                .min_width(if compact { 80.0 } else { 140.0 })
                .grow(1.0)
                .basis(if compact { 82.0 } else { 180.0 }),
            Box::new(tile),
            placement_key(
                format!("{}-group-{section:?}", kind.title()),
                &mut group_keys,
            ),
        ));
    }
    content = content.child(group(vec![Box::new(overview)]));
    let section = &bar.sections[selected];
    let count = section.islands.len();
    let add_state = state.clone();
    let window = context.window.clone();
    let add = apply.clone();
    let mut rows: Vec<BoxedWidget> = vec![Box::new(
        Flex::row()
            .full_width()
            .align(Align::Center)
            .justify(Justify::Between)
            .gap(12.0)
            .padding(16.0)
            .child(Box::new(
                Text::new(format!("Group {}", selected + 1)).bold(true),
            ))
            .child(Box::new(
                Button::styled(
                    ButtonVariant::Primary,
                    ButtonSize::Sm,
                    "Add item…",
                    ButtonState::Normal,
                    move || {
                        if let Some(apply) = &add {
                            add_state.open_catalog(selected, count, apply.clone());
                            if let Some(window) = window.borrow().as_ref() {
                                window.focus_text_input();
                            }
                        }
                    },
                )
                .disabled(busy),
            )),
    )];
    let mut item_keys = std::collections::HashMap::<String, usize>::new();
    for (item, entry) in ordered_islands(&section.islands).iter().enumerate() {
        let source = Region::Item {
            group: selected,
            item,
        };
        let description = BAR_ITEMS.iter().find(|known| known.id == entry.id).map_or(
            "Unavailable widget · its configuration is preserved",
            |known| known.description,
        );
        let menu_context = context.clone();
        let current = bar.clone();
        let row = creamui_widgets::RawView::new(
            creamui_core::Style::new().width(creamui_core::LengthValue::Percent(100.0)),
        )
        .background(if state.drag.source() == Some(source) {
            theme.colors.surface_hover
        } else {
            theme.colors.surface_elevated
        })
        .with_context_menu(move |point| {
            menu(
                &menu_context,
                kind,
                &current,
                Target::Item {
                    group: selected,
                    item,
                },
                point,
            )
        })
        .child(Box::new(
            Flex::row()
                .full_width()
                .align(Align::Center)
                .gap(8.0)
                .padding(12.0)
                .child(drag_handle(context, source, bar, &apply))
                .child(Box::new(
                    Flex::column()
                        .grow(1.0)
                        .min_width(0.0)
                        .gap(4.0)
                        .child(Box::new(Text::new(bar_editor::item_label(&entry.id))))
                        .child(Box::new(Text::secondary(description).size(TextSize::Sm))),
                ))
                .child(menu_button(
                    "…",
                    context,
                    kind,
                    bar,
                    Target::Item {
                        group: selected,
                        item,
                    },
                )),
        ));
        rows.push(bar_drag::measured(
            &state.drag,
            source,
            creamui_core::Style::new().width(creamui_core::LengthValue::Percent(100.0)),
            Box::new(row),
            placement_key(
                format!("{}-item-{}-{:?}", kind.title(), entry.id, entry.config),
                &mut item_keys,
            ),
        ));
    }
    if count == 0 {
        rows.push(Box::new(
            Flex::column()
                .padding(16.0)
                .gap(4.0)
                .child(Box::new(Text::new("Make room for something useful")))
                .child(Box::new(
                    Text::secondary("Choose Add item to browse available widgets.")
                        .size(TextSize::Sm),
                )),
        ));
    }
    let mut gaps = vec![
        ("None".to_owned(), "0px".to_owned()),
        ("Small".into(), "4px".into()),
        ("Medium".into(), "10px".into()),
        ("Large".into(), "20px".into()),
        ("Between".into(), "between".into()),
        ("Evenly".into(), "evenly".into()),
    ];
    if !gaps.iter().any(|(_, gap)| *gap == section.gap) {
        gaps.push((section.gap.clone(), section.gap.clone()));
    }
    let gap_index = gaps
        .iter()
        .position(|(_, gap)| *gap == section.gap)
        .unwrap_or(0);
    let labels = gaps
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    let gap_apply = apply.clone();
    let spacing = Select::controlled(
        &labels,
        context.native.select(
            &format!("{}-group-{selected}-spacing", kind.title()),
            gap_index,
        ),
    )
    .width(130.0)
    .on_select(move |index| {
        if let Some(apply) = &gap_apply {
            apply(BarEdit::GroupGap {
                group: selected,
                gap: gaps[index].1.clone(),
            });
        }
    });
    let show = apply.clone();
    let enabled = section.enabled;
    let remove = apply.clone();
    rows.push(Box::new(
        Flex::row()
            .full_width()
            .wrap(Wrap::Wrap)
            .align(Align::Center)
            .gap(14.0)
            .padding(16.0)
            .child(Box::new(
                Flex::row()
                    .align(Align::Center)
                    .gap(8.0)
                    .child(Box::new(Text::secondary("Show group").size(TextSize::Sm)))
                    .child(Box::new(Switch::new(enabled, move || {
                        if let Some(apply) = &show {
                            apply(BarEdit::ShowGroup {
                                group: selected,
                                enabled: !enabled,
                            });
                        }
                    }))),
            ))
            .child(Box::new(
                Flex::row()
                    .align(Align::Center)
                    .gap(8.0)
                    .child(Box::new(Text::secondary("Spacing").size(TextSize::Sm)))
                    .child(Box::new(spacing)),
            ))
            .child(Box::new(
                Button::styled(
                    ButtonVariant::Tertiary,
                    ButtonSize::Sm,
                    "Remove group",
                    ButtonState::Normal,
                    move || {
                        if let Some(apply) = &remove {
                            apply(BarEdit::RemoveGroup(selected));
                        }
                    },
                )
                .disabled(busy),
            )),
    ));
    content = content.child(group(rows));
    content = content.child(Box::new(Text::secondary("Drag to reorder or drop onto another group. Alt + arrow keys also move a focused handle. Esc cancels.").size(TextSize::Sm)));
    let status = state.editor.status.get();
    if !status.is_empty() {
        content = content.child(Box::new(Text::secondary(status).size(TextSize::Sm)));
    }
    Box::new(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_search_adds_to_the_requested_group_and_dismisses() {
        creamui_reactive::with_context_scope(|| {
            let state = State::default();
            let edits = Rc::new(std::cell::RefCell::new(Vec::new()));
            let changed = edits.clone();
            state.open_catalog(2, 3, Rc::new(move |edit| changed.borrow_mut().push(edit)));
            state.search.query().set_value("clock");
            assert_eq!(
                state
                    .catalog_items()
                    .iter()
                    .filter(|item| item.id == "clock")
                    .count(),
                1
            );
            state.handle_key(
                KeyInput {
                    key: creamui_core::Key::Enter,
                    modifiers: Default::default(),
                },
                true,
            );
            assert!(
                matches!(&edits.borrow()[0], BarEdit::AddItem { group: 2, at: 3, id } if id == "clock")
            );
            assert!(state.catalog.peek().is_none());
        });
    }
}
