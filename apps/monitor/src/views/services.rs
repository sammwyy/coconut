use super::table::{Column, Row};
use super::*;

pub(super) fn build(state: &State, size: Size) -> BoxedWidget {
    let s = state.snapshot.get();
    let user = state.user_services.get();
    let services = if user { &s.user_services } else { &s.services };
    let query = state.service_search.value().to_lowercase();
    let items = services
        .iter()
        .filter(|service| {
            query.is_empty()
                || service.unit.to_lowercase().contains(&query)
                || service.description.to_lowercase().contains(&query)
                || service.state.to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();
    let count = items.len();
    let running = items
        .iter()
        .filter(|service| service.state.starts_with("active"))
        .count();
    let columns = vec![
        Column::flex("Unit", format!("{count} services"), 1.0, None),
        Column::fixed("State", format!("{running} running"), 136.0, false, None),
        Column::flex("Description", "", 1.4, None),
    ];
    let rows = items
        .into_iter()
        .map(|service| {
            let selection = state.selected_service.clone();
            let unit = service.unit.clone();
            Row {
                key: unit.clone(),
                values: vec![
                    unit.clone(),
                    service.state.clone(),
                    service.description.clone(),
                ],
                selected: state.selected_service.get().as_ref() == Some(&unit),
                select: Rc::new(move || selection.set(Some(unit.clone()))),
            }
        })
        .collect();
    let selected = services
        .iter()
        .find(|service| state.selected_service.get().as_deref() == Some(&service.unit));
    let busy = state.action_busy.get();
    let active = selected.is_some_and(|service| service.state.starts_with("active"));
    let mut colors = creamui_widgets::TabColors::dark();
    let t = use_theme();
    colors.background = CLEAR;
    colors.inactive_background = None;
    colors.active_background = t.colors.accent.with_alpha(0.12);
    colors.active_text = t.colors.accent;
    colors.text = t.colors.text_secondary;
    let mut scope = creamui_widgets::Tabs::new(colors, row(0.0));
    for (user_scope, title) in [(false, "System"), (true, "User")] {
        let selected = state.user_services.clone();
        scope = scope.child(Box::new(creamui_widgets::Tab::new(
            colors,
            fixed(76.0, 30.0),
            title,
            user == user_scope,
            move || selected.set(user_scope),
        )));
    }
    let mut tools = vec![view(
        grow(),
        vec![search(&state.service_search, "Search services…")],
    )];
    for (action, title, enabled) in [
        (ServiceAction::Start, "Start", !active),
        (ServiceAction::Stop, "Stop", active),
        (ServiceAction::Restart, "Restart", true),
    ] {
        let unit = selected.map(|service| service.unit.clone());
        let state = state.clone();
        tools.push(button(
            title,
            if action == ServiceAction::Restart {
                72.0
            } else {
                56.0
            },
            unit.is_some() && enabled && !busy,
            action == ServiceAction::Stop,
            move || {
                if let Some(unit) = &unit {
                    state.request(Action::Service {
                        unit: unit.clone(),
                        user,
                        action,
                    });
                }
            },
        ));
    }
    view(
        fill(),
        vec![
            section_header("Services", Box::new(scope)),
            content_panel(vec![table::build(
                state,
                if user {
                    "services:user"
                } else {
                    "services:system"
                },
                Size {
                    width: size.width,
                    height: (size.height - 150.0).max(0.0),
                },
                columns,
                rows,
            )]),
            view(pad(row(7.0), 12.0, 10.0), tools),
        ],
    )
}
