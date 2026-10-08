use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let id = creamui_router::use_params()
        .remove("app_id")
        .unwrap_or_default();
    let id = &id;
    let body = {
        let app = s.entries("applications").iter().find(|e| &e.id == id);
        app.map(|app| {
            vec![card(
                "Application",
                vec![
                    key_value("Name", &app.name),
                    key_value("Desktop ID", &app.id),
                    key_value("Description", &app.description),
                    key_value(
                        "Executable",
                        app.properties.get("Exec").cloned().unwrap_or_default(),
                    ),
                    key_value(
                        "Desktop entry",
                        app.properties.get("path").cloned().unwrap_or_default(),
                    ),
                ],
            )]
        })
        .unwrap_or_else(|| vec![value("This application is no longer installed")])
    };
    detail_page(body, state)
}
