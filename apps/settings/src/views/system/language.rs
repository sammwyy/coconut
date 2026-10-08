use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![card(
        "System locale",
        vec![state.preference(
            "Language & region",
            "locale",
            "Choose an installed locale; new sessions pick up the change",
        )],
    )];
    detail_page(body, state)
}
