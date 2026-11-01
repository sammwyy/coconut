use coconut_plugin_kit::task_icons::{application_icon, TaskIconStyle};
use coconut_plugin_kit::{
    app_categories, app_matches,
    chrome::{
        shell_accent, shell_border, shell_card, shell_control_hover, shell_muted, shell_selected,
        shell_text,
    },
    launch_app,
    pins::{PinnedApp, Pins},
    AppCatalog, AppEntry, FileDropFilter, Interactive, IslandRenderContext, PanelRenderContext,
};
use creamui_core::{BoxedWidget, Point, StateStyle, Style, Styled};
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    Button, ButtonSize, ButtonState, ButtonVariant, ContextMenuItem, RawButton, RawText,
    ScrollController, TextController, TextInput, VirtualList, VirtualListState,
};
use std::rc::Rc;

#[derive(Clone)]
struct PickerState {
    query: TextController,
    scroll: ScrollController,
    rows: VirtualListState,
}
impl Default for PickerState {
    fn default() -> Self {
        Self {
            query: TextController::default(),
            scroll: ScrollController::default(),
            rows: VirtualListState::new(0, 50.0),
        }
    }
}

const VISIBLE_PINS: usize = 6;

enum PickerRow {
    Category(String),
    Application(AppEntry),
}

pub(super) fn island(ctx: &IslandRenderContext<'_>) -> BoxedWidget {
    let pins = ctx.shared.get::<Pins>();
    let entries = pins.as_ref().map(Pins::entries).unwrap_or_default();
    let error = pins.as_ref().and_then(Pins::error);
    build_island(
        entries,
        error,
        pins,
        ctx.position.is_vertical(),
        TaskIconStyle::new(
            ctx.config,
            ctx.scale,
            ctx.position == coconut_core::DockPosition::Top,
        ),
        ctx.open_panel.clone(),
    )
}

fn build_island(
    entries: Vec<PinnedApp>,
    error: Option<String>,
    pins: Option<Pins>,
    vertical: bool,
    style: TaskIconStyle,
    open_panel: OpenPanel,
) -> BoxedWidget {
    let scale = style.scale;
    let mut content = if vertical {
        Flex::column()
    } else {
        Flex::row()
    };
    content = content.align(Align::Center).gap(6.0 * scale);
    let empty = entries.is_empty();
    let overflow = entries
        .iter()
        .skip(VISIBLE_PINS)
        .cloned()
        .collect::<Vec<_>>();
    for entry in entries.into_iter().take(VISIBLE_PINS) {
        content = content.child(pinned_button(
            entry,
            pins.clone(),
            open_panel.clone(),
            style,
        ));
    }
    if !overflow.is_empty() {
        let host = creamui_reactive::try_use_context::<coconut_plugin_kit::InteractionHost>()
            .unwrap_or_default();
        let more_pins = pins.clone();
        let more_open = open_panel.clone();
        let menu = Rc::new(move |at: Point| {
            overflow_menu(&overflow, more_pins.clone(), more_open.clone(), at)
        });
        let secondary = menu.clone();
        content = content.child(Box::new(
            Interactive::new(Box::new(
                Flex::row()
                    .size(style.width(), style.height())
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .child(Box::new(RawText::new("…", shell_text(), 18.0 * scale))),
            ))
            .with_style(style.style())
            .on_click(move |at| host.open_menu(at, menu(at)))
            .context_menu(move |at| secondary(at)),
        ));
    }
    if empty {
        content = content.child(Box::new(Flex::row().size(style.width(), style.height())));
    }
    if error.is_some() {
        content = content.child(Box::new(
            Flex::row()
                .size(12.0 * scale, style.height())
                .align(Align::Center)
                .justify(Justify::Center)
                .child(Box::new(RawText::new("!", shell_text(), 14.0 * scale))),
        ));
    }
    let separator = || {
        Box::new(
            Flex::row()
                .size(
                    if vertical { 24.0 } else { 1.0 } * scale,
                    if vertical { 1.0 } else { 24.0 } * scale,
                )
                .background(coconut_plugin_kit::apply_opacity(shell_text(), 0.12)),
        ) as BoxedWidget
    };
    let mut group = if vertical {
        Flex::column()
    } else {
        Flex::row()
    };
    group = group
        .align(Align::Center)
        .gap(6.0 * scale)
        .child(separator())
        .child(Box::new(content))
        .child(separator());
    let open = open_panel.clone();
    let menu_pins = pins.clone();
    let mut widget = Interactive::new(Box::new(group))
        .context_menu(move |at| pin_menu(open.clone(), at, menu_pins.clone()));
    if let Some(pins) = pins {
        widget = widget.on_drop_files(FileDropFilter::extensions(["desktop"]), move |paths| {
            pins.pin(paths)
        });
    }
    Box::new(widget)
}

