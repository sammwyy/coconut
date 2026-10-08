use coconut_api::settings::Action;
use creamui_core::{BoxedWidget, Styled};
use creamui_widgets::TextInput;

use crate::{components::system_settings::*, services::settings::State};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = password_form(state);
    detail_page(body, state)
}

fn password_form(state: &State) -> Vec<BoxedWidget> {
    let write = state.clone();
    vec![value("Changes only your own system account password through AccountsService. System authorization is required."),
        card("New password", vec![item("Password", "At least 8 characters", Box::new(TextInput::controlled(&state.password).password().width(260.0)), || {}),
            item("Confirm password", "Enter the same password again", Box::new(TextInput::controlled(&state.confirmation).password().width(260.0)), || {}),
            item("", "", action("Change password", move || {
                let password = write.password.value(); if password != write.confirmation.value() { write.status.set("The passwords do not match".into()); return; }
                if password.len() < 8 { write.status.set("Use at least 8 characters".into()); return; }
                if write.busy.peek() { return; }
                write.password.set_value(""); write.confirmation.set_value(""); write.apply(Action::ChangePassword { password });
            }), || {})])]
}
