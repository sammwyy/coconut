use crate::common::{section, update_config};
use coconut_core::ShellConfig;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::Align;
use creamui_widgets::{Switch, Text, TextSize};

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let widgets = config.get().widgets;
    let rows = vec![
        toggle_row(
            "Weather",
            widgets.weather.enabled,
            config,
            |c| &mut c.widgets.weather.enabled,
        ),
        toggle_row(
            "Now playing",
            widgets.current_playing.enabled,
            config,
            |c| &mut c.widgets.current_playing.enabled,
        ),
        toggle_row(
            "App launcher",
            widgets.app_launcher.enabled,
            config,
            |c| &mut c.widgets.app_launcher.enabled,
        ),
        toggle_row(
            "Control center",
            widgets.control_center.enabled,
            config,
            |c| &mut c.widgets.control_center.enabled,
        ),
        toggle_row("Clock", widgets.clock.enabled, config, |c| {
            &mut c.widgets.clock.enabled
        }),
        clock_format_row(&widgets.clock.format, config),
    ];
    section(
        "Widgets",
        "Which widgets appear in the bar, and the clock's time format.",
        rows,
    )
}

fn toggle_row(
    label: &str,
    checked: bool,
    config: &Signal<ShellConfig>,
    field: impl Fn(&mut ShellConfig) -> &mut bool + Copy + 'static,
) -> BoxedWidget {
    let theme = use_theme();
    let apply = config.clone();
    let switch: BoxedWidget = Box::new(Switch::new(checked, move || {
        update_config(&apply, |c| *field(c) = !*field(c));
    }));
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={theme.spacing_medium}>
            {switch}
            {Box::new(Text::new(label.to_owned()).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    })
}

fn clock_format_row(format: &str, config: &Signal<ShellConfig>) -> BoxedWidget {
    let theme = use_theme();
    let apply = config.clone();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_small}>
            {Box::new(Text::new("Clock format (strftime)".to_owned()).size(TextSize::Sm)) as BoxedWidget}
            <TextInput value={format.to_owned()} on_change={Box::new(move |value: String| {
                update_config(&apply, |c| c.widgets.clock.format = value);
            }) as Box<dyn Fn(String)>} />
        </Flex>
    })
}
