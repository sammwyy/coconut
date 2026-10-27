use super::*;
use crate::navigation::DirectoryView;
use crate::views::directory::visible_entries;
use creamui_widgets::AppTitlebar;

pub(crate) const CHROME_HEIGHT: f32 = 54.0;
pub(crate) const COMPACT_STATUS_WIDTH: f32 = 210.0;

pub(crate) fn tab_bar(
    b: &BrowserState,
    width: f32,
    content_x: f32,
    show_client_controls: bool,
) -> Option<BoxedWidget> {
    // A lone tab does not need its own title strip: its location is already
    // visible in the address bar. Show the strip only when it is needed to
    // switch between tabs, reclaiming that vertical space for the file list.
    if b.tabs.get().len() == 1 {
        return None;
    }
    let t = use_theme();
    let titlebar = show_client_controls.then(|| {
        AppTitlebar::new(width)
            .origin_x(content_x)
            .height(CHROME_HEIGHT)
            .padding(0.0)
            .gap(0.0)
    });
    let (lead, trail) = titlebar
        .as_ref()
        .map(AppTitlebar::reserved)
        .unwrap_or_default();
    let usable_width = (width - lead - trail).max(0.0);
    let tabs = b.tabs.get();
    // Page the strip when space is tight, keeping the active tab and native
    // window controls visible even after shrinking a window with many tabs.
    let capacity = ((usable_width - 136.0) / 116.0).floor().max(1.0) as usize;
    let paged = tabs.len() > capacity;
    let start = if paged {
        b.active.get() / capacity * capacity
    } else {
        0
    };
    let end = (start + capacity).min(tabs.len());
    // Fill the usable strip instead of leaving a large, ambiguous blank area.
    // The decoration inset is supplied by the compositor/window decorations,
    // so tabs never extend under KWin's (or another compositor's) controls.
    let tab_width =
        ((usable_width - if paged { 136.0 } else { 66.0 }) / (end - start) as f32).max(58.0);
    let mut children = Vec::new();
    if paged {
        let previous = b.clone();
        children.push(icon_button(b, "back", start > 0, false, move || {
            previous.switch(start.saturating_sub(1))
        }));
    }
    for (index, tab) in tabs.iter().enumerate().take(end).skip(start) {
        let state = b.clone();
        let close = b.clone();
        let active = index == b.active.get();
        let title_width = (tab_width - 80.0).max(1.0);
        let tab_children = vec![
            Box::new(button(
                pad(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..row(7.0)
                    },
                    9.0,
                    0.0,
                ),
                false,
                move || state.switch(index),
                vec![
                    glyph(
                        b,
                        if tab.location() == &Location::Home {
                            "home"
                        } else {
                            "folder"
                        },
                        14.0,
                        if active {
                            t.colors.accent
                        } else {
                            t.colors.text_secondary
                        },
                    ),
                    label(
                        short(&tab.location().label(), (title_width / 8.0) as usize),
                        12.0,
                        !active,
                    ),
                ],
            )) as BoxedWidget,
            icon_button(b, "close", tabs.len() > 1, false, move || {
                close.close_tab(index)
            }),
        ];
        children.push(Box::new(
            RawView::new(fixed(tab_width, 36.0))
                .background(if active { t.colors.surface } else { CLEAR })
                .corner_radius(9.0)
                .with_children(tab_children),
        ) as BoxedWidget);
    }
    if paged {
        let next = b.clone();
        children.push(icon_button(
            b,
            "forward",
            end < tabs.len(),
            false,
            move || next.switch(end),
        ));
    }
    let add = b.clone();
    children.push(icon_button(b, "plus", true, false, move || add.add_tab()));
    children.push(Box::new(CUIWindowDragArea::new().layout(Style {
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(CHROME_HEIGHT),
        },
        ..row(0.0)
    })));
    let tabs = Box::new(
        RawView::new(pad(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(CHROME_HEIGHT),
                },
                flex_shrink: 0.0,
                ..row(3.0)
            },
            8.0,
            0.0,
        ))
        .background(t.colors.surface.mix(t.colors.accent, 0.025))
        .with_children(children),
    ) as BoxedWidget;
    Some(match titlebar {
        Some(titlebar) => Box::new(
            titlebar
                .background(t.colors.surface.mix(t.colors.accent, 0.025))
                .body_boxed(tabs),
        ),
        None => tabs,
    })
}

