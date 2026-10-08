use coconut_api::settings::{Action, Value};
use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let body = {
        let write = state.clone();
        vec![
            card(
                "Clock",
                vec![
                    state.preference(
                        "Automatic date & time",
                        "ntp",
                        "Network time synchronization",
                    ),
                    item(
                        "Time zone",
                        "IANA name, such as America/Argentina/Buenos_Aires",
                        input(&state.timezone, "Area/City"),
                        || {},
                    ),
                    item(
                        "",
                        "Requires authorization",
                        action("Apply time zone", move || {
                            write.apply(Action::Set {
                                key: "timezone".into(),
                                value: Value::Text(write.timezone.value()),
                            })
                        }),
                        || {},
                    ),
                ],
            ),
            key_value("Uptime", s.fact("uptime")),
        ]
    };
    detail_page(body, state)
}
