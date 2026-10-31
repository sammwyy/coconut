use super::islands::page_link;
use crate::{
    components::{group, labeled_group, row},
    views::context::ViewContext,
};
use coconut_plugin_tray::{TrayConfig, TrayMode, TrayVisibility};
use creamui_core::{BoxedWidget, Styled};
use creamui_widgets::{
    layout::Flex, Button, ButtonSize, ButtonState, ButtonVariant, SegmentedControl, Select, Switch,
    Text, TextSize,
};

type GetVisibility = fn(&TrayConfig) -> TrayVisibility;
type SetVisibility = fn(&mut TrayConfig, TrayVisibility);

pub(super) fn widget_options(context: &ViewContext, id: &str) -> BoxedWidget {
    let state = &context.island_settings;
    let tray = state.tray.get();
    let mut body = Vec::<BoxedWidget>::new();
    if id == "control_center" {
        let update = state.tray.clone();
        let mode = SegmentedControl::new(
            usize::from(tray.mode == TrayMode::Individual),
            move |index| {
                update.update(|tray| {
                    tray.mode = if index == 0 {
                        TrayMode::Grouped
                    } else {
                        TrayMode::Individual
                    }
                })
            },
        )
        .option("Together")
        .option("Separate");
        body.push(group(vec![row("Status controls", Box::new(mode))]));
        body.push(labeled_group(
            "Included controls",
            group(
                [
                    ("Wi-Fi", "/desktop/widgets/tray.wifi"),
                    ("Bluetooth", "/desktop/widgets/tray.bluetooth"),
                    ("Battery", "/desktop/widgets/tray.battery"),
                    ("Volume", "/desktop/widgets/tray.volume"),
                    ("Brightness", "/desktop/widgets/tray.brightness"),
                ]
                .into_iter()
                .map(|(label, route)| page_link(context, label, "Configure this control", route))
                .collect(),
            ),
        ));
    } else if let Some((get, set)) = visibility(id) {
        let options = [
            TrayVisibility::Always,
            TrayVisibility::Hidden,
            TrayVisibility::Off,
        ];
        let selected = options
            .iter()
            .position(|option| *option == get(&tray))
            .unwrap_or(0);
        let update = state.tray.clone();
        let control = Select::controlled(
            &["Bar and panel", "Panel only", "Off"],
            context.native.select(id, selected),
        )
        .width(160.0)
        .on_select(move |index| update.update(|tray| set(tray, options[index])));
        let mut rows = vec![row("Show in", Box::new(control))];
        if id == "tray.battery" {
            let update = state.tray.clone();
            rows.push(row(
                "Battery percentage",
                Box::new(Switch::new(tray.show_battery_percentage, move || {
                    update
                        .update(|tray| tray.show_battery_percentage = !tray.show_battery_percentage)
                })),
            ));
        }
        body.push(group(rows));
        let (label, route) = match id {
            "tray.wifi" => ("Network settings", "/connectivity/network"),
            "tray.bluetooth" => ("Bluetooth settings", "/connectivity/bluetooth"),
            "tray.volume" => ("Sound settings", "/hardware"),
            "tray.brightness" => ("Display settings", "/hardware/displays"),
            _ => ("Power settings", "/hardware"),
        };
        body.push(group(vec![
            page_link(
                context,
                label,
                "Configure the device and system behavior",
                route,
            ),
            page_link(
                context,
                "Control Center",
                "Choose whether controls appear together or separately",
                "/desktop/widgets/control_center",
            ),
        ]));
    }
    let save = state.clone();
    let window = context.window.clone();
    body.push(Box::new(Flex::column().gap(10.0)
        .child(Box::new(Text::secondary("Separate controls require Separate mode in Control Center. Apply changes, then start a new session.").size(TextSize::Sm)))
        .child(Box::new(Button::styled(ButtonVariant::Primary, ButtonSize::Sm, "Apply changes", ButtonState::Normal, move || { if let Some(window) = window.borrow().as_ref() { save.save_tray(window); } }).disabled(state.busy.get() || tray == state.saved_tray.get())))));
    Box::new(Flex::column().gap(18.0).with_children(body))
}

fn visibility(id: &str) -> Option<(GetVisibility, SetVisibility)> {
    match id {
        "tray.wifi" => Some((|tray| tray.wifi, |tray, value| tray.wifi = value)),
        "tray.bluetooth" => Some((|tray| tray.bluetooth, |tray, value| tray.bluetooth = value)),
        "tray.battery" => Some((|tray| tray.battery, |tray, value| tray.battery = value)),
        "tray.volume" => Some((|tray| tray.volume, |tray, value| tray.volume = value)),
        "tray.brightness" => Some((
            |tray| tray.brightness,
            |tray, value| tray.brightness = value,
        )),
        _ => None,
    }
}
