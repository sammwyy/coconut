use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let body = vec![key_value("Reader", s.fact("fingerprint")), entries(state, "fingerprints", "No enrolled fingerprints reported"), value("Fingerprint enrollment and unlocking are managed by fprintd and the system PAM policy. Coconut does not override authentication policy.")];
    detail_page(body, state)
}
