use crate::components::group;
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};

pub(crate) fn route_view() -> BoxedWidget {
    application_list(false)
}

pub(super) fn application_list(startup: bool) -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let nav = &context.view;
    let body = vec![
        input(&state.search, "Search applications…"),
        crate::views::applications::installed::app_list(state, nav, startup),
    ];
    detail_page(body, state)
}

pub(super) fn app_list(state: &State, nav: &creamui_router::Router, startup: bool) -> BoxedWidget {
    let s = state.snapshot.get();
    let query = state.search.value().to_lowercase();
    let rows = s
        .entries("applications")
        .iter()
        .filter(|e| {
            query.is_empty()
                || e.name.to_lowercase().contains(&query)
                || e.id.to_lowercase().contains(&query)
        })
        .map(|e| {
            if startup {
                let write = state.clone();
                let id = e.id.clone();
                item(
                    &e.name,
                    &e.description,
                    action("Add", move || {
                        write.apply(Action::AddStartup {
                            desktop_id: id.clone(),
                        })
                    }),
                    || {},
                )
            } else {
                detail_link(&e.name, &e.description, Page::App(e.id.clone()), nav)
            }
        })
        .collect::<Vec<_>>();
    group(if rows.is_empty() {
        vec![key_value(
            "Applications",
            if s.loaded {
                "No matching desktop applications"
            } else {
                "Loading…"
            },
        )]
    } else {
        rows
    })
}
