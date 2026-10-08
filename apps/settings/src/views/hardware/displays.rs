use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![value("Connected outputs reported by the compositor. DRM fallback lists supported modes, not the active mode."),
            entries(state, "displays", "No connected outputs reported"), value("Changing modes is unavailable until this compositor exposes safe apply/revert support.")];
    detail_page(body, state)
}
