use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![unsupported(
        "Hot corners",
        "Blair does not expose a hot-corner service. No synthetic preference is saved.",
    )];
    detail_page(body, state)
}
