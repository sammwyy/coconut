use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    build(native, view)
}

fn build(state: &State, nav: &creamui_router::Router) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    page(
        vec![
            card(
                "Defaults",
                vec![
                    state.preference("Web browser", "browser", "Open web links with"),
                    detail_link(
                        "File types",
                        "Default applications by MIME type",
                        Page::FileTypes,
                        nav,
                    ),
                ],
            ),
            card(
                "Management",
                vec![
                    detail_link(
                        "Installed applications",
                        format!(
                            "{} desktop applications",
                            snapshot.entries("applications").len()
                        ),
                        Page::InstalledApps,
                        nav,
                    ),
                    detail_link(
                        "Startup applications",
                        format!("{} entries", snapshot.entries("startup").len()),
                        Page::Startup,
                        nav,
                    ),
                ],
            ),
            card(
                "Search",
                vec![item(
                    "Find applications",
                    "Search installed applications by name",
                    input(&state.search, "Search applications…"),
                    || {},
                )],
            ),
            if state.search.value().is_empty() {
                empty()
            } else {
                installed::app_list(state, nav, false)
            },
        ],
        state,
    )
}
pub mod add_startup;
pub mod application;
pub mod file_types;
pub mod installed;
pub mod startup;
