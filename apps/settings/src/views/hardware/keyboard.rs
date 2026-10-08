pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    crate::views::windows::build_keyboard(size, window_settings)
}
