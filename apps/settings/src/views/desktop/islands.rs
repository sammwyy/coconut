use crate::components::{group, labeled_group, row, section};
use coconut_plugin_app_launcher::{AppLauncherIsland, OpenWindowsIsland};
use coconut_plugin_clock::ClockConfig;
use coconut_plugin_kit::{ConfigField, Island};
use creamui_core::{BoxedWidget, Size};
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{Button, ButtonSize, ButtonState, ButtonVariant, TextInput};

struct KnownIsland {
    id: &'static str,
    label: &'static str,
    schema: Vec<ConfigField>,
}

fn known_islands() -> Vec<KnownIsland> {
    vec![
        #[cfg(target_os = "linux")]
        KnownIsland {
            id: "clipboard",
            label: "Clipboard history",
            schema: Vec::new(),
        },
        KnownIsland {
            id: "workspaces",
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

pub fn build(size: Size, settings: &crate::views::desktop::island_settings::State) -> BoxedWidget {
    if let Some(id) = creamui_router::use_params().get("widget_id") {
        if let Some(entry) = known_islands()
            .into_iter()
            .find(|entry| entry.id == id.as_str())
        {
            return island_detail_page(size, entry, settings);
        }
        return crate::components::pages::unavailable_page("Unknown widget");
    }

    let widgets = group(known_islands().into_iter().map(island_row).collect());
    let format = group(vec![clock_format_row()]);

    section(
        "Islands",
        "Add and arrange items in the bar editor. Configure shared widget behavior here.",
        vec![
            group(vec![
                bar_editor_row("Statusbar", "/desktop/statusbar?tab=contents"),
                bar_editor_row("Dockbar", "/desktop/dock?tab=contents"),
            ]),
            labeled_group("Widgets", widgets),
            labeled_group("Clock", format),
        ],
    )
}

fn bar_editor_row(label: &str, route: &'static str) -> BoxedWidget {
    let router = creamui_router::use_router();
    row(
        label,
        Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "Edit contents…",
            ButtonState::Normal,
            move || crate::routes::open(&router, route),
        )),
    )
}

fn island_row(entry: KnownIsland) -> BoxedWidget {
    let mut control = Flex::row().gap(8.0).align(Align::Center);
    let route = coconut_plugin_kit::bar_editor::item_settings_route(entry.id);
    if let Some(route) = route.filter(|route| *route != "/desktop/widgets") {
        let router = creamui_router::use_router();
        control = control.child(Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "Configure…",
            ButtonState::Normal,
            move || crate::routes::open(&router, route),
        )));
    }
    row(entry.label, Box::new(control))
}

fn island_detail_page(
    size: Size,
    entry: KnownIsland,
    settings: &crate::views::desktop::island_settings::State,
) -> BoxedWidget {
    let back = {
        let router = creamui_router::use_router();
        Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "‹ Islands",
            ButtonState::Normal,
            move || crate::routes::open(&router, "/desktop/widgets"),
        )) as BoxedWidget
    };
    let page = crate::views::desktop::island_settings::build(
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
    let input: BoxedWidget = Box::new(crate::components::form_input(
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

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let island_settings = &context.island_settings;
    build(size, island_settings)
}
