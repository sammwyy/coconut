use crate::common::{group, row, section, update_config};
use coconut_core::{IslandEntry, ShellConfig};
use coconut_plugin_app_launcher::{AppLauncherIsland, OpenWindowsIsland};
use coconut_plugin_clock::ClockConfig;
use coconut_plugin_kit::{ConfigField, Island};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{Button, ButtonSize, ButtonState, ButtonVariant, Switch, TextInput};

struct KnownIsland {
    id: &'static str,
    label: &'static str,
    schema: Vec<ConfigField>,
}

fn known_islands() -> Vec<KnownIsland> {
    vec![
        KnownIsland { id: "weather", label: "Weather", schema: Vec::new() },
        KnownIsland { id: "current_playing", label: "Now playing", schema: Vec::new() },
        KnownIsland {
            id: "app_launcher",
            label: "App launcher",
            schema: AppLauncherIsland.config_schema(),
        },
        KnownIsland {
            id: "open_windows",
            label: "Open windows",
            schema: OpenWindowsIsland.config_schema(),
        },
        KnownIsland { id: "control_center", label: "Control center", schema: Vec::new() },
        KnownIsland { id: "clock", label: "Clock", schema: Vec::new() },
    ]
}

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

pub fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    detail: &Signal<Option<&'static str>>,
) -> BoxedWidget {
    if let Some(id) = detail.get() {
        if let Some(entry) = known_islands().into_iter().find(|entry| entry.id == id) {
            return island_detail_page(size, entry, detail);
        }
        detail.set(None);
    }

    let toggles = group(
        known_islands()
            .into_iter()
            .map(|entry| island_row(entry, config, detail))
            .collect(),
    );
    let format = group(vec![clock_format_row()]);

    section(
        "Islands",
        "Choose which islands appear on your panels and set the clock format.",
        vec![toggles, format],
    )
}

fn island_row(
    entry: KnownIsland,
    config: &Signal<ShellConfig>,
    detail: &Signal<Option<&'static str>>,
) -> BoxedWidget {
    let checked = island_present(&config.get(), entry.id);
    let apply = config.clone();
    let id = entry.id;
    let switch: BoxedWidget = Box::new(Switch::new(checked, move || {
        update_config(&apply, |c| toggle_island(c, id));
    }));
    let mut control = Flex::row().gap(10.0).align(Align::Center).child(switch);
    if !entry.schema.is_empty() {
        let detail = detail.clone();
        control = control.child(Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "Configure",
            ButtonState::Normal,
            move || detail.set(Some(id)),
        )));
    }
    row(entry.label, Box::new(control))
}

fn island_detail_page(
    size: Size,
    entry: KnownIsland,
    detail: &Signal<Option<&'static str>>,
) -> BoxedWidget {
    let back = {
        let detail = detail.clone();
        Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "‹ Islands",
            ButtonState::Normal,
            move || detail.set(None),
        )) as BoxedWidget
    };
    let page = crate::sections::island_settings::build(size, entry.id, entry.label, &entry.schema);
    Box::new(Flex::column().gap(14.0).with_children(vec![back, page]))
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
