use crate::common::{group, row, section, update_config};
use coconut_core::{DockConfig, DockPosition, ShellConfig};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::SegmentedControl;

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

/// Edits the first configured dock's position only. Adding, removing, or
/// otherwise managing multiple docks is a full structural editor left for a
/// later pass (see the dock/island/panel refactor plan's "Deferred"
/// section) — today's Settings still assumes the single-dock setup every
/// `shell.toml` ships with by default.
pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let position = config
        .get()
        .docks
        .first()
        .map(|dock| dock.position)
        .unwrap_or_default();
    let selected = POSITIONS
        .iter()
        .position(|candidate| *candidate == position)
        .unwrap_or(0);
    let set_position = config.clone();
    let control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index: usize| {
            update_config(&set_position, |c| {
                if let Some(dock) = c.docks.first_mut() {
                    dock.position = POSITIONS[index];
                } else {
                    c.docks.push(DockConfig {
                        position: POSITIONS[index],
                        ..Default::default()
                    });
                }
            });
        })
        .option("Top")
        .option("Bottom")
        .option("Left")
        .option("Right"),
    );

    section(
        "Dock",
        "Choose where the dock sits on screen. Side docks use a compact icon layout.",
        vec![group(vec![row("Dock position", control)])],
    )
}
