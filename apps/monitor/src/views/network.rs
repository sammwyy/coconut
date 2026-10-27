use super::table::{Column, Row};
use super::*;

pub(super) fn build(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let mode = state.network_mode.get();
    let filter = state
        .interface
        .get()
        .filter(|name| s.network.iter().any(|n| &n.name == name));
    let rail_width = if size.width < 800.0 { 154.0 } else { 180.0 };
    let mut modes = vec![];
    for (index, title) in ["Connections", "Ports"].iter().enumerate() {
        let selection = state.network_mode.clone();
        modes.push(side_button(title, mode == index, move || {
            selection.set(index)
        }));
    }
    let all = state.interface.clone();
    let mut interfaces = vec![side_button("All", filter.is_none(), move || all.set(None))];
    for interface in &s.network {
        let selection = state.interface.clone();
        let name = interface.name.clone();
        let active = filter.as_ref() == Some(&name);
        interfaces.push(side_button(&name.clone(), active, move || {
            selection.set(Some(name.clone()))
        }));
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
            view(pad(column(3.0), 8.0, 12.0), modes),
            divider(),
            view(
                pad(column(0.0), 12.0, 13.0),
                vec![label("INTERFACES", 10.0, true)],
            ),
            scroll(
                state,
                "network:interfaces",
                vec![view(pad(column(2.0), 8.0, 0.0), interfaces)],
            ),
        ]),
    );
    let main_size = Size {
        width: (size.width - rail_width - 16.0).max(0.0),
        height: size.height,
    };
    let detail = format!(
        "{} · {}",
        ["Connections", "Ports"][mode],
        filter.as_deref().unwrap_or("All interfaces")
    );
    let (content, tools) = endpoints(state, &s, main_size, filter.as_deref(), mode == 1);
    view(
        Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Stretch),
            ..fill()
        },
        vec![
            rail,
            view(
                fill(),
                vec![
                    content_panel(vec![page_title("Networking", detail), content]),
                    tools,
                ],
            ),
        ],
    )
}
fn side_button(title: &str, active: bool, select: impl Fn() + 'static) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawButton::new(
            pad(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(34.0),
                    },
                    flex_shrink: 0.0,
                    ..row(0.0)
                },
                10.0,
                0.0,
            ),
            select,
        )
        .background(if active {
            t.colors.accent.with_alpha(0.10)
        } else {
            CLEAR
        })
        .corner_radius(t.sidebar_item_radius)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .focus_style(StateStyle::new().outline(t.colors.accent, 1.0))
        .child(Box::new(Text::new(title).font_size(12.0).color(
            if active {
                t.colors.accent
            } else {
                t.colors.text_primary
            },
        ))),
    )
}

pub(super) fn is_port(socket: &system::Socket) -> bool {
    socket.state == "LISTEN"
        || (socket.protocol.starts_with("UDP")
            && socket.remote.ends_with(":0")
            && socket.local.rsplit(':').next().is_some_and(|p| p != "0"))
}
pub(super) fn matches_interface(
    socket: &system::Socket,
    interface: Option<&system::Network>,
) -> bool {
    let Some(interface) = interface else {
        return true;
    };
    let ip = socket
        .local
        .rsplit_once(':')
        .map(|(ip, _)| ip.trim_matches(['[', ']']))
        .unwrap_or_default();
    if matches!(ip, "0.0.0.0" | "::") {
        return true;
    }
    interface
        .addresses
        .iter()
        .any(|address| address == ip || ip.strip_prefix("::ffff:") == Some(address.as_str()))
}
fn endpoints(
    state: &State,
    s: &Snapshot,
    size: Size,
    filter: Option<&str>,
    ports: bool,
) -> (BoxedWidget, BoxedWidget) {
    let interface = filter.and_then(|name| s.network.iter().find(|n| n.name == name));
    let query = state.network_search.value().to_lowercase();
    let items = s
        .sockets
        .iter()
        .filter(|socket| {
            is_port(socket) == ports
                && matches_interface(socket, interface)
                && [
                    &socket.protocol,
                    &socket.local,
                    &socket.remote,
                    &socket.state,
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();
    let mut columns = vec![
        Column::fixed("Protocol", items.len().to_string(), 62.0, false, None),
        Column::flex("Local address", "", 1.0, None),
        Column::flex("Remote address", "", 1.0, None),
    ];
    if size.width >= 650.0 {
        columns.push(Column::fixed("State", "", 104.0, false, None));
    }
    if size.width >= 900.0 {
        columns.push(Column::fixed("Inode", "", 76.0, true, None));
    }
    let rows = items
        .into_iter()
        .map(|socket| {
            let mut values = vec![
                socket.protocol.clone(),
                socket.local.clone(),
                socket.remote.clone(),
            ];
            if size.width >= 650.0 {
                values.push(socket.state.clone());
            }
            if size.width >= 900.0 {
                values.push(socket.inode.clone());
            }
            let state = state.clone();
            let title = format!("{} endpoint", socket.protocol);
            let detail = format!(
                "Local address\n{}\n\nRemote address\n{}\n\nState\n{}\n\nSocket inode\n{}",
                socket.local, socket.remote, socket.state, socket.inode
            );
            Row {
                key: format!("{}:{}", socket.protocol, socket.inode),
                values,
                selected: false,
                select: Rc::new(move || {
                    state
                        .detail
                        .set(Some(Detail::Text(title.clone(), detail.clone())))
                }),
            }
        })
        .collect();
    let key = format!(
        "network:{}:{}",
        if ports { "ports" } else { "connections" },
        filter.unwrap_or("all")
    );
    let traffic_height = if ports { 0.0 } else { 152.0 };
    let mut children = vec![];
    if !ports {
        children.push(traffic(state, s, filter));
    }
    children.push(table::build(
        state,
        &key,
        Size {
            width: size.width,
            height: (size.height - 150.0 - traffic_height).max(0.0),
        },
        columns,
        rows,
    ));
    let tools = view(
        pad(column(0.0), 12.0, 10.0),
        vec![search(
            &state.network_search,
            "Filter addresses, ports or protocols…",
        )],
    );
    (view(fill(), children), tools)
}
fn traffic(state: &State, s: &Snapshot, filter: Option<&str>) -> BoxedWidget {
    let interfaces = s
        .network
        .iter()
        .filter(|n| filter.is_none_or(|name| n.name == name))
        .collect::<Vec<_>>();
    let rx = interfaces.iter().map(|n| n.receive_rate).sum();
    let tx = interfaces.iter().map(|n| n.transmit_rate).sum();
    let (rx_key, tx_key) = if let Some(name) = filter {
        (format!("net:{name}:rx"), format!("net:{name}:tx"))
    } else {
        ("network:rx".into(), "network:tx".into())
    };
    let mut graphs = vec![];
    for (title, value, key, color) in [
        ("Receive", rx, rx_key, RX_COLOR),
        ("Send", tx, tx_key, TX_COLOR),
    ] {
        graphs.push(view(
            grow(),
            vec![
                view(
                    row(12.0),
                    vec![
                        label(title, 12.0, true),
                        view(
                            grow(),
                            vec![Box::new(
                                Line::new(rate(value), 16.0, use_theme().colors.text_primary).end(),
                            )],
                        ),
                    ],
                ),
                Chart::boxed(state.series(&key), color, 90.0, 0.0, true),
            ],
        ));
    }
    view(
        pad(
            Style {
                flex_shrink: 0.0,
                ..row(18.0)
            },
            18.0,
            12.0,
        ),
        graphs,
    )
}
