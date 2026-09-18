use crate::common::{group, row, section, update_config};
use coconut_core::{IslandEntry, ShellConfig};
use coconut_plugin_clock::ClockConfig;
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::{Switch, TextInput};

fn island_present(config: &ShellConfig, id: &str) -> bool {
    config
        .docks
        .iter()
        .flat_map(|dock| &dock.sections)
        .flat_map(|section| &section.islands)
        .any(|entry| entry.id == id)
}

/// Toggling an island on adds it to the first dock's first section; toggling
/// it off removes it wherever it currently sits. A full editor for
/// choosing which dock/section/index an island lands in — the real design
/// from the dock/island/panel refactor plan — is a later structural-editor
/// pass; this keeps the common "just turn it on/off" case working against
/// the new `[[dock.section.island]]` shape.
fn toggle_island(config: &mut ShellConfig, id: &str) {
    let present = island_present(config, id);
    if present {
        for dock in &mut config.docks {
            for section in &mut dock.sections {
                section.islands.retain(|entry| entry.id != id);
            }
        }
        return;
    }
    if config.docks.is_empty() {
        config.docks.push(Default::default());
    }
    let dock = &mut config.docks[0];
    if dock.sections.is_empty() {
        dock.sections.push(Default::default());
    }
    dock.sections[0].islands.push(IslandEntry::with_id(id));
}

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let toggles = group(vec![
        island_toggle_row("Weather", "weather", config),
        island_toggle_row("Now playing", "current_playing", config),
        island_toggle_row("App launcher", "app_launcher", config),
        island_toggle_row("Control center", "control_center", config),
        island_toggle_row("Clock", "clock", config),
    ]);
    let format = group(vec![clock_format_row()]);

    section(
        "Widgets",
        "Choose which widgets appear on your panels and set the clock format.",
        vec![toggles, format],
    )
}

fn island_toggle_row(label: &str, id: &'static str, config: &Signal<ShellConfig>) -> BoxedWidget {
    let checked = island_present(&config.get(), id);
    let apply = config.clone();
    let switch: BoxedWidget = Box::new(Switch::new(checked, move || {
        update_config(&apply, |c| toggle_island(c, id));
    }));
    row(label, switch)
}

fn clock_format_row() -> BoxedWidget {
    let clock: ClockConfig = coconut_core::modules::load_module("clock");
    let input: BoxedWidget = Box::new(TextInput::new(clock.format, move |value: String| {
        let clock = ClockConfig { format: value };
        if let Err(error) = coconut_core::modules::save_module("clock", &clock) {
            eprintln!("settings: failed to save modules/clock.toml: {error}");
        }
    }));
    row("Clock format", input)
}
