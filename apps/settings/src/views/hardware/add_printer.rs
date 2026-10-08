use coconut_api::settings::Action;
use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = {
        let write = state.clone();
        vec![card(
            "Printer",
            vec![
                item(
                    "Name",
                    "Letters, digits, hyphens and underscores",
                    input(&state.printer_name, "Office-printer"),
                    || {},
                ),
                item(
                    "Address",
                    "Driverless IPP or IPPS endpoint",
                    input(&state.printer_uri, "ipps://printer.local/ipp/print"),
                    || {},
                ),
                item(
                    "",
                    "CUPS must be running; permission is checked by the server",
                    action("Add printer", move || {
                        write.apply(Action::AddPrinter {
                            name: write.printer_name.value(),
                            uri: write.printer_uri.value(),
                        })
                    }),
                    || {},
                ),
            ],
        )]
    };
    detail_page(body, state)
}