pub(crate) fn toolbar(
    b: &BrowserState,
    tab: &Tab,
    content_x: f32,
    width: f32,
    show_client_controls: bool,
) -> BoxedWidget {
    let t = use_theme();
    let back = b.clone();
    let forward = b.clone();
    let up = b.clone();
    let address = b.clone();
    let refresh = b.clone();
    let settings = b.clone();
    let add = b.clone();
    let parent = match tab.location() {
        Location::Directory(p) => p.parent().map(Path::to_path_buf),
        _ => None,
    };
    let titlebar = show_client_controls.then(|| {
        AppTitlebar::new(width)
            .origin_x(content_x)
            .height(CHROME_HEIGHT)
            .padding(0.0)
            .gap(0.0)
    });
    let (lead, trail) = titlebar
        .as_ref()
        .map(AppTitlebar::reserved)
        .unwrap_or_default();
    let usable_width = (width - lead - trail).max(0.0);
    let compact_view_mode = usable_width < 610.0;
    let mut items = Vec::new();
    items.extend([
        icon_button(b, "back", tab.cursor > 0, false, move || back.travel(false)),
        icon_button(
            b,
            "forward",
            tab.cursor + 1 < tab.history.len(),
            false,
            move || forward.travel(true),
        ),
        icon_button(b, "up", parent.is_some(), false, move || {
            if let Some(p) = &parent {
                up.navigate(Location::Directory(p.clone()));
            }
        }),
    ]);
    if b.tabs.get().len() == 1 {
        // Keep a compact, dedicated drag handle when the single-tab strip is
        // hidden. It replaces the otherwise empty title-strip real estate.
        items.push(Box::new(CUIWindowDragArea::new().layout(fixed(24.0, 32.0))));
    }
    if b.search_active.get() {
        let cancel = b.clone();
        let escape = b.clone();
        let blur = b.clone();
        items.push(Box::new(
            RawView::new(Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(if compact_view_mode { 72.0 } else { 96.0 }),
                    height: Dimension::Length(34.0),
                },
                ..row(8.0)
            })
            .with_children(vec![
                Box::new(
                    TextInput::controlled(&tab.search)
                        .layout(Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            ..TextInput::default_style()
                        })
                        .font_size(12.0)
                        .placeholder("Search files")
                        .corner_radius(7.0)
                        .background(t.colors.surface_elevated)
                        .border(t.colors.border, 0.5)
                        .on_key_press(move |input| {
                            if input.key == creamui_core::Key::Escape {
                                escape.cancel_search();
                            }
                        })
                        .on_focus_change(move |focused| {
                            if !focused {
                                blur.close_search_if_empty();
                            }
                        }),
                ),
                action("Cancel", false, true, move || cancel.cancel_search()),
            ]),
        ));
    } else if b.address_editing.get() {
        let escape = b.clone();
        let blur = b.clone();
        items.push(Box::new(
            TextInput::controlled(&tab.address)
                .layout(address_field_style(compact_view_mode))
                .font_size(12.0)
                .corner_radius(7.0)
                .background(t.colors.surface_elevated)
                .border(t.colors.border, 0.5)
                .on_submit(move || address.submit_address())
                .on_key_press(move |input| {
                    if input.key == creamui_core::Key::Escape {
                        escape.cancel_address_edit();
                    }
                })
                .on_focus_change(move |focused| {
                    if !focused {
                        blur.cancel_address_edit();
                    }
                }),
        ));
    } else {
        items.push(address_breadcrumb(b, tab, compact_view_mode));
    }
    items.push(icon_button(b, "refresh", true, false, move || {
        refresh.edit(Tab::reload);
        refresh.drives.set(files::drives());
    }));
    items.push(icon_button(b, "settings", true, false, move || {
        settings.open_settings()
    }));
    if b.tabs.get().len() == 1 {
        items.push(icon_button(b, "plus", true, false, move || add.add_tab()));
    }
    if tab.location() != &Location::Home {
        items.push(view_mode_switcher(b, tab.view, compact_view_mode));
    }
    let toolbar = Box::new(
        CUIWindowDragArea::new()
            .layout(pad(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(CHROME_HEIGHT),
                    },
                    flex_shrink: 0.0,
                    ..row(4.0)
                },
                12.0,
                0.0,
            ))
            .with_children(items),
    ) as BoxedWidget;
    match titlebar {
        Some(titlebar) => Box::new(titlebar.body_boxed(toolbar)),
        None => toolbar,
    }
}

