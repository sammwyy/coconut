use coconut_plugin_kit::chrome::{
    compact_switch, fat_slider, icon_badge, shell_accent, shell_border, shell_control_hover,
    shell_fill, shell_muted, shell_on_accent, shell_panel, shell_selected, shell_text, shell_track,
    ISLAND_RADIUS,
};
use creamui_core::layout::Style as LayoutStyle;
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_widgets::{
    layout::{fixed, Align, Flex, Justify},
    ProgressBar, RawButton, RawMarquee, RawSlider, RawText,
};
use std::rc::Rc;

pub(super) fn slider_tile(
    size: Size,
    icon: &str,
    label: &str,
    value: f32,
    text: String,
    on_open: Rc<dyn Fn()>,
    on_change: impl Fn(f32) + 'static,
) -> BoxedWidget {
    if size.height > size.width + 25.0 {
        let padding = 8.0;
        let bar_height = (size.height - 102.0).max(60.0);
        let open =
            RawButton::new(Style::new().corner_radius(6.0), move || on_open()).child(Box::new(
                Flex::row()
                    .justify(Justify::Center)
                    .child(icon_badge(icon, 22.0)),
            ));
        return Box::new(
            super::control_editor::container(size)
                .padding(padding)
                .gap(6.0)
                .child(Box::new(open))
                .child(Box::new(RawMarquee::new(
                    label,
                    shell_muted(),
                    10.0,
                    (size.width - 18.0).max(1.0),
                )))
                .child(Box::new(
                    Flex::row()
                        .justify(Justify::Center)
                        .grow(1.0)
                        .child(Box::new(super::level_control::LevelControl::new(
                            value,
                            Size {
                                width: (size.width - 28.0).clamp(28.0, 52.0),
                                height: bar_height,
                            },
                            super::level_control::LevelShape::Vertical,
                            on_change,
                        ))),
                ))
                .child(center_text(
                    &text,
                    if size.width < 110.0 { 18.0 } else { 25.0 },
                )),
        );
    }
    if size.width < 115.0 {
        let diameter = (size.width - 16.0).clamp(40.0, 62.0);
        let open =
            RawButton::new(Style::new().corner_radius(6.0), move || on_open()).child(Box::new(
                RawMarquee::new(label, shell_muted(), 10.0, (size.width - 16.0).max(1.0)),
            ));
        return Box::new(
            super::control_editor::container(size)
                .padding(6.0)
                .gap(4.0)
                .child(Box::new(open))
                .child(Box::new(Flex::row().justify(Justify::Center).child(
                    Box::new(super::level_control::LevelControl::new(
                        value,
                        Size {
                            width: diameter,
                            height: diameter,
                        },
                        super::level_control::LevelShape::Dial,
                        on_change,
                    )),
                ))),
        );
    }
    let compact = size.width < 120.0;
    let tall = size.height > 150.0;
    let padding = if compact { 6.0 } else { 12.0 };
    let inner = size.width - padding * 2.0;
    if !compact && !tall {
        return Box::new(
            super::control_editor::container(size)
                .padding(padding)
                .justify(Justify::Center)
                .child(fat_slider(
                    icon,
                    label,
                    value,
                    text,
                    inner,
                    Some(on_open),
                    on_change,
                )),
        );
    }
    let mut header = Flex::row()
        .gap(6.0)
        .align(Align::Center)
        .child(icon_badge(icon, if tall { 32.0 } else { 20.0 }));
    if !compact {
        header = header.child(text_widget(label, 13.0, false));
    }
    header = header.child(Box::new(Flex::row().grow(1.0)));
    if !tall {
        header = header.child(text_widget(&text, 12.0, true));
    }
    let open = RawButton::new(
        Style::new()
            .corner_radius(6.0)
            .hover(StateStyle::new().background(shell_control_hover())),
        move || on_open(),
    )
    .child(Box::new(header));
    let mut content = super::control_editor::container(size)
        .padding(padding)
        .gap(if tall { 10.0 } else { 4.0 })
        .child(Box::new(open));
    if tall {
        content = content.child(Box::new(
            Flex::column()
                .grow(1.0)
                .justify(Justify::Center)
                .gap(6.0)
                .child(text_widget(&text, if compact { 26.0 } else { 38.0 }, true))
                .child(text_widget(
                    if compact {
                        label
                    } else if label == "Brightness" {
                        "Display level"
                    } else {
                        "System output"
                    },
                    11.0,
                    false,
                )),
        ));
    } else {
        content = content.child(text_widget(label, 11.0, false));
    }
    content = content.child(Box::new(
        RawSlider::new(
            Style::new().layout(LayoutStyle {
                size: fixed(inner, if tall { 40.0 } else { 24.0 }),
                ..Default::default()
            }),
            value,
            shell_track(),
            shell_fill(),
            shell_on_accent(),
            on_change,
        )
        .track(if tall { 12.0 } else { 6.0 }, if tall { 6.0 } else { 3.0 })
        .handle(if tall { 22.0 } else { 14.0 }, 11.0),
    ));
    if tall && !compact {
        content = content.child(Box::new(
            Flex::row()
                .justify(Justify::Between)
                .child(text_widget("0", 10.0, false))
                .child(text_widget("100%", 10.0, false)),
        ));
    }
    Box::new(content)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn device_tile(
    size: Size,
    icon: &str,
    label: &str,
    name: String,
    caption: &str,
    enabled: bool,
    on_toggle: Rc<dyn Fn()>,
    on_open: Rc<dyn Fn()>,
) -> BoxedWidget {
    let compact = size.width < 150.0;
    let tall = size.height > 150.0;
    let padding = if compact { 6.0 } else { 12.0 };
    let mut header = Flex::row()
        .gap(if compact { 3.0 } else { 6.0 })
        .align(Align::Center);
    if size.width >= 74.0 {
        header = header.child(icon_badge(
            icon,
            if compact {
                16.0
            } else if tall {
                30.0
            } else {
                24.0
            },
        ));
    }
    if !compact {
        header = header.child(text_widget(label, 11.0, false));
    }
    header = header
        .child(Box::new(Flex::row().grow(1.0)))
        .child(compact_switch(enabled, on_toggle));
    let mut details = Flex::column().gap(if tall { 8.0 } else { 5.0 });
    if compact {
        details = details.child(text_widget(
            label,
            if size.width < 74.0 { 10.0 } else { 11.0 },
            false,
        ));
    }
    details = details.child(Box::new(
        RawMarquee::new(
            name,
            shell_text(),
            if tall && !compact { 20.0 } else { 13.0 },
            size.width - padding * 2.0,
        )
        .bold(true),
    ));
    if tall || !compact {
        details = details.child(text_widget(caption, 11.0, false));
    }
    if tall {
        details = details.grow(1.0).justify(Justify::Center);
    }
    let mut content = Flex::column()
        .size(size.width, size.height)
        .padding(padding)
        .gap(8.0)
        .child(Box::new(header))
        .child(Box::new(details));
    if tall && !compact {
        content = content.child(settings_hint(if label == "Wi-Fi" {
            "Network settings"
        } else {
            "Bluetooth settings"
        }));
    }
    Box::new(
        RawButton::new(active_style(size, enabled), move || on_open()).child(Box::new(content)),
    )
}

pub(super) fn battery_tile(
    size: Size,
    icon: &str,
    percentage: Option<u8>,
    caption: &str,
    on_open: Rc<dyn Fn()>,
) -> BoxedWidget {
    let compact = size.width < 120.0;
    let tall = size.height > 150.0;
    let padding = if compact { 6.0 } else { 12.0 };
    if compact {
        let mut body = Flex::column()
            .size(size.width, size.height)
            .padding(6.0)
            .gap(5.0)
            .align(Align::Center)
            .justify(Justify::Center)
            .child(center_text("Battery", 10.0));
        if let Some(percent) = percentage {
            body = body.child(Box::new(super::charge_meter::ChargeMeter::new(
                percent,
                (size.width - 16.0).clamp(40.0, 62.0),
            )));
        } else {
            body = body
                .child(icon_badge(icon, 30.0))
                .child(center_text("—", 13.0));
        }
        if tall && !caption.is_empty() {
            body = body.child(center_text(caption.trim_matches(['(', ')']), 10.0));
        }
        return Box::new(RawButton::new(tile_style(size), move || on_open()).child(Box::new(body)));
    }
    let value = percentage.map_or_else(|| "—".into(), |percent| format!("{percent}%"));
    let mut content = Flex::column()
        .size(size.width, size.height)
        .padding(padding)
        .gap(8.0)
        .child(Box::new(
            Flex::row()
                .gap(6.0)
                .align(Align::Center)
                .child(icon_badge(icon, if tall && !compact { 32.0 } else { 20.0 }))
                .child(text_widget(
                    "Battery",
                    if compact { 10.0 } else { 11.0 },
                    false,
                )),
        ));
    let mut details =
        Flex::column()
            .gap(8.0)
            .child(text_widget(&value, if tall { 32.0 } else { 20.0 }, true));
    if tall || !compact {
        let status = if caption.is_empty() {
            "On battery"
        } else {
            caption.trim_matches(['(', ')'])
        };
        details = details.child(text_widget(status, 11.0, false));
    }
    if tall {
        details = details.grow(1.0).justify(Justify::Center);
    }
    content = content.child(Box::new(details));
    if let Some(percent) = percentage {
        content = content.child(Box::new(
            ProgressBar::new(f32::from(percent) / 100.0).with_style(LayoutStyle {
                size: fixed(size.width - padding * 2.0, if tall { 8.0 } else { 4.0 }),
                ..Default::default()
            }),
        ));
    }
    if tall && !compact {
        content = content.child(settings_hint("Power settings"));
    }
    Box::new(RawButton::new(tile_style(size), move || on_open()).child(Box::new(content)))
}

pub(super) fn awake_tile(size: Size, awake: bool, toggle_awake: Rc<dyn Fn()>) -> BoxedWidget {
    let compact = size.width < 120.0;
    let tall = size.height > 150.0;
    let padding = if compact { 6.0 } else { 12.0 };
    let mut header = Flex::row().gap(4.0).align(Align::Center);
    if size.width >= 74.0 {
        header = header.child(icon_badge(
            "monitor",
            if compact {
                16.0
            } else if tall {
                30.0
            } else {
                20.0
            },
        ));
    }
    if !compact {
        header = header.child(text_widget("Awake", 11.0, false));
    }
    header = header
        .child(Box::new(Flex::row().grow(1.0)))
        .child(compact_switch(awake, toggle_awake));
    let mut details = Flex::column().gap(6.0);
    if compact {
        details = details.child(text_widget(
            "Keep awake",
            if size.width < 74.0 { 10.0 } else { 11.0 },
            false,
        ));
    }
    details = details.child(text_widget(
        if awake { "On" } else { "Off" },
        if tall { 28.0 } else { 15.0 },
        true,
    ));
    if tall || !compact {
        details = details.child(text_widget(
            if awake {
                "Screen on"
            } else if compact {
                "Auto sleep"
            } else {
                "Automatic sleep"
            },
            11.0,
            false,
        ));
    }
    if tall {
        details = details.grow(1.0).justify(Justify::Center);
    }
    Box::new(
        super::control_editor::container(size)
            .padding(padding)
            .gap(8.0)
            .background(if awake {
                shell_panel().mix(shell_selected(), 0.35)
            } else {
                shell_panel()
            })
            .child(Box::new(header))
            .child(Box::new(details)),
    )
}

pub(super) fn tile_style(size: Size) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(size.width, size.height),
            ..Default::default()
        })
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(ISLAND_RADIUS)
        .box_shadow(coconut_plugin_kit::design::card_shadow())
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
        .focus(StateStyle::new().outline(shell_accent(), 2.0))
}

fn active_style(size: Size, active: bool) -> Style {
    if active {
        tile_style(size)
            .background(shell_panel().mix(shell_selected(), 0.35))
            .border(shell_border().mix(shell_accent(), 0.35), 1.0)
    } else {
        tile_style(size)
    }
}

fn text_widget(text: &str, size: f32, primary: bool) -> BoxedWidget {
    Box::new(RawText::new(
        text.to_owned(),
        if primary { shell_text() } else { shell_muted() },
        size,
    ))
}

fn settings_hint(label: &str) -> BoxedWidget {
    Box::new(
        Flex::row()
            .justify(Justify::Between)
            .child(text_widget(label, 11.0, false))
            .child(text_widget("›", 15.0, false)),
    )
}

fn center_text(text: &str, size: f32) -> BoxedWidget {
    Box::new(
        RawText::new(text.to_owned(), shell_text(), size)
            .text_align(creamui_core::TextAlign::Center),
    )
}
