use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let window = &context.window_settings;
    let body = vec![window.rules(&state.rules)];
    detail_page(body, state)
}
