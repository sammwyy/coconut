use crate::*;

pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    users::create_user_view(size)
}
