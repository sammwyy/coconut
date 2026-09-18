use crate::common::{group, row, section};
use coconut_core::ShellConfig;
use coconut_plugin_tray::{TrayConfig, TrayMode, TrayVisibility};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::SegmentedControl;

const MODES: [TrayMode; 2] = [TrayMode::Grouped, TrayMode::Individual];
const VISIBILITIES: [TrayVisibility; 3] = [
    TrayVisibility::Always,
    TrayVisibility::Hidden,
    TrayVisibility::Off,
];

fn persist(tray: &TrayConfig) {
    if let Err(error) = coconut_core::modules::save_module("tray", tray) {
        eprintln!("settings: failed to save modules/tray.toml: {error}");
    }
}

/// The tray's own settings (mode, per-device visibility) live in
/// `modules/tray.toml`, not `shell.toml` — this section manages that file
/// directly instead of the shared `ShellConfig` signal every other section
/// edits. A generic, schema-free per-module editor (modeled on
/// `sections/window.rs`'s `toml::Value` path helpers) is the real design
/// for this from the dock/island/panel refactor plan; this is the minimal
/// version that keeps Settings compiling and usable against the new
/// `modules/<id>.toml` split.
pub fn build(_: Size, _config: &Signal<ShellConfig>) -> BoxedWidget {
    let tray: TrayConfig = coconut_core::modules::load_module("tray");
    let state = Signal::new(tray);

    let mode_selected = MODES
        .iter()
        .position(|mode| *mode == state.get().mode)
        .unwrap_or(0);
    let set_mode = state.clone();
    let mode_control: BoxedWidget = Box::new(
        SegmentedControl::new(mode_selected, move |index: usize| {
            set_mode.update(|t| t.mode = MODES[index]);
            persist(&set_mode.peek());
        })
        .option("One group")
        .option("Separate"),
    );

    section(
        "Status icons",
        "Choose which quick status controls are shown and how they are arranged. Changes appear after signing out and back in.",
        vec![
            group(vec![row("Mode", mode_control)]),
            group(vec![
                visibility_row("Wi-Fi", &state, |t| t.wifi, |t, v| t.wifi = v),
                visibility_row("Bluetooth", &state, |t| t.bluetooth, |t, v| t.bluetooth = v),
                visibility_row("Battery", &state, |t| t.battery, |t, v| t.battery = v),
                visibility_row("Volume", &state, |t| t.volume, |t, v| t.volume = v),
                visibility_row(
                    "Brightness",
                    &state,
                    |t| t.brightness,
                    |t, v| t.brightness = v,
                ),
            ]),
        ],
    )
}

fn visibility_row(
    label: &str,
    state: &Signal<TrayConfig>,
    get: impl Fn(&TrayConfig) -> TrayVisibility,
    set: impl Fn(&mut TrayConfig, TrayVisibility) + Copy + 'static,
) -> BoxedWidget {
    let selected = VISIBILITIES
        .iter()
        .position(|visibility| *visibility == get(&state.get()))
        .unwrap_or(0);
    let apply = state.clone();
    let control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index: usize| {
            apply.update(|t| set(t, VISIBILITIES[index]));
            persist(&apply.peek());
        })
        .option("Always")
        .option("Hidden")
        .option("Off"),
    );
    row(label, control)
}