type OpenPanel = Rc<dyn Fn(&'static str, Point)>;

fn pin_menu(open: OpenPanel, at: Point, pins: Option<Pins>) -> Vec<ContextMenuItem> {
    let mut items = vec![ContextMenuItem::new(
        "pin-application",
        "Manage pinned applications…",
        move || open("pin_application", at),
    )];
    if let Some(pins) = pins {
        if let Some(error) = pins.error() {
            items.push(ContextMenuItem::separator());
            items.push(ContextMenuItem::new("pin-error", error, || {}).enabled(false));
            items.push(ContextMenuItem::new(
                "dismiss-error",
                "Dismiss error",
                move || pins.dismiss_error(),
            ));
        }
    }
    items
}

fn pinned_button(
    entry: PinnedApp,
    pins: Option<Pins>,
    open: OpenPanel,
    style: TaskIconStyle,
) -> BoxedWidget {
    let app = entry.application.clone();
    let button = RawButton::new(style.style(), move || {
        if let Some(app) = &app {
            launch_app(app);
        }
    })
    .disabled(entry.application.is_none())
    .child(style.content(
        application_icon(entry.application.as_ref(), style.icon_size()),
        false,
    ));
    Box::new(
        Interactive::new(Box::new(button))
            .context_menu(move |at| pinned_actions(&entry, pins.clone(), open.clone(), at)),
    )
}

fn pinned_name(entry: &PinnedApp) -> String {
    entry
        .application
        .as_ref()
        .map(|app| app.name.clone())
        .unwrap_or_else(|| {
            entry
                .pin
                .source
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
}

fn pinned_actions(
    entry: &PinnedApp,
    pins: Option<Pins>,
    open: OpenPanel,
    at: Point,
) -> Vec<ContextMenuItem> {
    let name = pinned_name(entry);
    let mut items = Vec::new();
    if let Some(app) = entry.application.clone() {
        items.push(ContextMenuItem::new(
            "launch",
            format!("Open {name}"),
            move || launch_app(&app),
        ));
    } else {
        items.push(
            ContextMenuItem::new("unavailable", format!("{name} is unavailable"), || {})
                .enabled(false),
        );
    }
    if let Some(pins) = pins.clone() {
        let source = entry.pin.source.clone();
        let action = pins.clone();
        items.push(
            ContextMenuItem::new("unpin", "Unpin", move || action.unpin(source.clone()))
                .enabled(!pins.busy()),
        );
    }
    items.push(ContextMenuItem::separator());
    items.extend(pin_menu(open, at, pins));
    items
}

fn overflow_menu(
    entries: &[PinnedApp],
    pins: Option<Pins>,
    open: OpenPanel,
    at: Point,
) -> Vec<ContextMenuItem> {
    let mut items: Vec<_> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let app = entry.application.clone();
            ContextMenuItem::new(format!("overflow-{index}"), pinned_name(entry), move || {
                if let Some(app) = &app {
                    launch_app(app);
                }
            })
            .enabled(entry.application.is_some())
        })
        .collect();
    items.push(ContextMenuItem::separator());
    items.extend(pin_menu(open, at, pins));
    items
}

pub(super) fn picker(ctx: &PanelRenderContext) -> BoxedWidget {
    let state = ctx.scratch.get_or_insert_with(PickerState::default);
    let pins = ctx.shared.get::<Pins>();
    let catalog = ctx.shared.get::<AppCatalog>().unwrap_or_default();
    let scroll = state.scroll.clone();
    state.query.on_change(move |_, next| {
        scroll.set(0.0);
        Some(next.to_owned())
    });
    let query = state.query.value();
    let apps: Vec<_> = catalog
        .apps()
        .into_iter()
        .filter(|app| app.desktop_file.is_some() && app_matches(app, &query, "All"))
        .collect();
    let categories = app_categories(&apps);
    let empty = apps.is_empty();
    let mut rows = Vec::new();
    for category in categories.into_iter().filter(|category| category != "All") {
        rows.push(PickerRow::Category(category.clone()));
        rows.extend(
            apps.iter()
                .filter(|app| app.category == category)
                .cloned()
                .map(PickerRow::Application),
        );
    }
    state.rows.set_item_count(rows.len(), 50.0);
    for (index, row) in rows.iter().enumerate() {
        if matches!(row, PickerRow::Category(_)) {
            state.rows.set_height(index, 30.0);
        }
    }
    let width = ctx.size.width - 34.0;
    let row_pins = pins.clone();
    let list = VirtualList::new(
        Style::new()
            .flex_grow(1.0)
            .flex_basis(0.0)
            .min_height(0.0)
            .into(),
        state.rows.clone(),
        state.scroll.clone(),
        (ctx.size.height - 118.0).max(80.0),
        move |index| match &rows[index] {
            PickerRow::Category(category) => Box::new(
                Flex::row()
                    .height(30.0)
                    .padding(8.0)
                    .child(Box::new(RawText::new(category, shell_muted(), 11.0))),
            ),
            PickerRow::Application(app) => {
                let path = app.desktop_file.clone().unwrap();
                let pinned = row_pins.as_ref().is_some_and(|pins| pins.contains(&path));
                let enabled = row_pins
                    .as_ref()
                    .is_some_and(|pins| pins.loaded() && !pins.busy());
                let action = row_pins.clone();
                let row = application_row(app, pinned, width);
                let button = RawButton::new(
                    Style::new()
                        .width(width)
                        .height(46.0)
                        .corner_radius(8.0)
                        .hover(StateStyle::new().background(shell_control_hover()))
                        .pressed(StateStyle::new().background(shell_selected())),
                    move || {
                        if let Some(pins) = &action {
                            if pinned {
                                pins.unpin(path.clone());
                            } else {
                                pins.pin(vec![path.clone()]);
                            }
                        }
                    },
                )
                .disabled(!enabled)
                .child(row);
                Box::new(Flex::column().height(50.0).child(Box::new(button)))
            }
        },
    )
    .background(shell_card());
    let close = ctx.close.clone();
    let mut root = Flex::column()
        .size(ctx.size.width, ctx.size.height)
        .padding(12.0)
        .gap(10.0)
        .background(shell_card())
        .border(shell_border(), 1.0)
        .child(Box::new(
            Flex::row()
                .height(32.0)
                .flex_shrink(0.0)
                .align(Align::Center)
                .gap(8.0)
                .child(Box::new(RawText::new(
                    "Pinned applications",
                    shell_text(),
                    16.0,
                )))
                .child(Box::new(Flex::row().grow(1.0)))
                .child(Box::new(Button::styled(
                    ButtonVariant::Tertiary,
                    ButtonSize::Sm,
                    "Done",
                    ButtonState::Normal,
                    move || close(),
                ))),
        ))
        .child(Box::new(
            TextInput::controlled(&state.query)
                .placeholder("Search applications")
                .width(ctx.size.width - 26.0)
                .height(36.0)
                .flex_shrink(0.0),
        ));
    if let Some(error) = pins.as_ref().and_then(Pins::error) {
        root = root.child(Box::new(RawText::new(error, shell_text(), 12.0)));
    }
    if empty {
        root = root.child(Box::new(
            Flex::column()
                .grow(1.0)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(Box::new(RawText::new(
                    if catalog.loaded() {
                        "No applications found"
                    } else {
                        "Scanning applications…"
                    },
                    shell_muted(),
                    13.0,
                ))),
        ));
    } else {
        root = root.child(Box::new(list));
    }
    Box::new(root)
}

fn application_row(app: &AppEntry, pinned: bool, width: f32) -> BoxedWidget {
    let mut row = Flex::row()
        .size(width, 46.0)
        .padding(8.0)
        .gap(10.0)
        .align(Align::Center)
        .child(application_icon(Some(app), 28.0))
        .child(Box::new(Flex::column().grow(1.0).min_width(0.0).child(
            Box::new(RawText::new(
                creamui_widgets::clamp_to_lines(
                    &app.name,
                    12.0,
                    (width - 100.0).max(40.0),
                    None,
                    1,
                ),
                shell_text(),
                12.0,
            )),
        )));
    row = row.child(Box::new(RawText::new(
        if pinned { "Unpin" } else { "Pin" },
        if pinned {
            shell_muted()
        } else {
            shell_accent()
        },
        11.0,
    )));
    Box::new(row)
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
