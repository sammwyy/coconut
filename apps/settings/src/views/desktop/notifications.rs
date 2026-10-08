use crate::components::{group, row, section, section_label, update_config};
use coconut_core::{PopupAlign, PopupConfig, PopupEdge, ShellConfig};
use creamui_core::{BoxedWidget, Styled};
use creamui_reactive::Signal;
use creamui_widgets::{
    layout::{Align, Flex},
    SegmentedControl, Slider, Switch, Text, TextSize,
};

#[derive(Clone, Copy)]
enum Kind {
    Status,
    Notifications,
}

impl Kind {
    fn get(self, config: &ShellConfig) -> &PopupConfig {
        match self {
            Self::Status => &config.status_updates,
            Self::Notifications => &config.notifications,
        }
    }

    fn get_mut(self, config: &mut ShellConfig) -> &mut PopupConfig {
        match self {
            Self::Status => &mut config.status_updates,
            Self::Notifications => &mut config.notifications,
        }
    }
}

pub fn build(config: &Signal<ShellConfig>) -> BoxedWidget {
    let card = |kind, title, description| -> BoxedWidget {
        Box::new(
            Flex::column()
                .gap(12.0)
                .child(section_label(title))
                .child(Box::new(Text::secondary(description).size(TextSize::Sm)))
                .child(controls(config, kind)),
        )
    };
    section("Notifications & popups", "", vec![
        card(Kind::Status, "Status updates", "Volume and brightness changes appear briefly over the desktop."),
        card(Kind::Notifications, "Notifications", "Messages from applications. Apps can request their own duration or keep a message visible."),
        battery_alerts(config),
    ])
}

fn battery_alerts(config: &Signal<ShellConfig>) -> BoxedWidget {
    let current = config.get().battery_alerts;
    let threshold = |label: &'static str, percent: u8, on_change: Box<dyn Fn(f32)>| {
        row(
            label,
            Box::new(
                Flex::row()
                    .align(Align::Center)
                    .gap(12.0)
                    .child(Box::new(Text::secondary(format!("{percent}%"))))
                    .child(Box::new(
                        Slider::new((percent as f32 / 100.0).clamp(0.01, 1.0), on_change)
                            .width(160.0),
                    )),
            ),
        )
    };
    let low_config = config.clone();
    let low = threshold(
        "Low battery",
        current.low_percent,
        Box::new(move |value| {
            update_config(&low_config, |config| {
                config.battery_alerts.low_percent = ((value * 100.0).round() as u8)
                    .clamp(config.battery_alerts.critical_percent, 100);
            });
        }),
    );
    let critical_config = config.clone();
    let critical = threshold(
        "Critical battery",
        current.critical_percent,
        Box::new(move |value| {
            update_config(&critical_config, |config| {
                config.battery_alerts.critical_percent =
                    ((value * 100.0).round() as u8).clamp(1, config.battery_alerts.low_percent);
            });
        }),
    );
    Box::new(
        Flex::column()
            .gap(12.0)
            .child(section_label("Battery alerts"))
            .child(Box::new(Text::secondary(
                "Shown once as charge falls below each level while the device is unplugged.",
            )))
            .child(group(vec![low, critical])),
    )
}

fn controls(config: &Signal<ShellConfig>, kind: Kind) -> BoxedWidget {
    let current = kind.get(&config.get()).clone();
    let enabled = {
        let config = config.clone();
        Box::new(Switch::new(current.enabled, move || {
            update_config(&config, |c| kind.get_mut(c).enabled = !kind.get(c).enabled);
        })) as BoxedWidget
    };
    let edge = {
        let config = config.clone();
        Box::new(
            SegmentedControl::new(
                usize::from(current.edge == PopupEdge::Bottom),
                move |index| {
                    update_config(&config, |c| {
                        kind.get_mut(c).edge = [PopupEdge::Top, PopupEdge::Bottom][index]
                    });
                },
            )
            .option("Top")
            .option("Bottom"),
        ) as BoxedWidget
    };
    let alignment = {
        let config = config.clone();
        let values = [PopupAlign::Left, PopupAlign::Center, PopupAlign::Right];
        let selected = values.iter().position(|v| *v == current.align).unwrap_or(1);
        Box::new(
            SegmentedControl::new(selected, move |index| {
                update_config(&config, |c| kind.get_mut(c).align = values[index]);
            })
            .option("Left")
            .option("Center")
            .option("Right"),
        ) as BoxedWidget
    };
    let offset = {
        let config = config.clone();
        Box::new(
            Flex::row()
                .align(Align::Center)
                .gap(12.0)
                .child(Box::new(Text::secondary(format!(
                    "{:.0} px",
                    current.offset
                ))))
                .child(Box::new(
                    Slider::new((current.offset / 240.0).clamp(0.0, 1.0), move |value| {
                        update_config(&config, |c| {
                            kind.get_mut(c).offset = (value * 240.0).round()
                        });
                    })
                    .width(160.0),
                )),
        ) as BoxedWidget
    };
    let duration = {
        let config = config.clone();
        Box::new(
            Flex::row()
                .align(Align::Center)
                .gap(12.0)
                .child(Box::new(Text::secondary(format!(
                    "{:.1} s",
                    current.duration_ms as f32 / 1000.0
                ))))
                .child(Box::new(
                    Slider::new(
                        ((current.duration_ms as f32 - 500.0) / 9500.0).clamp(0.0, 1.0),
                        move |value| {
                            update_config(&config, |c| {
                                kind.get_mut(c).duration_ms =
                                    (500.0 + value * 9500.0).round() as u32
                            });
                        },
                    )
                    .width(160.0),
                )),
        ) as BoxedWidget
    };
    group(vec![
        row("Enabled", enabled),
        row("Screen edge", edge),
        row("Alignment", alignment),
        row("Edge offset", offset),
        row("Duration", duration),
    ])
}

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    build(&context.config)
}
