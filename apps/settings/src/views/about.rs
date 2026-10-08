use coconut_api::settings::{Action, Value};
use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let native = &context.native;
    build(native)
}

fn build(state: &State) -> BoxedWidget {
    let s = state.snapshot.get();
    let write = state.clone();
    let mut identity = vec![item(
        "Hostname",
        "Device name on the network",
        input(&state.hostname, s.fact("hostname")),
        || {},
    )];
    if s.preferences.get("hostname").is_some_and(|p| p.writable) {
        identity.push(item(
            "",
            "Requires authorization",
            action("Apply", move || {
                write.apply(Action::Set {
                    key: "hostname".into(),
                    value: Value::Text(write.hostname.value()),
                })
            }),
            || {},
        ));
    }
    page(
        vec![
            card("Device", identity),
            card(
                "System",
                vec![
                    key_value("Operating system", s.fact("os")),
                    key_value("Kernel", s.fact("kernel")),
                    key_value("Desktop", &s.session),
                ],
            ),
            card(
                "Hardware",
                vec![
                    key_value("Processor", s.fact("processor")),
                    key_value("Logical CPUs", s.fact("cores")),
                    key_value("Memory", s.fact("memory")),
                    key_value("Graphics", s.fact("graphics")),
                ],
            ),
            card(
                "Software",
                vec![key_value("Coconut", env!("CARGO_PKG_VERSION"))],
            ),
        ],
        state,
    )
}
