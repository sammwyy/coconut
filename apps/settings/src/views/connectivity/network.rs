use crate::components::{self, group, labeled_group, section};
use crate::icons::SettingsIcons;
use coconut_api::network::NetworkIntegration;
use coconut_api::settings::{Action, Value};
use creamui_core::layout::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, Rect, Style,
};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::{IconSource, RawButton, RawView, Text, TextInput, TextSize};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct State {
    editing_hostname: Signal<bool>,
    diagnostics: Signal<bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            editing_hostname: Signal::new(false),
            diagnostics: Signal::new(false),
        }
    }
}

fn action(label: &str, enabled: bool, click: impl Fn() + 'static) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    Box::new(
        RawButton::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(34.0),
                },
                flex_shrink: 0.0,
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::Center),
                padding: Rect {
                    left: LengthPercentage::Length(14.0),
                    right: LengthPercentage::Length(14.0),
                    top: LengthPercentage::Length(0.0),
                    bottom: LengthPercentage::Length(0.0),
                },
                ..Default::default()
            },
            click,
        )
        .background(theme.colors.surface_elevated)
        .outline(theme.colors.border, 1.0)
        .corner_radius(14.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .disabled(!enabled)
        .child(Box::new(Text::new(label).size(TextSize::Sm).color(
            if enabled {
                theme.colors.text_primary
            } else {
                theme.colors.text_secondary
            },
        ))),
    )
}

fn row(
    label: &str,
    hint: impl Into<String>,
    icon: IconSource,
    control: BoxedWidget,
) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} style={Style {
            size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
            min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(56.0) },
            flex_shrink: 0.0,
            padding: Rect { left: LengthPercentage::Length(16.0), right: LengthPercentage::Length(16.0), top: LengthPercentage::Length(10.0), bottom: LengthPercentage::Length(10.0) },
            ..Default::default()
        }}>
            {components::icon_badge(icon, components::category_color())}
            <Flex direction={FlexDirection::Column} grow={1.0} shrink={1.0} gap={2.0} style={Style {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                gap: creamui_core::layout::Size { width: LengthPercentage::Length(0.0), height: LengthPercentage::Length(2.0) },
                min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Auto },
                ..Default::default()
            }}>
                {Box::new(Text::new(label)) as BoxedWidget}
                {Box::new(Text::secondary(hint.into()).size(TextSize::Sm)) as BoxedWidget}
            </Flex>
            {control}
        </Flex>
    })
}

pub(super) fn build(
    network: Rc<dyn NetworkIntegration>,
    state: &State,
    icons: &SettingsIcons,
) -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let native = &context.native;
    let snapshot = native.snapshot.get();
    let writable = snapshot
        .preferences
        .get("hostname")
        .is_some_and(|pref| pref.writable);
    let busy = native.busy.get();
    let edit = state.editing_hostname.clone();
    let hostname_input = native.hostname.clone();
    let hostname = snapshot.fact("hostname").to_owned();
    let initial_hostname = hostname.clone();
    let mut system_rows = vec![
        row(
            "Hostname",
            hostname,
            icons.computer.clone(),
            action("Edit", writable && !busy, move || {
                hostname_input.set_value(&initial_hostname);
                edit.set(!edit.peek());
            }),
        ),
        row(
            "Connection priority",
            "Managed by the system",
            icons.network.clone(),
            action("Configure", false, || {}),
        ),
    ];
    if state.editing_hostname.get() {
        let write = native.clone();
        system_rows.push(Box::new(jsx! {
            <Flex direction={FlexDirection::Column} gap={12.0} padding={16.0}>
                {Box::new(components::form_input(TextInput::controlled(&native.hostname).placeholder("Hostname"), 260.0)) as BoxedWidget}
                <Flex direction={FlexDirection::Row} gap={8.0}>
                    {action("Save", !busy, move || write.apply(Action::Set { key: "hostname".into(), value: Value::Text(write.hostname.value()) }))}
                    {action("Cancel", !busy, {
                        let edit = state.editing_hostname.clone();
                        let native = native.clone();
                        move || {
                            native.hostname.set_value(native.snapshot.peek().fact("hostname"));
                            edit.set(false);
                        }
                    })}
                </Flex>
                {Box::new(Text::secondary("Changing the hostname requires authorization.").size(TextSize::Sm)) as BoxedWidget}
            </Flex>
        }));
    }

    // The network API exposes interface addresses, but not DNS policy, TLS,
    // search domains, connection priority or proxy preferences. Do not invent
    // automatic/off values or offer controls that silently do nothing.
    let mut body = vec![
        labeled_group("System network", group(system_rows)),
        labeled_group(
            "DNS",
            group(vec![
                row(
                    "DNS mode",
                    "Managed by the system",
                    icons.server.clone(),
                    action("Configure", false, || {}),
                ),
                row(
                    "DNS-over-TLS",
                    "Not available yet",
                    icons.lock.clone(),
                    action("Configure", false, || {}),
                ),
                row(
                    "Search domains",
                    "Not available yet",
                    icons.server.clone(),
                    action("Edit", false, || {}),
                ),
            ]),
        ),
        labeled_group(
            "Proxy",
            group(vec![
                row(
                    "Proxy configuration",
                    "Not available yet",
                    icons.system.clone(),
                    action("Configure", false, || {}),
                ),
                row(
                    "Proxy bypass list",
                    "Not available yet",
                    icons.bypass.clone(),
                    action("Edit", false, || {}),
                ),
            ]),
        ),
        labeled_group(
            "Diagnostics",
            group(vec![
                row(
                    "Network diagnostics",
                    "Check interface, gateway and DNS configuration",
                    icons.about.clone(),
                    action("Run checks", true, {
                        let show = state.diagnostics.clone();
                        move || show.set(!show.peek())
                    }),
                ),
                row(
                    "Connection logs",
                    "Not available yet",
                    icons.computer.clone(),
                    action("View logs", false, || {}),
                ),
            ]),
        ),
    ];
    if state.diagnostics.get() {
        let devices = network.devices();
        let mut results = Vec::new();
        for device in devices.iter().filter(|device| device.active) {
            results.push(components::setting_row::key_value(
                "Interface",
                &device.interface,
            ));
            results.push(components::setting_row::key_value(
                "IPv4 address",
                device.ip_address.as_deref().unwrap_or("Not assigned"),
            ));
            results.push(components::setting_row::key_value(
                "Gateway",
                device.gateway.as_deref().unwrap_or("Not assigned"),
            ));
            results.push(components::setting_row::key_value(
                "DNS servers",
                if device.dns.is_empty() {
                    "Not reported".into()
                } else {
                    device.dns.join(" · ")
                },
            ));
        }
        if results.is_empty() {
            results.push(components::setting_row::key_value(
                "Connection",
                "No active network interface",
            ));
        }
        body.push(labeled_group("Connection checks", group(results)));
        body.push(Box::new(
            Text::secondary(
                "These checks show the current configuration; internet reachability is not tested.",
            )
            .size(TextSize::Sm),
        ));
    }
    let status = native.status.get();
    if !status.is_empty() {
        body.push(Box::new(Text::secondary(status).size(TextSize::Sm)));
    }
    Box::new(
        RawView::new(Style {
            flex_direction: FlexDirection::Column,
            align_self: Some(creamui_core::layout::AlignSelf::Center),
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            max_size: creamui_core::layout::Size {
                width: Dimension::Length(690.0),
                height: Dimension::Auto,
            },
            ..Default::default()
        })
        .child(section("", "", body)),
    )
}
