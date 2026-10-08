use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, routes::detail::Page};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let nav = &context.view;
    let s = state.snapshot.get();
    let body = vec![
        value(s.fact("printer-service")),
        entries(state, "printers", "No printers reported"),
        detail_link(
            "Add printer",
            "IPP / IPPS · driverless printing",
            Page::AddPrinter,
            nav,
        ),
    ];
    detail_page(body, state)
}
