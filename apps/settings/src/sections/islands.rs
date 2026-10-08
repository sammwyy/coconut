use crate::common::{group, labeled_group, row, section, update_config};
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
        KnownIsland {
            id: "logo",
            label: "Workspaces",
            schema: Vec::new(),
        },
        KnownIsland {
            id: "weather",
            label: "Weather",
            schema: Vec::new(),
        },
        KnownIsland {
            id: "current_playing",
            label: "Now playing",
            schema: Vec::new(),
        },
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
        KnownIsland {
            id: "control_center",
            label: "Control center",
            schema: Vec::new(),
        },
        KnownIsland {
            id: "clock",
            label: "Clock",
            schema: Vec::new(),
        },
    ]
}

fn island_present(config: &ShellConfig, id: &str, statusbar: bool) -> bool {
    let bar = if statusbar {
        &config.statusbar
    } else {
        &config.dockbar
    };
    bar.sections
        .iter()
        .flat_map(|section| &section.islands)
        .any(|entry| entry.id == id)
}

fn toggle_island(config: &mut ShellConfig, id: &str, statusbar: bool) {
    let bar = if statusbar {
        &mut config.statusbar
    } else {
        &mut config.dockbar
    };
    let present = bar
        .sections
        .iter()
        .flat_map(|section| &section.islands)
        .any(|entry| entry.id == id);
    if present {
        for section in &mut bar.sections {
            section.islands.retain(|entry| entry.id != id);
        }
        return;
    }
    if bar.sections.is_empty() {
        bar.sections.push(Default::default());
    }
    bar.sections[0].islands.push(IslandEntry::with_id(id));
}

pub fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    detail: &Signal<Option<&'static str>>,
    settings: &crate::sections::island_settings::State,
) -> BoxedWidget {
    if let Some(id) = detail.get() {
        if let Some(entry) = known_islands().into_iter().find(|entry| entry.id == id) {
            return island_detail_page(size, entry, detail, settings);
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
        "Choose whether each widget appears in the statusbar, dockbar, or both.",
        vec![
            labeled_group("Widgets", toggles),
            labeled_group("Clock", format),
        ],
    )
}

fn island_row(
    entry: KnownIsland,
    config: &Signal<ShellConfig>,
    detail: &Signal<Option<&'static str>>,
) -> BoxedWidget {
    let status_checked = island_present(&config.get(), entry.id, true);
    let dock_checked = island_present(&config.get(), entry.id, false);
    let status_apply = config.clone();
    let id = entry.id;
    let status_switch: BoxedWidget = Box::new(Switch::new(status_checked, move || {
        update_config(&status_apply, |c| toggle_island(c, id, true));
    }));
    let dock_apply = config.clone();
    let dock_switch: BoxedWidget = Box::new(Switch::new(dock_checked, move || {
        update_config(&dock_apply, |c| toggle_island(c, id, false));
    }));
    let mut control = Flex::row()
        .gap(8.0)
        .align(Align::Center)
        .child(Box::new(creamui_widgets::Text::secondary("Status")) as BoxedWidget)
        .child(status_switch)
        .child(Box::new(creamui_widgets::Text::secondary("Dock")) as BoxedWidget)
        .child(dock_switch);
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
    settings: &crate::sections::island_settings::State,
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
    let page = crate::sections::island_settings::build(
        size,
        entry.id,
        entry.label,
        &entry.schema,
        settings,
    );
    Box::new(Flex::column().gap(14.0).with_children(vec![back, page]))
}

fn clock_format_row() -> BoxedWidget {
    let clock: ClockConfig = coconut_core::modules::load_module("clock");
    let input: BoxedWidget = Box::new(crate::common::form_input(
        TextInput::new(clock.format, move |value: String| {
            let clock = ClockConfig { format: value };
            if let Err(error) = coconut_core::modules::save_module("clock", &clock) {
                eprintln!("settings: failed to save modules/clock.toml: {error}");
            }
        }),
        180.0,
    ));
    row("Clock format", input)
}
