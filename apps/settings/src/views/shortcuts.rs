pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let shortcut_settings = &context.shortcut_settings;
    crate::views::windows::build_shortcuts(size, shortcut_settings)
}

pub(crate) fn keyboard_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let shortcut_settings = &context.shortcut_settings;
    crate::views::windows::build_shortcuts(size, shortcut_settings)
}
