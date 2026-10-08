use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let body = vec![
        entries(state, "online-accounts", "No online accounts reported"),
        value(s.fact("online-accounts-service")),
    ];
    detail_page(body, state)
}
