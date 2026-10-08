pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    let account_list = context.users.get();
    crate::views::native::accounts(native, &account_list, view)
}
pub mod account;
pub mod create;
pub mod profile;
