use crate::common::{group, row, section, update_config};
use coconut_core::{ShellConfig, TrayConfig, TrayMode, TrayVisibility};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::SegmentedControl;

const MODES: [TrayMode; 2] = [TrayMode::Grouped, TrayMode::Individual];
const VISIBILITIES: [TrayVisibility; 3] = [
    TrayVisibility::Always,
    TrayVisibility::Hidden,
    TrayVisibility::Off,
];

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let mode_selected = MODES
        .iter()
        .position(|mode| *mode == config.get().tray.mode)
        .unwrap_or(0);
    let set_mode = config.clone();
    let mode_control: BoxedWidget = Box::new(
        SegmentedControl::new(mode_selected, move |index: usize| {
            update_config(&set_mode, |c| c.tray.mode = MODES[index]);
        })
        .option("Grouped")
        .option("Individual"),
    );

    section(
        "Tray",
        "How status icons open their controls, and which ones show in the bar.",
        vec![
            group(vec![row("Mode", mode_control)]),
            group(vec![
                visibility_row("Wi-Fi", config, |t| t.wifi, |t, v| t.wifi = v),
                visibility_row("Bluetooth", config, |t| t.bluetooth, |t, v| t.bluetooth = v),
                visibility_row("Battery", config, |t| t.battery, |t, v| t.battery = v),
                visibility_row("Volume", config, |t| t.volume, |t, v| t.volume = v),
                visibility_row(
                    "Brightness",
                    config,
                    |t| t.brightness,
                    |t, v| t.brightness = v,
                ),
            ]),
        ],
    )
}

fn visibility_row(
    label: &str,
    config: &Signal<ShellConfig>,
    get: impl Fn(&TrayConfig) -> TrayVisibility,
    set: impl Fn(&mut TrayConfig, TrayVisibility) + Copy + 'static,
) -> BoxedWidget {
    let selected = VISIBILITIES
        .iter()
        .position(|visibility| *visibility == get(&config.get().tray))
        .unwrap_or(0);
    let apply = config.clone();
    let control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index: usize| {
            update_config(&apply, |c| set(&mut c.tray, VISIBILITIES[index]));
        })
        .option("Always")
        .option("Hidden")
        .option("Off"),
    );
    row(label, control)
}
