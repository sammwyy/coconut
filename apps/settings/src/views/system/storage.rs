use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![
        value("Mounted non-temporary filesystems. Capacities are read from the operating system."),
        entries(state, "storage", "Storage information unavailable"),
    ];
    detail_page(body, state)
}
