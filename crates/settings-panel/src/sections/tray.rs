use crate::common::{section, update_config};
use coconut_core::{ShellConfig, TrayConfig, TrayMode, TrayVisibility};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::{SegmentedControl, Text, TextSize};

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

    let rows = vec![
        labeled_row("Mode", mode_control),
        visibility_row("Wi-Fi", config, |t| t.wifi, |t, v| t.wifi = v),
        visibility_row(
            "Bluetooth",
            config,
            |t| t.bluetooth,
            |t, v| t.bluetooth = v,
        ),
        visibility_row("Battery", config, |t| t.battery, |t, v| t.battery = v),
        visibility_row("Volume", config, |t| t.volume, |t, v| t.volume = v),
        visibility_row(
            "Brightness",
            config,
            |t| t.brightness,
            |t, v| t.brightness = v,
        ),
    ];

    section(
        "Tray",
        "How status icons open their controls, and which ones show in the bar.",
        rows,
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
    labeled_row(label, control)
}

fn labeled_row(label: &str, control: BoxedWidget) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_small}>
            {Box::new(Text::new(label.to_owned()).size(TextSize::Sm)) as BoxedWidget}
            {control}
        </Flex>
    })
}
