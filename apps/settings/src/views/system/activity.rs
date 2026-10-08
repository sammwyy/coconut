use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![
        value("A snapshot of the 50 processes using the most CPU. Refresh to read current values."),
        entries(state, "activity", "Process information unavailable"),
    ];
    detail_page(body, state)
}
