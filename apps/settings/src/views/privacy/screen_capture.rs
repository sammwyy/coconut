use crate::components::group;
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::Switch;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let rows = s
        .entries("screen-capture-permissions")
        .iter()
        .map(|e| {
            let write = state.clone();
            let subject = e.id.clone();
            let allowed = e.enabled;
            item(
                &e.name,
                &e.description,
                Box::new(Switch::new(allowed, move || {
                    write.apply(Action::SetScreenCapturePermission {
                        subject: subject.clone(),
                        allowed: !allowed,
                    })
                })),
                || {},
            )
        })
        .collect::<Vec<_>>();
    let body = vec![
        value("Permissions saved by the Coconut permission portal. Applications without a saved decision will be asked when they try to capture the screen."),
        value(s.fact("screen-capture-service")),
        group(if rows.is_empty() {
            vec![key_value("Applications", "No saved permissions reported")]
        } else {
            rows
        }),
    ];
    detail_page(body, state)
}
