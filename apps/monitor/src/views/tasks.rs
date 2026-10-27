use super::table::{Column, Row};
use super::*;

pub(super) fn build(state: &State, size: Size) -> BoxedWidget {
    if state.task_mode.get() == 0 {
        apps(state, size)
    } else {
        processes(state, size)
    }
}
fn header(state: &State) -> BoxedWidget {
    let mut colors = creamui_widgets::TabColors::dark();
    let t = use_theme();
    colors.background = CLEAR;
    colors.inactive_background = None;
    colors.active_background = t.colors.accent.with_alpha(0.12);
    colors.active_text = t.colors.accent;
    colors.text = t.colors.text_secondary;
    colors.radius = t.tab_radius;
    let mut tabs = creamui_widgets::Tabs::new(colors, row(0.0));
    for (mode, title) in ["Applications", "Processes"].iter().enumerate() {
        let selection = state.task_mode.clone();
        tabs = tabs.child(Box::new(creamui_widgets::Tab::new(
            colors,
            fixed(108.0, 30.0),
            *title,
            state.task_mode.get() == mode,
            move || selection.set(mode),
        )));
    }
    section_header("Apps", Box::new(tabs))
}

fn share(value: f32, s: &Snapshot) -> f32 {
    (value / s.cpu.cores.max(1) as f32).clamp(0.0, 100.0)
}
fn columns(s: &Snapshot, width: f32, apps: bool, count: usize) -> Vec<Column> {
    let memory = percent(
        s.memory.total.saturating_sub(s.memory.available),
        s.memory.total,
    );
    let io = s.drives.iter().map(|d| d.read_rate + d.write_rate).sum();
    let mut columns = vec![Column::flex(
        if apps { "Application" } else { "Name" },
        format!("{count} {}", if apps { "apps" } else { "processes" }),
        2.0,
        Some(Sort::Name),
    )];
    if apps {
        columns.push(Column::fixed("Processes", "", 76.0, true, None));
    } else {
        columns.push(Column::fixed("PID", "", 62.0, true, Some(Sort::Pid)));
        if width >= 900.0 {
            columns.push(Column::fixed("State", "", 80.0, false, None));
        }
    }
    columns.extend([
        Column::fixed("CPU", cpu_value(s), 74.0, true, Some(Sort::Cpu)),
        Column::fixed(
            "Memory",
            format!("{memory:.0}%"),
            104.0,
            true,
            Some(Sort::Memory),
        ),
        Column::fixed("Disk", rate(io), 104.0, true, Some(Sort::Disk)),
    ]);
    if !apps && width >= 1120.0 {
        columns.push(Column::flex("Command", "", 2.0, None));
    }
    columns
}
fn values(p: &Process, s: &Snapshot, width: f32) -> Vec<String> {
    let mut values = vec![p.name.clone(), p.pid.to_string()];
    if width >= 900.0 {
        values.push(process_state(&p.state).into());
    }
    values.extend([
        format!("{:.1}%", share(p.cpu, s)),
        system::bytes(p.memory),
        if p.io_available {
            rate(p.read_rate + p.write_rate)
        } else {
            "—".into()
        },
    ]);
    if width >= 1120.0 {
        values.push(p.command.clone());
    }
    values
}
fn compare(a: &Process, b: &Process, sort: Sort) -> std::cmp::Ordering {
    match sort {
        Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        Sort::Pid => a.pid.cmp(&b.pid),
        Sort::Cpu => a.cpu.total_cmp(&b.cpu),
        Sort::Memory => a.memory.cmp(&b.memory),
        Sort::Disk => (a.read_rate + a.write_rate).total_cmp(&(b.read_rate + b.write_rate)),
    }
    .then(a.pid.cmp(&b.pid))
}

pub(super) fn processes(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let query = state.process_search.value().to_lowercase();
    let (sort, descending) = state.sort.get();
    let mut processes = s
        .processes
        .iter()
        .filter(|p| matches_process(p, &query))
        .collect::<Vec<_>>();
    processes.sort_by(|a, b| {
        let order = compare(a, b, sort);
        if descending {
            order.reverse()
        } else {
            order
        }
    });
    let count = processes.len();
    let rows = processes
        .into_iter()
        .map(|p| {
            let selection = state.selected_process.clone();
            let pid = p.pid;
            Row {
                key: pid.to_string(),
                values: values(p, &s, size.width),
                selected: state.selected_process.get() == Some(pid),
                select: Rc::new(move || selection.set(Some(pid))),
            }
        })
        .collect();
    let selected = s
        .processes
        .iter()
        .find(|p| state.selected_process.get() == Some(p.pid));
    let can_act = selected.is_some_and(actions::can_signal) && !state.action_busy.get();
    let mut tools = vec![view(
        grow(),
        vec![search(
            &state.process_search,
            "Search name, PID or command…",
        )],
    )];
    if size.width >= 850.0 {
        tools.push(label(format!("{count} processes"), 11.0, true));
    }
    let details = state.clone();
    let process = selected.cloned();
    tools.push(button(
        "Details",
        64.0,
        selected.is_some(),
        false,
        move || {
            if let Some(p) = &process {
                details.detail.set(Some(Detail::Process(p.clone())));
            }
        },
    ));
    let control = state.clone();
    let process = selected.cloned();
    let paused = selected.is_some_and(|p| matches!(p.state.as_str(), "T" | "t"));
    tools.push(button(
        if paused { "Resume" } else { "Pause" },
        72.0,
        can_act,
        false,
        move || {
            if let Some(p) = &process {
                control.request(Action::Process(
                    p.clone(),
                    if paused {
                        ProcessAction::Resume
                    } else {
                        ProcessAction::Pause
                    },
                ));
            }
        },
    ));
    let end = state.clone();
    let process = selected.cloned();
    tools.push(button("End task", 82.0, can_act, true, move || {
        if let Some(p) = &process {
            end.request(Action::Process(p.clone(), ProcessAction::End));
        }
    }));
    let mut children = vec![table::build(
        state,
        "processes",
        Size {
            width: size.width,
            height: (size.height - 152.0).max(0.0),
        },
        columns(&s, size.width, false, count),
        rows,
    )];
    if let Some(p) = selected {
        children.push(view(
            pad(column(0.0), 14.0, 4.0),
            vec![line(
                format!("{} · PID {} · {}", p.name, p.pid, p.command),
                10.5,
                true,
            )],
        ));
    }
    view(
        fill(),
        vec![
            header(state),
            content_panel(children),
            view(pad(row(8.0), 12.0, 10.0), tools),
        ],
    )
}

