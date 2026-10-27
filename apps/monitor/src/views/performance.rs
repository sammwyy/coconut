use super::*;

struct Device {
    resource: Resource,
    title: String,
    summary: String,
    detail: String,
    color: Color,
    ceiling: f32,
}
fn devices(s: &Snapshot) -> Vec<Device> {
    let mut devices = vec![
        Device {
            resource: Resource::Cpu,
            title: "CPU".into(),
            summary: cpu_value(s),
            detail: [
                s.cpu
                    .frequency_mhz
                    .map(|v| format!("{:.2} GHz", v / 1000.0)),
                system::cpu_temperature(s).map(|v| format!("{v:.0} °C")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
            color: CPU_COLOR,
            ceiling: 100.0,
        },
        Device {
            resource: Resource::Memory,
            title: "Memory".into(),
            summary: format!(
                "{:.0}%",
                percent(
                    s.memory.total.saturating_sub(s.memory.available),
                    s.memory.total
                )
            ),
            detail: format!(
                "{} / {}",
                system::bytes(s.memory.total.saturating_sub(s.memory.available)),
                system::bytes(s.memory.total)
            ),
            color: MEMORY_COLOR,
            ceiling: 100.0,
        },
    ];
    for d in &s.drives {
        devices.push(Device {
            resource: Resource::Disk(d.name.clone()),
            title: format!("Disk ({})", d.name),
            summary: if d.sampled {
                format!("{:.0}%", d.usage)
            } else {
                "—".into()
            },
            detail: d.kind.clone(),
            color: DISK_COLOR,
            ceiling: 100.0,
        });
    }
    for g in &s.gpus {
        devices.push(Device {
            resource: Resource::Gpu(g.id.clone()),
            title: format!("GPU ({})", g.id),
            summary: g
                .usage
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "No usage counter".into()),
            detail: g.name.clone(),
            color: GPU_COLOR,
            ceiling: 100.0,
        });
    }
    for n in &s.network {
        devices.push(Device {
            resource: Resource::Network(n.name.clone()),
            title: n.name.clone(),
            summary: format!("R: {}", rate(n.receive_rate)),
            detail: format!("S: {}", rate(n.transmit_rate)),
            color: RX_COLOR,
            ceiling: 0.0,
        });
    }
    devices
}

pub(super) fn build(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let t = use_theme();
    let devices = devices(&s);
    let selection = state.resource.get();
    let selected = devices
        .iter()
        .find(|device| device.resource == selection)
        .unwrap_or(&devices[0]);
    let rail_width = if size.width < 800.0 { 172.0 } else { 206.0 };
    let mut rows = vec![];
    for device in &devices {
        let selection = state.resource.clone();
        let resource = device.resource.clone();
        let samples = state.series(&resource.key());
        let active = resource == selected.resource;
        rows.push(Box::new(
            RawButton::new(
                pad(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Length(76.0),
                        },
                        flex_shrink: 0.0,
                        ..row(10.0)
                    },
                    10.0,
                    0.0,
                ),
                move || selection.set(resource.clone()),
            )
            .background(if active {
                t.colors.accent.with_alpha(0.10)
            } else {
                CLEAR
            })
            .corner_radius(t.sidebar_item_radius)
            .hover_style(StateStyle::new().background(t.colors.surface_hover))
            .focus_style(StateStyle::new().outline(t.colors.accent, 1.0))
            .with_children(vec![
                Box::new(
                    RawView::new(fixed(64.0, 44.0))
                        .outline(device.color.with_alpha(0.55), 1.0)
                        .corner_radius(t.radius_small * 0.25)
                        .child(Chart::boxed(
                            samples,
                            device.color,
                            44.0,
                            device.ceiling,
                            false,
                        )),
                ),
                view(
                    grow(),
                    vec![
                        line(&device.title, 12.0, false),
                        line(&device.summary, 10.5, true),
                        line(&device.detail, 10.0, true),
                    ],
                ),
            ]),
        ) as BoxedWidget);
    }
    let rail = Box::new(
        RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(rail_width),
                height: Dimension::Percent(1.0),
            },
            flex_shrink: 0.0,
            ..column(0.0)
        })
        .background(CLEAR)
        .with_children(vec![
            view(pad(column(0.0), 12.0, 12.0), vec![heading("Devices", 12.0)]),
            scroll(
                state,
                "performance:devices",
                vec![view(pad(column(2.0), 5.0, 0.0), rows)],
            ),
        ]),
    );
    let width = (size.width - rail_width - 36.0 - 7.0 - 16.0).max(0.0);
    let content = match &selected.resource {
        Resource::Cpu => cpu(state, &s, width, size.height),
        Resource::Memory => memory(state, &s, size.height),
        Resource::Disk(name) => disk(state, &s, name, size.height),
        Resource::Gpu(id) => gpu(state, &s, id, size.height),
        Resource::Network(name) => interface(state, &s, name, size.height),
    };
    let key = format!("performance:{}", selected.resource.key());
    let document = view(pad(column(12.0), 18.0, 12.0), content);
    let main = scroll(state, &key, vec![document]);
    view(
        Style {
            align_items: Some(AlignItems::Stretch),
            ..fill_row()
        },
        vec![rail, content_panel(vec![main])],
    )
}
fn fill_row() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        ..fill()
    }
}
fn title(name: &str, model: &str) -> BoxedWidget {
    view(
        row(16.0),
        vec![
            heading(name, 23.0),
            view(grow(), vec![line(model, 12.0, true)]),
        ],
    )
}
fn graph(state: &State, key: &str, color: Color, height: f32, ceiling: f32) -> BoxedWidget {
    Chart::boxed(state.series(key), color, height, ceiling, true)
}
fn stat_list(rows: Vec<(String, String)>) -> BoxedWidget {
    view(
        column(10.0),
        rows.into_iter()
            .map(|(title, value)| {
                view(
                    column(3.0),
                    vec![label(title, 10.5, true), label(value, 13.0, false)],
                )
            })
            .collect(),
    )
}

