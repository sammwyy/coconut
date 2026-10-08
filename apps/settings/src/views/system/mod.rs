pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    crate::views::native::system(native, view)
}
