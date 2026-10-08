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
                "Maintenance",
                vec![
                    detail_link(
                        "Updates",
                        "Check system and application updates",
                        Page::Updates,
                        nav,
                    ),
                    detail_link("Storage", snapshot.fact("storage"), Page::Storage, nav),
                    detail_link(
                        "Activity monitor",
                        "CPU, memory and running processes",
                        Page::Activity,
                        nav,
                    ),
                ],
            ),
            card(
                "Region",
                vec![
                    state.preference(
                        "Automatic date & time",
                        "ntp",
                        "Synchronize with network time servers",
                    ),
                    detail_link(
                        "Date & time",
                        "Time zone and clock synchronization",
                        Page::DateTime,
                        nav,
                    ),
                    state.preference(
                        "Language",
                        "locale",
                        "Installed system locales; authorization may be required",
                    ),
                    detail_link(
                        "Language & region",
                        "System locale and regional format",
                        Page::Language,
                        nav,
                    ),
                ],
            ),
            card(
                "Reset",
                vec![detail_link(
                    "Reset shell settings",
                    "Only Coconut shell layout; a backup is kept",
                    Page::Reset,
                    nav,
                )],
            ),
        ],
        state,
    )
}
pub mod activity;
pub mod date_time;
pub mod language;
pub mod reset;
pub mod storage;
pub mod updates;