pub(super) fn apps(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let query = state.app_search.value().to_lowercase();
    let (sort, descending) = state.sort.get();
    let mut apps = s
        .applications
        .iter()
        .filter(|app| {
            query.is_empty()
                || app.name.to_lowercase().contains(&query)
                || app.processes.iter().any(|p| matches_process(p, &query))
        })
        .collect::<Vec<_>>();
    apps.sort_by(|a, b| {
        let order = match sort {
            Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Sort::Cpu => a.cpu.total_cmp(&b.cpu),
            Sort::Memory => a.memory.cmp(&b.memory),
            Sort::Disk => app_io(a).total_cmp(&app_io(b)),
            Sort::Pid => a.id.cmp(&b.id),
        }
        .then(a.id.cmp(&b.id));
        if descending {
            order.reverse()
        } else {
            order
        }
    });
    let count = apps.len();
    let mut rows = vec![];
    for app in apps {
        let selected = state.selected_app.get().as_deref() == Some(&app.id);
        let expanded = state.expanded_app.get().as_deref() == Some(&app.id);
        let selection = state.clone();
        let id = app.id.clone();
        rows.push(Row {
            key: app.id.clone(),
            values: vec![
                format!("{} {}", if expanded { "▾" } else { "▸" }, app.name),
                app.processes.len().to_string(),
                format!("{:.1}%", share(app.cpu, &s)),
                system::bytes(app.memory),
                rate(app_io(app)),
            ],
            selected,
            select: Rc::new(move || {
                batch(|| {
                    selection.selected_app.set(Some(id.clone()));
                    selection.expanded_app.set(
                        if selection.expanded_app.peek().as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        },
                    );
                })
            }),
        });
        if expanded {
            for p in &app.processes {
                let details = state.clone();
                let process = p.clone();
                rows.push(Row {
                    key: format!("{}:{}", app.id, p.pid),
                    values: vec![
                        format!("    {} ({})", p.name, p.pid),
                        String::new(),
                        format!("{:.1}%", share(p.cpu, &s)),
                        system::bytes(p.memory),
                        rate(p.read_rate + p.write_rate),
                    ],
                    selected: false,
                    select: Rc::new(move || {
                        details.detail.set(Some(Detail::Process(process.clone())))
                    }),
                });
            }
        }
    }
    let selected = s
        .applications
        .iter()
        .find(|app| state.selected_app.get().as_deref() == Some(&app.id));
    let details = state.clone();
    let app = selected.cloned();
    let end = state.clone();
    let ending = selected.cloned();
    let tools = vec![
        view(
            grow(),
            vec![search(
                &state.app_search,
                "Search applications or their processes…",
            )],
        ),
        button("Details", 64.0, selected.is_some(), false, move || {
            if let Some(app) = &app {
                details.detail.set(Some(Detail::Text(
                    app.name.clone(),
                    format!(
                        "Desktop ID: {}\nProcesses: {}\nResident memory (sum): {}\n\n{}",
                        app.id,
                        app.processes.len(),
                        system::bytes(app.memory),
                        app.processes
                            .iter()
                            .map(|p| format!("{} · {}\n{}", p.pid, p.name, p.command))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    ),
                )));
            }
        }),
        button(
            "End app",
            82.0,
            selected.is_some_and(|app| app.processes.iter().all(actions::can_signal))
                && !state.action_busy.get(),
            true,
            move || {
                if let Some(app) = &ending {
                    end.request(Action::Application(app.name.clone(), app.processes.clone()));
                }
            },
        ),
    ];
    view(
        fill(),
        vec![
            header(state),
            content_panel(vec![table::build(
                state,
                "apps",
                Size {
                    width: size.width,
                    height: (size.height - 150.0).max(0.0),
                },
                columns(&s, size.width, true, count),
                rows,
            )]),
            view(pad(row(8.0), 12.0, 10.0), tools),
        ],
    )
}
fn app_io(app: &crate::applications::Application) -> f32 {
    app.processes
        .iter()
        .map(|p| p.read_rate + p.write_rate)
        .sum()
}
