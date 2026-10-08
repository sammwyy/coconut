use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::Switch;

use crate::{components::system_settings::*, services::settings::State};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = account_form(state);
    detail_page(body, state)
}

fn account_form(state: &State) -> Vec<BoxedWidget> {
    let write = state.clone();
    let admin = state.administrator.get();
    let admin_write = state.administrator.clone();
    vec![value("Creates a system account through AccountsService after administrator authorization. Set a password with the system account manager before signing in."),
        card("Account", vec![item("Username", "Lowercase letters, digits, hyphens and underscores", input(&state.username, "username"), || {}), item("Display name", "Name shown on this device", input(&state.real_name, "Full name"), || {}),
            item("Administrator", "Allow administrative operations", Box::new(Switch::new(admin, move || admin_write.set(!admin))), || {}),
            item("", "", action("Create account", move || write.apply(Action::CreateUser { username: write.username.value(), real_name: write.real_name.value(), administrator: write.administrator.peek() })), || {})])]
}
