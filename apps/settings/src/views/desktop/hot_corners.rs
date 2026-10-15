use creamui_core::BoxedWidget;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    crate::views::windows::build_hot_corners(context.size, &context.window_settings)
}
