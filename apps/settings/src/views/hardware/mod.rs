use crate::routes::destination::Section;
use creamui_core::BoxedWidget;
use creamui_widgets::Switch;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    build(native, view)
}
pub mod keyboard;
pub mod mouse;
pub mod touchpad;

fn build(state: &State, nav: &creamui_router::Router) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    let tray: coconut_plugin_tray::TrayConfig = coconut_core::modules::load_module("tray");
    let percentage = tray.show_battery_percentage;
    let state_write = state.clone();
    let percentage_control: BoxedWidget = Box::new(Switch::new(percentage, move || {
        let mut tray: coconut_plugin_tray::TrayConfig = coconut_core::modules::load_module("tray");
        tray.show_battery_percentage = !tray.show_battery_percentage;
        let result = coconut_core::modules::save_module("tray", &tray)
            .map_err(|e| e.to_string())
            .and_then(|_| coconut_core::ipc::publish_module_config_changed());
        state_write.status.set(
            result
                .map(|_| "Battery widget updated".into())
                .unwrap_or_else(|e| format!("Could not apply: {e}")),
        );
    }));
    page(
        vec![
            card(
                "Displays & sound",
                vec![
                    detail_link(
                        "Displays",
                        collection_summary(&snapshot, "displays", "No connected displays reported"),
                        Page::Displays,
                        nav,
                    ),
                    state.preference("Brightness", "brightness", "Screen backlight"),
                    state.preference("Sound output", "sound-output", "Default playback device"),
                    state.preference("Volume", "volume", "Output volume"),
                ],
            ),
            card(
                "Input & devices",
                vec![
                    link(
                        "Mouse & touchpad",
                        "Pointer sensitivity, scrolling and gestures",
                        Section::Mouse,
                        nav,
                    ),
                    link(
                        "Keyboard",
                        "Layout, repeat and input preferences",
                        Section::Keyboard,
                        nav,
                    ),
                    detail_link(
                        "Connected devices",
                        format!("{} USB devices", snapshot.entries("devices").len()),
                        Page::Devices,
                        nav,
                    ),
                    detail_link(
                        "Printers",
                        collection_summary(
                            &snapshot,
                            "printers",
                            "Manage printers and add an IPP printer",
                        ),
                        Page::Printers,
                        nav,
                    ),
                ],
            ),
            card(
                "Power",
                vec![
                    state.preference("Power mode", "power-mode", "Performance and battery life"),
                    item(
                        "Show battery percentage",
                        "Show charge next to the battery icon",
                        percentage_control,
                        || {},
                    ),
                    state.preference(
                        "Dim screen when idle",
                        "idle-dim",
                        "Reduce backlight while idle",
                    ),
                ],
            ),
        ],
        state,
    )
}
pub mod add_printer;
pub mod devices;
pub mod displays;
pub mod printers;
