use crate::components::group;
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::Switch;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    permissions(true)
}

pub(super) fn permissions(camera: bool) -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let s = state.snapshot.get();
    let body = {
        let key = if camera {
            "camera-permissions"
        } else {
            "microphone-permissions"
        };
        let device = if camera { "camera" } else { "microphone" };
        let rows = s
            .entries(key)
            .iter()
            .map(|e| {
                let write = state.clone();
                let app = e.id.clone();
                let allowed = e.enabled;
                item(
                    &e.name,
                    &e.description,
                    Box::new(Switch::new(allowed, move || {
                        write.apply(Action::SetDevicePermission {
                            device: device.into(),
                            app: app.clone(),
                            allowed: !allowed,
                        })
                    })),
                    || {},
                )
            })
            .collect::<Vec<_>>();
        let mut body = vec![value("Permissions saved by the desktop portal. These permissions do not restrict applications that access hardware directly."), value(s.fact(if camera { "camera-service" } else { "microphone-service" })),
                group(if rows.is_empty() { vec![key_value("Applications", "No saved permissions reported")] } else { rows })];
        if camera {
            body.push(key_value("Camera devices", s.fact("camera-devices")));
        } else {
            body.push(card(
                "Audio input",
                vec![
                    state.preference("Input device", "sound-input", "Default recording device"),
                    state.preference(
                        "Mute microphone",
                        "microphone-muted",
                        "Mute the default audio source",
                    ),
                ],
            ));
        }
        body
    };
    detail_page(body, state)
}
