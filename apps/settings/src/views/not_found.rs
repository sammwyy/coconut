use crate::components::pages::unavailable_page;

pub(crate) fn route_view(name: &str) -> creamui_core::BoxedWidget {
    unavailable_page(name)
}
