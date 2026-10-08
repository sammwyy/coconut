use coconut_api::settings::Action;
use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let body = {
        let write = state.clone();
        vec![value("Checks PackageKit when available, otherwise Flatpak applications. No packages are installed or removed automatically."), action("Check now", move || write.apply(Action::CheckUpdates))]
    };
    detail_page(body, state)
}
