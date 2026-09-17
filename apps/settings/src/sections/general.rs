use crate::common::{group, row, section, update_config};
use coconut_core::{DockConfig, DockPosition, IslandEntry, SectionConfig, ShellConfig};
use creamui_core::{BoxedWidget, Size, StateStyle, Style};
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::{RawButton, SegmentedControl, Text, TextSize};

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

    let mut rows = vec![row(&format!("Dock {} position", dock_index + 1), position_control)];
    for (section_index, dock_section) in dock.sections.iter().enumerate() {
        rows.push(section_row(config, dock_index, section_index, dock_section));
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

fn section_row(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    dock_section: &SectionConfig,
) -> BoxedWidget {
    let mut chips = Flex::row().gap(6.0).align(Align::Center).justify(Justify::End);
    for entry in &dock_section.islands {
        let id = entry.id.clone();
        chips = chips.child(text_button(&format!("{id} \u{2715}"), {
            let config = config.clone();
            let id = id.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        if let Some(s) = d.sections.get_mut(section_index) {
                            s.islands.retain(|entry| entry.id != id);
                        }
                    }
                });
            }
        }));
    }
    for &id in KNOWN_ISLAND_IDS {
        if dock_section.islands.iter().any(|entry| entry.id == id) {
            continue;
        }
        chips = chips.child(text_button(&format!("+ {id}"), {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        if let Some(s) = d.sections.get_mut(section_index) {
                            s.islands.push(IslandEntry::with_id(id));
                        }
                    }
                });
            }
        }));
    }
    row(&format!("Section {}", section_index + 1), Box::new(chips))
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
