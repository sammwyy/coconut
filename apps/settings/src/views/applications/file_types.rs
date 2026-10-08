use crate::components::group;
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::Select;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let body = {
        let rows = s
            .entries("file-types")
            .iter()
            .map(|e| {
                let mut apps = s
                    .entries("applications")
                    .iter()
                    .filter(|a| {
                        a.properties
                            .get("MimeType")
                            .is_some_and(|types| types.split(';').any(|mime| mime == e.id))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if apps.is_empty() {
                    return key_value(&e.name, &e.description);
                }
                let current = e.properties.get("desktop-id");
                let selected = apps
                    .iter()
                    .position(|a| Some(&a.id) == current)
                    .unwrap_or_else(|| {
                        apps.insert(
                            0,
                            coconut_api::settings::Entry {
                                name: format!("Current: {}", e.description),
                                ..Default::default()
                            },
                        );
                        0
                    });
                let labels: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
                let write = state.clone();
                let mime = e.id.clone();
                item(
                    &e.name,
                    &e.description,
                    Box::new(
                        Select::controlled(
                            &labels,
                            state.select(&format!("mime:{}", e.id), selected),
                        )
                        .searchable()
                        .on_select(move |i| {
                            if let Some(app) = apps.get(i).filter(|app| !app.id.is_empty()) {
                                write.apply(Action::SetDefaultApp {
                                    mime: mime.clone(),
                                    desktop_id: app.id.clone(),
                                });
                            }
                        }),
                    ),
                    || {},
                )
            })
            .collect::<Vec<_>>();
        vec![value("Explicit default associations saved by the desktop. Choose an installed application that advertises the file type."), group(if rows.is_empty() { vec![key_value("Associations", "No explicit defaults reported")] } else { rows })]
    };
    detail_page(body, state)
}
