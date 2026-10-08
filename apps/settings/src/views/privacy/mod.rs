use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    build(native, view)
}

fn build(state: &State, nav: &creamui_router::Router) -> BoxedWidget {
    let s = state.snapshot.get();
    page(
        vec![
            card(
                "Security",
                vec![
                    detail_link("Fingerprint", s.fact("fingerprint"), Page::Fingerprint, nav),
                    state.preference(
                        "Require password after sleep",
                        "lock-after-sleep",
                        "Session screen-lock policy",
                    ),
                ],
            ),
            card(
                "Permissions",
                vec![
                    state.preference(
                        "Location services",
                        "location",
                        "Applications may request your location",
                    ),
                    detail_link(
                        "Camera permissions",
                        "Saved desktop-portal permissions",
                        Page::Camera,
                        nav,
                    ),
                    detail_link(
                        "Microphone permissions",
                        "Saved permissions and input device",
                        Page::Microphone,
                        nav,
                    ),
                    unsupported(
                        "Usage analytics",
                        "Coconut has no analytics setting or telemetry service to enable",
                    ),
                ],
            ),
        ],
        state,
    )
}
pub mod camera;
pub mod fingerprint;
pub mod microphone;
