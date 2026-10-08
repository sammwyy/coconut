use crate::components::group;
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::Switch;

use crate::{components::system_settings::*, routes::detail::Page};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let nav = &context.view;
    let s = state.snapshot.get();
    let body = {
        let rows = s
            .entries("startup")
            .iter()
            .map(|e| {
                let write = state.clone();
                let id = e.id.clone();
                let enabled = e.enabled;
                item(
                    &e.name,
                    &e.description,
                    Box::new(Switch::new(enabled, move || {
                        write.apply(Action::SetStartup {
                            id: id.clone(),
                            enabled: !enabled,
                        })
                    })),
                    || {},
                )
            })
            .collect::<Vec<_>>();
        vec![
            value(s.fact("startup-service")),
            group(if rows.is_empty() {
                vec![key_value("Startup", "No startup entries reported")]
            } else {
                rows
            }),
            detail_link(
                "Add application",
                "Choose an installed desktop application",
                Page::AddStartup,
                nav,
            ),
        ]
    };
    detail_page(body, state)
}