fn cpu(state: &State, s: &Snapshot, width: f32, height: f32) -> Vec<BoxedWidget> {
    let mode = state.per_core.get();
    let t = use_theme();
    let mut colors = creamui_widgets::TabColors::dark();
    colors.background = card_color();
    colors.inactive_background = None;
    colors.active_background = t.colors.accent.with_alpha(0.12);
    colors.active_text = t.colors.accent;
    colors.text = t.colors.text_secondary;
    let mut toggle = creamui_widgets::Tabs::new(colors, row(0.0));
    for (per_core, name, width) in [(false, "Total", 60.0), (true, "Per core", 74.0)] {
        let selected = state.per_core.clone();
        toggle = toggle.child(Box::new(creamui_widgets::Tab::new(
            colors,
            fixed(width, 30.0),
            name,
            mode == per_core,
            move || selected.set(per_core),
        )));
    }
    let header = view(
        row(12.0),
        vec![
            heading("CPU", 23.0),
            view(grow(), vec![line(&s.cpu.model, 12.0, true)]),
            Box::new(toggle),
        ],
    );
    let count = s.cpu.cores.max(1);
    let columns: usize = if mode && count > 8 {
        4
    } else if mode && count > 1 {
        2
    } else {
        1
    };
    // Keep each mini-chart readable (12 × 8 logical pixels); decide whether
    // stats fit alongside from the actual grid, not a fixed page breakpoint.
    let minimum_graph_width = if mode {
        columns as f32 * 96.0 + columns.saturating_sub(1) as f32 * 10.0
    } else {
        320.0
    };
    let side = width >= minimum_graph_width + 208.0;
    let graph_width = if side { width - 208.0 } else { width };
    let header_height = (23.0 * t.text_scale * 1.5).max(30.0);
    let body_height = (height - 8.0 - 24.0 - header_height - 12.0).max(0.0);
    let chart = if mode {
        let rows = count.div_ceil(columns);
        let label_height = 15.0 * t.text_scale + 4.0;
        let chart_height = ((body_height - rows.saturating_sub(1) as f32 * 10.0) / rows as f32
            - label_height)
            .clamp(48.0, 210.0);
        let cell_width = ((graph_width - (columns - 1) as f32 * 10.0) / columns as f32).max(0.0);
        view(
            Style {
                flex_wrap: FlexWrap::Wrap,
                align_items: Some(AlignItems::Start),
                ..row(10.0)
            },
            (0..count)
                .map(|index| {
                    view(
                        Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Length(cell_width),
                                height: Dimension::Auto,
                            },
                            ..column(4.0)
                        },
                        vec![
                            line(
                                format!(
                                    "CPU {index} · {:.0}%",
                                    s.cpu.per_core.get(index).copied().unwrap_or(0.0)
                                ),
                                10.0,
                                true,
                            ),
                            graph(
                                state,
                                &format!("cpu:{index}"),
                                CPU_COLOR,
                                chart_height,
                                100.0,
                            ),
                        ],
                    )
                })
                .collect(),
        )
    } else {
        graph(
            state,
            "cpu",
            CPU_COLOR,
            body_height.clamp(180.0, 440.0),
            100.0,
        )
    };
    let mut stats = vec![
        ("Utilization".into(), cpu_value(s)),
        (
            "Current speed".into(),
            s.cpu
                .frequency_mhz
                .map(|v| format!("{:.2} GHz", v / 1000.0))
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Processes / threads".into(),
            format!(
                "{} / {}",
                s.processes.len(),
                s.processes.iter().map(|p| p.threads).sum::<u64>()
            ),
        ),
        ("Logical processors".into(), s.cpu.cores.to_string()),
        ("Uptime".into(), system::duration(s.uptime)),
        (
            "Load · 1 / 5 / 15 min".into(),
            format!(
                "{:.2} / {:.2} / {:.2}",
                s.cpu.load.0, s.cpu.load.1, s.cpu.load.2
            ),
        ),
        ("Scaling driver".into(), s.cpu.driver.clone()),
        ("Governor".into(), s.cpu.governor.clone()),
        ("Architecture".into(), s.architecture.clone()),
    ];
    if let Some(temp) = system::cpu_temperature(s) {
        stats.push(("Temperature".into(), format!("{temp:.0} °C")));
    }
    let body = if side {
        view(
            Style {
                align_items: Some(AlignItems::Start),
                ..row(20.0)
            },
            vec![
                view(grow(), vec![chart]),
                view(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(188.0),
                            height: Dimension::Auto,
                        },
                        flex_shrink: 0.0,
                        ..column(0.0)
                    },
                    vec![stat_list(stats)],
                ),
            ],
        )
    } else {
        view(column(18.0), vec![chart, info_grid(stats)])
    };
    vec![header, body]
}
fn memory(state: &State, s: &Snapshot, height: f32) -> Vec<BoxedWidget> {
    let used = s.memory.total.saturating_sub(s.memory.available);
    vec![
        title(
            "Memory",
            &format!("{} installed", system::bytes(s.memory.total)),
        ),
        line("Utilization over the last minute · 100%", 10.5, true),
        graph(
            state,
            "memory",
            MEMORY_COLOR,
            (height - 220.0).clamp(140.0, 360.0),
            100.0,
        ),
        progress(percent(used, s.memory.total), MEMORY_COLOR),
        info_grid(vec![
            ("In use".into(), system::bytes(used)),
            ("Available".into(), system::bytes(s.memory.available)),
            ("Installed".into(), system::bytes(s.memory.total)),
            (
                "Swap used".into(),
                system::bytes(s.memory.swap_total.saturating_sub(s.memory.swap_free)),
            ),
            ("Swap total".into(), system::bytes(s.memory.swap_total)),
        ]),
    ]
}
fn disk(state: &State, s: &Snapshot, name: &str, height: f32) -> Vec<BoxedWidget> {
    let Some(d) = s.drives.iter().find(|d| d.name == name) else {
        return vec![empty("Device disconnected", "Select another device.")];
    };
    let mut result = vec![
        title(&format!("Disk ({name})"), &d.model),
        line(format!("Active time · {:.0}%", d.usage), 11.0, true),
        graph(
            state,
            &format!("disk:{name}"),
            DISK_COLOR,
            (height - 380.0).clamp(100.0, 240.0),
            100.0,
        ),
        transfer_pair(
            state,
            &format!("disk:{name}:read"),
            &format!("disk:{name}:write"),
            "Read",
            "Write",
            d.read_rate,
            d.write_rate,
        ),
        info_grid(vec![
            ("Capacity".into(), system::bytes(d.size)),
            ("Type".into(), d.kind.clone()),
            ("Active time".into(), format!("{:.0}%", d.usage)),
            ("Read speed".into(), rate(d.read_rate)),
            ("Write speed".into(), rate(d.write_rate)),
        ]),
    ];
    let volumes = s
        .disks
        .iter()
        .filter(|volume| volume.device.contains(name))
        .map(|volume| {
            format!(
                "{} · {} free of {} · {}",
                volume.mount,
                system::bytes(volume.available),
                system::bytes(volume.total),
                volume.filesystem
            )
        })
        .collect::<Vec<_>>();
    if !volumes.is_empty() {
        result.push(label(volumes.join("\n"), 11.0, true));
    }
    result
}
fn gpu(state: &State, s: &Snapshot, id: &str, height: f32) -> Vec<BoxedWidget> {
    let Some(g) = s.gpus.iter().find(|g| g.id == id) else {
        return vec![empty("GPU unavailable", "Select another device.")];
    };
    let mut result = vec![title("GPU", &g.name)];
    if let Some(usage) = g.usage {
        result.extend([
            line(format!("Utilization · {usage:.0}%"), 11.0, true),
            graph(
                state,
                &format!("gpu:{id}"),
                GPU_COLOR,
                (height - 250.0).clamp(140.0, 360.0),
                100.0,
            ),
        ]);
    } else {
        result.push(empty(
            "Usage telemetry unavailable",
            "This graphics driver does not expose a GPU utilization counter.",
        ));
    }
    result.push(info_grid(vec![
        ("Driver".into(), g.driver.clone()),
        ("Device".into(), g.id.clone()),
        (
            "Video memory used".into(),
            g.memory_used
                .map(system::bytes)
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Video memory total".into(),
            g.memory_total
                .map(system::bytes)
                .unwrap_or_else(|| "Unavailable".into()),
        ),
        (
            "Temperature".into(),
            g.temperature
                .map(|v| format!("{v:.1} °C"))
                .unwrap_or_else(|| "Unavailable".into()),
        ),
    ]));
    result
}
fn interface(state: &State, s: &Snapshot, name: &str, _height: f32) -> Vec<BoxedWidget> {
    let Some(n) = s.network.iter().find(|n| n.name == name) else {
        return vec![empty("Interface disconnected", "Select another interface.")];
    };
    vec![
        title(name, &n.state),
        transfer_pair(
            state,
            &format!("net:{name}:rx"),
            &format!("net:{name}:tx"),
            "Receive",
            "Send",
            n.receive_rate,
            n.transmit_rate,
        ),
        info_grid(vec![
            ("Received".into(), system::bytes(n.received)),
            ("Sent".into(), system::bytes(n.transmitted)),
            (
                "Link speed".into(),
                n.speed_mbps
                    .map(|v| format!("{v} Mbps"))
                    .unwrap_or_else(|| "Unavailable".into()),
            ),
            ("MAC address".into(), n.mac.clone()),
            ("IP addresses".into(), n.addresses.join("\n")),
        ]),
    ]
}
#[allow(clippy::too_many_arguments)]
pub(super) fn transfer_pair(
    state: &State,
    first: &str,
    second: &str,
    first_title: &str,
    second_title: &str,
    a: f32,
    b: f32,
) -> BoxedWidget {
    let a_samples = state.series(first);
    let b_samples = state.series(second);
    let ceiling = a_samples
        .iter()
        .chain(&b_samples)
        .copied()
        .fold(1024.0_f32, f32::max)
        * 1.1;
    view(
        Style {
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::Stretch),
            ..row(12.0)
        },
        [
            (first_title, a, a_samples, RX_COLOR),
            (second_title, b, b_samples, TX_COLOR),
        ]
        .into_iter()
        .map(|(title, value, samples, color)| {
            view(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(230.0),
                    ..column(8.0)
                },
                vec![
                    label(format!("{title} · {}", rate(value)), 11.0, true),
                    Chart::boxed(samples, color, 150.0, ceiling, true),
                ],
            )
        })
        .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn network_interfaces_always_follow_hardware_devices() {
        let mut s = Snapshot::default();
        s.gpus = vec![crate::telemetry::Gpu {
            id: "card0".into(),
            ..Default::default()
        }];
        s.network = (0..50)
            .map(|index| system::Network {
                name: format!("veth{index}"),
                ..Default::default()
            })
            .collect();
        let resources = devices(&s)
            .into_iter()
            .map(|device| device.resource)
            .collect::<Vec<_>>();
        assert_eq!(resources[2], Resource::Gpu("card0".into()));
        assert!(resources[3..]
            .iter()
            .all(|resource| matches!(resource, Resource::Network(_))));
    }
}
