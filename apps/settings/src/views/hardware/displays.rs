use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let snapshot = state.snapshot.get();
    let mut body = Vec::new();
    for display in snapshot.entries("displays") {
        let key = format!("display:{}", display.id);
        let mut rows = vec![key_value("Current mode", &display.description)];
        if snapshot.preferences.contains_key(&key) {
            rows.push(state.preference(
                "Resolution & refresh rate",
                &key,
                "Try a mode; confirm within 15 seconds to keep it",
            ));
        }
        if display
            .properties
            .get("pending-confirmation")
            .is_some_and(|value| value == "true")
        {
            let seconds = display
                .properties
                .get("confirmation-seconds")
                .map(String::as_str)
                .unwrap_or("15");
            rows.push(value(format!(
                "Keep this resolution? Reverting in {seconds} seconds."
            )));
            let keep = state.clone();
            let output = display.id.clone();
            rows.push(Box::new(
                creamui_widgets::Button::new("Keep changes", move || {
                    keep.apply(coconut_api::settings::Action::ConfirmDisplayMode {
                        output: output.clone(),
                    })
                })
                .disabled(state.busy.get()),
            ));
            let revert = state.clone();
            let output = display.id.clone();
            rows.push(Box::new(
                creamui_widgets::Button::new("Revert", move || {
                    revert.apply(coconut_api::settings::Action::RevertDisplayMode {
                        output: output.clone(),
                    })
                })
                .disabled(state.busy.get()),
            ));
        }
        body.push(card(&display.name, rows));
    }
    if body.is_empty() {
        body.push(value(if snapshot.loaded {
            "No connected displays reported"
        } else {
            "Loading displays…"
        }));
    }
    detail_page(body, state)
}
