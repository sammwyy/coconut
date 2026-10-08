pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let window_settings = &context.window_settings;
    let native = &context.native;
    crate::views::native::accessibility(native, window_settings)
}
