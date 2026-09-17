use crate::common::{group, row, section, update_config};
use coconut_core::{DockConfig, DockPosition, IslandEntry, SectionConfig, ShellConfig};
use creamui_core::layout::{Dimension, FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{
    RawButton, RawScrollView, RawView, ScrollController, SegmentedControl, Select,
    SelectController, Switch, Text, TextSize,
};

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

/// Ids a click-to-add picker offers per section. Tray's per-device ids
/// ("tray.wifi", ...) are intentionally left out here — those only exist
/// under `TrayMode::Individual` and are managed by `modules/tray.toml`, not
/// by picking a fixed id like the rest of these.
const KNOWN_ISLAND_IDS: &[&str] = &[
    "logo",
    "weather",
    "current_playing",
    "app_launcher",
    "control_center",
    "clock",
];

/// The sidebar label for a dock: its own name if it has one, otherwise a
/// positional fallback. Used both by `apps/settings/src/lib.rs`'s sidebar
/// tree and (implicitly, via the same value) nowhere else — the rendering
/// engine never reads `DockConfig::name`, it's Settings-only.
pub fn dock_label(index: usize, dock: &DockConfig) -> String {
    dock.name
        .clone()
        .unwrap_or_else(|| format!("Dock {}", index + 1))
}

/// One dock's full editor: enabled/position at the top, then every section
/// as a compact card (no "Section N" label — a card just shows what's in
/// it), separated by thin dividers, with a small "insert a section here"
/// button above each card and one more after the last. The whole page
/// scrolls, since a dock with several sections and many islands can run
/// taller than the window.
pub fn build_dock_page(
    _: Size,
    config: &Signal<ShellConfig>,
    scroll: &ScrollController,
    dock_index: usize,
) -> BoxedWidget {
    let docks = config.get().docks;
    let dock_count = docks.len();
    let Some(dock) = docks.get(dock_index) else {
        return section("Dock", "This dock no longer exists.", Vec::new());
    };

    let mut body: Vec<BoxedWidget> = vec![top_card(config, dock_index, dock)];
    body.push(separator());

    let section_count = dock.sections.len();
    for (section_index, dock_section) in dock.sections.iter().enumerate() {
        body.push(insert_section_button(config, dock_index, section_index));
        body.push(section_card(
            config,
            dock_index,
            section_index,
            section_count,
            dock_section,
        ));
    }
    body.push(insert_section_button(config, dock_index, section_count));

    if dock_count > 1 {
        body.push(separator());
        body.push(row(
            "Remove this dock",
            text_button("Remove dock", {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        if dock_index < c.docks.len() {
                            c.docks.remove(dock_index);
                        }
                    });
                }
            }),
        ));
    }

    let content: BoxedWidget = Box::new(Flex::column().gap(10.0).with_children(body));
    let scroll_style = LayoutStyle {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    let scroll_view: BoxedWidget = Box::new(
        RawScrollView::controlled(Style::new().layout(scroll_style), scroll.clone()).child(content),
    );

    section(
        &dock_label(dock_index, dock),
        "Enable or disable this dock, choose where it sits, and arrange its sections and islands.",
        vec![scroll_view],
    )
}

fn top_card(config: &Signal<ShellConfig>, dock_index: usize, dock: &DockConfig) -> BoxedWidget {
    let enabled_switch: BoxedWidget = Box::new(Switch::new(dock.enabled, {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    d.enabled = !d.enabled;
                }
            });
        }
    }));
    let selected = POSITIONS
        .iter()
        .position(|candidate| *candidate == dock.position)
        .unwrap_or(0);
    let position_control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, {
            let config = config.clone();
            move |index: usize| {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        d.position = POSITIONS[index];
                    }
                });
            }
        })
        .option("Top")
        .option("Bottom")
        .option("Left")
        .option("Right"),
    );
    group(vec![row("Enabled", enabled_switch), row("Position", position_control)])
}

/// A thin horizontal rule between cards — "--- (space) ---" spelled with
/// layout instead of text.
fn separator() -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(LayoutStyle {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(1.0),
            },
            ..Default::default()
        })
        .background(theme.colors.border),
    )
}

/// A small "+" button that inserts an empty section at `at` — before the
/// section currently occupying that position, or at the end if `at` is the
/// section count.
fn insert_section_button(config: &Signal<ShellConfig>, dock_index: usize, at: usize) -> BoxedWidget {
    Box::new(
        Flex::row().justify(creamui_widgets::layout::Justify::Center).child(text_button(
            "+ Section",
            {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        if let Some(d) = c.docks.get_mut(dock_index) {
                            let at = at.min(d.sections.len());
                            d.sections.insert(at, SectionConfig::default());
                        }
                    });
                }
            },
        )),
    )
}

