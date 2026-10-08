pub mod appearance;
pub mod asset_packs;
pub mod wallpaper;

pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let config = &context.config;
    let appearance = &context.appearance;
    let view = &context.view;
    let window = &context.window;
    let appearance_accent_picker = &context.appearance_accent_picker;
    let appearance_custom_accent = &context.appearance_custom_accent;
    let native = &context.native;
    crate::views::native::personalization(
        native,
        config,
        appearance,
        view,
        crate::views::personalization::appearance::overview(
            appearance,
            window,
            appearance_accent_picker,
            appearance_custom_accent,
        ),
    )
}
