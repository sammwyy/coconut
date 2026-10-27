use super::*;

pub(super) fn build(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let width = (size.width - 59.0).max(0.0);
    let columns = if width >= 660.0 { 3.0 } else { 2.0 };
    let card_width = (width - (columns - 1.0) * 12.0) / columns;
    let used = s.memory.total.saturating_sub(s.memory.available);
    let mut tiles = vec![
        tile(
            state,
            card_width,
            "CPU",
            cpu_value(&s),
            format!("{} logical processors", s.cpu.cores),
            Glyph::Cpu,
            CPU_COLOR,
            state.series("cpu"),
            100.0,
            Some(Resource::Cpu),
        ),
        tile(
            state,
            card_width,
            "Memory",
            format!("{:.0}%", percent(used, s.memory.total)),
            format!(
                "{} of {}",
                system::bytes(used),
                system::bytes(s.memory.total)
            ),
            Glyph::Memory,
            MEMORY_COLOR,
            state.series("memory"),
            100.0,
            Some(Resource::Memory),
        ),
    ];
    let disk = s.drives.first();
    let disk_usage = s.drives.iter().map(|d| d.usage).fold(0.0_f32, f32::max);
    let disk_rate = s.drives.iter().map(|d| d.read_rate + d.write_rate).sum();
    tiles.push(tile(
        state,
        card_width,
        "Disk",
        if s.drives.iter().any(|d| d.sampled) {
            format!("{disk_usage:.0}%")
        } else {
            "—".into()
        },
        if disk.is_some() {
            rate(disk_rate)
        } else {
            "No activity counters".into()
        },
        Glyph::Storage,
        DISK_COLOR,
        state.series("disk:all"),
        100.0,
        disk.map(|d| Resource::Disk(d.name.clone())),
    ));
    if let Some(gpu) = s.gpus.first() {
        tiles.push(tile(
            state,
            card_width,
            "GPU",
            gpu.usage
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "—".into()),
            gpu.name.clone(),
            Glyph::Hardware,
            GPU_COLOR,
            state.series(&format!("gpu:{}", gpu.id)),
            100.0,
            Some(Resource::Gpu(gpu.id.clone())),
        ));
    }
    let rx = s.network.iter().map(|n| n.receive_rate).sum();
    let tx = s.network.iter().map(|n| n.transmit_rate).sum();
    for (title, value, key, color) in [
        ("Receive", rx, "network:rx", RX_COLOR),
        ("Send", tx, "network:tx", TX_COLOR),
    ] {
        tiles.push(tile(
            state,
            card_width,
            title,
            if s.network.iter().any(|n| n.sampled) {
                rate(value)
            } else {
                "—".into()
            },
            "All network interfaces".into(),
            Glyph::Network,
            color,
            state.series(key),
            0.0,
            None,
        ));
    }
    let mut computer = vec![
        ("Computer", s.hostname.clone()),
        ("Uptime", system::duration(s.uptime)),
    ];
    if let Some(b) = &s.battery {
        computer.push((
            "Battery",
            format!("{} · {} · {}", b.capacity, b.status, b.energy),
        ));
    } else {
        computer.push((
            "Processes",
            format!(
                "{} · {} applications",
                s.processes.len(),
                s.applications.len()
            ),
        ));
    }
    let system = vec![
        ("System", s.os_name.clone()),
        ("Kernel", s.kernel.clone()),
        (
            "Version",
            format!("{} · Monitor {}", s.architecture, env!("CARGO_PKG_VERSION")),
        ),
    ];
    let cards = view(
        Style {
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::Stretch),
            ..row(12.0)
        },
        vec![info_card(computer), info_card(system)],
    );
    let document = vec![
        view(
            Style {
                flex_wrap: FlexWrap::Wrap,
                align_items: Some(AlignItems::Stretch),
                ..row(12.0)
            },
            tiles,
        ),
        cards,
    ];
    content_panel(vec![
        page_title("Overview", ""),
        scroll(
            state,
            "overview",
            vec![view(pad(column(12.0), 18.0, 0.0), document)],
        ),
    ])
}

#[allow(clippy::too_many_arguments)]
fn tile(
    state: &State,
    width: f32,
    title: &str,
    value: String,
    detail: String,
    glyph: Glyph,
    color: Color,
    samples: Vec<f32>,
    ceiling: f32,
    resource: Option<Resource>,
) -> BoxedWidget {
    let t = use_theme();
    let temperature = (title == "CPU")
        .then(|| system::cpu_temperature(&state.snapshot.get()))
        .flatten();
    let state = state.clone();
    let router = use_router();
    Box::new(
        RawButton::new(
            pad(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(width),
                    ..column(6.0)
                },
                12.0,
                11.0,
            ),
            move || {
                if let Some(resource) = &resource {
                    state.resource.set(resource.clone());
                    let _ = router.navigate("/performance");
                } else {
                    state.network_mode.set(0);
                    state.interface.set(None);
                    let _ = router.navigate("/network");
                }
            },
        )
        .background(card_color())
        .outline(t.colors.border.with_alpha(0.7), 1.0)
        .corner_radius(t.card_radius)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .focus_style(StateStyle::new().outline(t.colors.accent, 1.5))
        .with_children(vec![
            view(
                row(7.0),
                vec![
                    icon(glyph, color, 14.0),
                    view(grow(), vec![line(title, 14.0, false)]),
                    view(
                        Style {
                            flex_grow: 2.0,
                            ..grow()
                        },
                        vec![Box::new(
                            Line::new(value, 26.0, t.colors.text_primary).end(),
                        )],
                    ),
                ],
            ),
            view(
                row(8.0),
                vec![view(grow(), vec![line(detail, 12.5, true)]), {
                    temperature
                        .map(|value| label(format!("{value:.0} °C"), 13.0, false))
                        .unwrap_or_else(|| view(column(0.0), vec![]))
                }],
            ),
            Chart::boxed(samples, color, 54.0, ceiling, false),
        ]),
    )
}
fn info_card(rows: Vec<(&str, String)>) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawView::new(pad(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(280.0),
                ..column(9.0)
            },
            14.0,
            13.0,
        ))
        .background(card_color())
        .outline(t.colors.border.with_alpha(0.7), 1.0)
        .corner_radius(t.card_radius)
        .with_children(
            rows.into_iter()
                .map(|(key, value)| {
                    view(
                        row(12.0),
                        vec![
                            view(fixed(72.0, 21.0), vec![label(key, 12.0, true)]),
                            view(grow(), vec![line(value, 14.0, false)]),
                        ],
                    )
                })
                .collect(),
        ),
    )
}