#[allow(clippy::too_many_arguments)]
fn section_card(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    dock_section: &SectionConfig,
) -> BoxedWidget {
    let mut header = Flex::row().gap(6.0).align(Align::Center);
    header = header.child(Box::new(Switch::new(dock_section.enabled, {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    if let Some(s) = d.sections.get_mut(section_index) {
                        s.enabled = !s.enabled;
                    }
                }
            });
        }
    })));
    if section_index > 0 {
        header = header.child(text_button("\u{2191}", {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        d.sections.swap(section_index - 1, section_index);
                    }
                });
            }
        }));
    }
    if section_index + 1 < section_count {
        header = header.child(text_button("\u{2193}", {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        d.sections.swap(section_index, section_index + 1);
                    }
                });
            }
        }));
    }
    header = header.child(text_button("Remove section", {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    if section_index < d.sections.len() {
                        d.sections.remove(section_index);
                    }
                }
            });
        }
    }));

    let mut rows = vec![Box::new(header) as BoxedWidget];

    let island_count = dock_section.islands.len();
    for (island_index, entry) in dock_section.islands.iter().enumerate() {
        rows.push(row(
            &entry.id,
            island_controls(
                config,
                dock_index,
                section_index,
                section_count,
                island_index,
                island_count,
            ),
        ));
    }

    let missing: Vec<&'static str> = KNOWN_ISLAND_IDS
        .iter()
        .copied()
        .filter(|id| !dock_section.islands.iter().any(|entry| entry.id == *id))
        .collect();
    if !missing.is_empty() {
        let controller = SelectController::new(0);
        let picker: BoxedWidget = Box::new(Select::controlled(&missing, controller).searchable().on_select({
            let config = config.clone();
            move |index: usize| {
                let Some(&id) = missing.get(index) else {
                    return;
                };
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        if let Some(s) = d.sections.get_mut(section_index) {
                            s.islands.push(IslandEntry::with_id(id));
                        }
                    }
                });
            }
        }));
        rows.push(row("Add island", picker));
    }

    group(rows)
}

/// Move/remove controls for one island: "up" swaps it earlier in this
/// section, or — if it's already first — moves it to the end of the
/// previous section; "down" is the mirror image into the next section. At
/// either end of a dock's section list, the corresponding button is simply
/// omitted rather than wrapping around or crossing into another dock.
#[allow(clippy::too_many_arguments)]
fn island_controls(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    island_index: usize,
    island_count: usize,
) -> BoxedWidget {
    let mut controls = Flex::row().gap(4.0).align(Align::Center);
    if island_index > 0 || section_index > 0 {
        controls = controls.child(text_button("\u{2191}", {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    move_island_up(c, dock_index, section_index, island_index);
                });
            }
        }));
    }
    if island_index + 1 < island_count || section_index + 1 < section_count {
        controls = controls.child(text_button("\u{2193}", {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    move_island_down(c, dock_index, section_index, island_index);
                });
            }
        }));
    }
    controls = controls.child(text_button("\u{2715}", {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    if let Some(s) = d.sections.get_mut(section_index) {
                        if island_index < s.islands.len() {
                            s.islands.remove(island_index);
                        }
                    }
                }
            });
        }
    }));
    Box::new(controls)
}

fn move_island_up(config: &mut ShellConfig, dock_index: usize, section_index: usize, island_index: usize) {
    let Some(dock) = config.docks.get_mut(dock_index) else {
        return;
    };
    if island_index == 0 {
        if section_index == 0 || island_index >= dock.sections[section_index].islands.len() {
            return;
        }
        let entry = dock.sections[section_index].islands.remove(island_index);
        dock.sections[section_index - 1].islands.push(entry);
    } else if island_index < dock.sections[section_index].islands.len() {
        dock.sections[section_index]
            .islands
            .swap(island_index - 1, island_index);
    }
}

fn move_island_down(config: &mut ShellConfig, dock_index: usize, section_index: usize, island_index: usize) {
    let Some(dock) = config.docks.get_mut(dock_index) else {
        return;
    };
    let Some(current) = dock.sections.get(section_index) else {
        return;
    };
    let last = current.islands.len().saturating_sub(1);
    if island_index != last {
        if island_index + 1 < current.islands.len() {
            dock.sections[section_index]
                .islands
                .swap(island_index, island_index + 1);
        }
        return;
    }
    if section_index + 1 >= dock.sections.len() {
        return;
    }
    let entry = dock.sections[section_index].islands.remove(island_index);
    dock.sections[section_index + 1].islands.insert(0, entry);
}

fn text_button(label: &str, on_click: impl Fn() + 'static) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawButton::new(
            Style::new()
                .background(theme.colors.surface_hover)
                .corner_radius(6.0)
                .hover(StateStyle::new().background(theme.colors.accent))
                .pressed(StateStyle::new().background(theme.colors.accent_pressed)),
            move || on_click(),
        )
        .child(Box::new(Text::new(label.to_owned()).size(TextSize::Sm)) as BoxedWidget),
    )
}
