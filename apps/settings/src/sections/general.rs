use crate::common::{group, row, section, update_config};
use coconut_core::{DockConfig, DockPosition, IslandEntry, SectionConfig, ShellConfig};
use creamui_core::{BoxedWidget, Size, StateStyle, Style};
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{RawButton, SegmentedControl, Select, SelectController, Switch, Text, TextSize};

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

/// Ids a click-to-add button offers per section. Tray's per-device ids
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

/// A structural editor for the dock/section/island topology: add or remove
/// docks, add or remove sections within a dock, and move islands between
/// sections by removing them from one and adding them to another. There is
/// no drag-and-drop or index reordering here — an island's order within a
/// section still falls back to the order it was added in (see
/// `coconut_core::ordered_islands`) — but every structural change the old
/// single global on/off toggle in `sections/widgets.rs` couldn't express
/// (which dock, which section) is doable from here.
pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let docks = config.get().docks;
    let dock_count = docks.len();
    let mut groups: Vec<BoxedWidget> = docks
        .iter()
        .enumerate()
        .map(|(index, dock)| dock_group(config, index, dock, dock_count))
        .collect();
    groups.push(group(vec![row(
        "Add a dock",
        text_button("+ Dock", {
            let config = config.clone();
            move || {
                update_config(&config, |c| c.docks.push(DockConfig::default()));
            }
        }),
    )]));

    section(
        "Docks",
        "Add, remove, and position docks, and choose which islands show in each section.",
        groups,
    )
}

fn dock_group(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    dock: &DockConfig,
    dock_count: usize,
) -> BoxedWidget {
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
    let mut rows = vec![
        row(&format!("Dock {} enabled", dock_index + 1), enabled_switch),
        row(&format!("Dock {} position", dock_index + 1), position_control),
    ];
    let section_count = dock.sections.len();
    for (section_index, dock_section) in dock.sections.iter().enumerate() {
        rows.push(section_row(
            config,
            dock_index,
            section_index,
            section_count,
            dock_section,
        ));
    }
    rows.push(row(
        "Add a section",
        text_button("+ Section", {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        d.sections.push(SectionConfig::default());
                    }
                });
            }
        }),
    ));
    if dock_count > 1 {
        rows.push(row(
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
    group(rows)
}

#[allow(clippy::too_many_arguments)]
fn section_row(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    dock_section: &SectionConfig,
) -> BoxedWidget {
    let enabled_switch: BoxedWidget = Box::new(Switch::new(dock_section.enabled, {
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
    }));
    let mut rows = vec![row(
        &format!("Section {} enabled", section_index + 1),
        enabled_switch,
    )];

    let island_count = dock_section.islands.len();
    for (island_index, entry) in dock_section.islands.iter().enumerate() {
        rows.push(row(
            &format!("  {}", entry.id),
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
        let picker: BoxedWidget = Box::new(
            Select::controlled(&missing, controller)
                .searchable()
                .on_select({
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
                }),
        );
        rows.push(row(&format!("Add to section {}", section_index + 1), picker));
    }

    Box::new(Flex::column().gap(4.0).with_children(rows))
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