fn address_field_style(compact: bool) -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::Length(0.0),
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(if compact { 72.0 } else { 96.0 }),
            height: Dimension::Length(34.0),
        },
        ..TextInput::default_style()
    }
}

fn address_breadcrumb(b: &BrowserState, tab: &Tab, compact: bool) -> BoxedWidget {
    let t = use_theme();
    let edit = b.clone();
    let parts = breadcrumb_parts(tab.location());
    let trail = short(&parts.join("  ›  "), if compact { 18 } else { 54 });
    Box::new(
        RawButton::new(address_field_style(compact), move || {
            edit.begin_address_edit()
        })
        .background(t.colors.surface_elevated)
        .border(t.colors.border, 0.5)
        .corner_radius(7.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .with_children(vec![label(trail, 12.0, false)]),
    )
}

fn breadcrumb_parts(location: &Location) -> Vec<String> {
    let Location::Directory(path) = location else {
        return vec!["Home".into()];
    };
    let home = files::home();
    if path == &home {
        return vec!["Home".into()];
    }
    if let Ok(relative) = path.strip_prefix(&home) {
        return std::iter::once("Home".into())
            .chain(relative.components().filter_map(component_label))
            .collect();
    }
    let mut parts = if path.is_absolute() {
        vec!["/".into()]
    } else {
        Vec::new()
    };
    parts.extend(path.components().filter_map(component_label));
    if parts.is_empty() {
        parts.push(path.display().to_string());
    }
    parts
}

fn component_label(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn view_mode_switcher(b: &BrowserState, active: DirectoryView, _compact: bool) -> BoxedWidget {
    let modes = [
        (DirectoryView::List, "list"),
        (DirectoryView::Grid, "grid"),
        (DirectoryView::Columns, "columns"),
    ];
    view(
        row(2.0),
        modes
            .into_iter()
            .map(|(mode, icon)| {
                let state = b.clone();
                icon_button(b, icon, true, active == mode, move || {
                    state.edit(|tab| tab.view = mode)
                })
            })
            .collect(),
    )
}

pub(crate) fn statusbar(b: &BrowserState, tab: &Tab, compact: bool) -> BoxedWidget {
    let count = if tab.location() == &Location::Home {
        let query = tab.search.value().to_lowercase();
        b.pins
            .get()
            .iter()
            .filter(|p| files::name(p).to_lowercase().contains(&query))
            .count()
            + b.drives
                .get()
                .iter()
                .filter(|d| d.label.to_lowercase().contains(&query))
                .count()
            + if b.context.directories_only() {
                0
            } else {
                b.recent
                    .get()
                    .iter()
                    .filter(|p| {
                        files::name(p).to_lowercase().contains(&query)
                            && b.context.accepts(p, false)
                    })
                    .count()
            }
    } else {
        visible_entries(b, tab).len()
    };
    let selection = b.visible_selection(tab);
    let selected = selection
        .as_ref()
        .map(|p| format!("{} selected", files::name(p)))
        .unwrap_or_else(|| {
            if tab.location() == &Location::Home {
                "Home".into()
            } else {
                "Double-click to open".into()
            }
        });
    let mut children = vec![
        view(
            Style {
                flex_grow: 1.0,
                ..row(0.0)
            },
            vec![label(short(&selected, 65), 11.0, true)],
        ),
        label(format!("{count} items"), 11.0, true),
    ];
    if !compact && b.context.is_manager() && selection.is_some() {
        let state = b.clone();
        children.push(action("Open", false, true, move || state.accept()));
    }
    Box::new(
        RawView::new(pad(
            Style {
                size: creamui_core::layout::Size {
                    width: if compact {
                        Dimension::Length(COMPACT_STATUS_WIDTH)
                    } else {
                        Dimension::Percent(1.0)
                    },
                    height: Dimension::Length(if compact { CHROME_HEIGHT } else { 38.0 }),
                },
                flex_shrink: 0.0,
                ..row(12.0)
            },
            if compact { 12.0 } else { 16.0 },
            0.0,
        ))
        .background(
            use_theme()
                .colors
                .surface
                .mix(use_theme().colors.accent, 0.025),
        )
        .with_children(children),
    )
}
