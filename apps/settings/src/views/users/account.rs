use crate::*;

pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let account_list = context.users.get();
    let username = creamui_router::use_params()
        .remove("username")
        .unwrap_or_default();
    users::account_view(size, &account_list, &username)
}
