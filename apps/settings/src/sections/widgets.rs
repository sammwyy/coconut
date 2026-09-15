use crate::common::{group, row, section, update_config};
use coconut_core::ShellConfig;
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::{Switch, TextInput};

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let widgets = config.get().widgets;
    let toggles = group(vec![
        toggle_row("Weather", widgets.weather.enabled, config, |c| {
            &mut c.widgets.weather.enabled
        }),
        toggle_row(
            "Now playing",
            widgets.current_playing.enabled,
            config,
            |c| &mut c.widgets.current_playing.enabled,
        ),
        toggle_row("App launcher", widgets.app_launcher.enabled, config, |c| {
            &mut c.widgets.app_launcher.enabled
        }),
        toggle_row(
            "Control center",
            widgets.control_center.enabled,
            config,
            |c| &mut c.widgets.control_center.enabled,
        ),
        toggle_row("Clock", widgets.clock.enabled, config, |c| {
            &mut c.widgets.clock.enabled
        }),
    ]);
    let format = group(vec![clock_format_row(&widgets.clock.format, config)]);

    section(
        "Widgets",
        "Which widgets appear in the bar, and the clock's time format.",
        vec![toggles, format],
    )
}

fn toggle_row(
    label: &str,
    checked: bool,
    config: &Signal<ShellConfig>,
    field: impl Fn(&mut ShellConfig) -> &mut bool + Copy + 'static,
) -> BoxedWidget {
    let apply = config.clone();
    let switch: BoxedWidget = Box::new(Switch::new(checked, move || {
        update_config(&apply, |c| *field(c) = !*field(c));
    }));
    row(label, switch)
}

fn clock_format_row(format: &str, config: &Signal<ShellConfig>) -> BoxedWidget {
    let apply = config.clone();
    let input: BoxedWidget = Box::new(TextInput::new(format.to_owned(), move |value: String| {
        update_config(&apply, |c| c.widgets.clock.format = value);
    }));
    row("Clock format (strftime)", input)
}
