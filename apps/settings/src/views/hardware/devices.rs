use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = vec![
        card(
            "USB devices",
            vec![entries(state, "devices", "No USB devices reported")],
        ),
        card(
            "Input devices",
            vec![entries(state, "input-devices", "No input devices reported")],
        ),
    ];
    detail_page(body, state)
}
