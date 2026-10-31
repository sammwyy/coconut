use crate::{
    components::{group, system_settings::*},
    views::context::ViewContext,
};
use coconut_api::settings::Action;
use creamui_core::BoxedWidget;
use creamui_widgets::{layout::Flex, Button, ButtonSize, Switch};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<ViewContext>();
    build(&context.native)
}

fn build(state: &crate::services::settings::State) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    let mut rows = Vec::new();
    for entry in snapshot.entries("clipboard-permissions") {
        let Some(permission) = entry.properties.get("permission") else {
            continue;
        };
        let write = state.clone();
        let reset = state.clone();
        let subject = entry.id.clone();
        let reset_subject = subject.clone();
        let permission = permission.clone();
        let reset_permission = permission.clone();
        let allowed = entry.enabled;
        rows.push(item(
            &entry.name,
            &entry.description,
            Box::new(
                Flex::row()
                    .gap(8.0)
                    .child(Box::new(Switch::new(allowed, move || {
                        write.apply(Action::SetClipboardPermission {
                            subject: subject.clone(),
                            permission: permission.clone(),
                            allowed: !allowed,
                        })
                    })))
                    .child(Box::new(Button::secondary(
                        ButtonSize::Xs,
                        "Reset",
                        move || {
                            reset.apply(Action::RevokeClipboardPermission {
                                subject: reset_subject.clone(),
                                permission: reset_permission.clone(),
                            })
                        },
                    ))),
            ),
            || {},
        ));
    }
    if rows.is_empty() {
        rows.push(key_value("Applications", "No saved clipboard permissions"));
    }
    let mut body = vec![
        value(snapshot.fact("clipboard-service")),
        group(vec![
            state.preference("Keep clipboard history", "clipboard-enabled", "Pause saving new entries; existing history remains until it expires"),
            state.preference("History size", "clipboard-max-entries", "Maximum number of saved text entries"),
            state.preference("Keep entries for", "clipboard-max-age", "Expired entries are removed automatically"),
        ]),
        value("Applications request separate permissions for the current clipboard, history, and writing. Reset removes a saved decision so the next request asks again. The installed shell and Settings have access by default."),
        group(rows),
    ];
    if let Some(error) = snapshot.facts.get("clipboard-permission-service") {
        body.push(value(error));
    }
    detail_page(body, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_api::settings::{Choice, Entry, Preference, Snapshot, Value};
    use creamui_core::{layout::Style, Point, Renderer, Size};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Theme, ThemeProvider};
    use creamui_widgets::{layout::fixed, ScrollController, ScrollView};

    #[test]
    fn clipboard_preferences_and_saved_permission_controls_render() {
        for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            for size in [
                Size {
                    width: 900.0,
                    height: 700.0,
                },
                Size {
                    width: 600.0,
                    height: 430.0,
                },
            ] {
                creamui_reactive::with_context_scope(|| {
                    crate::components::load_settings_fonts();
                    let theme = crate::components::settings_theme(theme);
                    creamui_reactive::provide_context(ThemeProvider::new(theme));
                    let state = crate::services::settings::State::new(std::sync::Arc::new(
                        coconut_api::settings::Fallback,
                    ));
                    let mut snapshot = Snapshot {
                        loaded: true,
                        ..Default::default()
                    };
                    snapshot.facts.insert(
                        "clipboard-service".into(),
                        "Coconut clipboard service · text history stays in memory".into(),
                    );
                    snapshot.preferences.insert(
                        "clipboard-enabled".into(),
                        Preference {
                            value: Some(Value::Bool(true)),
                            writable: true,
                            ..Default::default()
                        },
                    );
                    for (key, value, label) in [
                        ("clipboard-max-entries", "50", "50 entries"),
                        ("clipboard-max-age", "86400", "24 hours"),
                    ] {
                        snapshot.preferences.insert(
                            key.into(),
                            Preference {
                                value: Some(Value::Text(value.into())),
                                writable: true,
                                choices: vec![Choice {
                                    id: value.into(),
                                    label: label.into(),
                                }],
                                ..Default::default()
                            },
                        );
                    }
                    snapshot.collections.insert(
                        "clipboard-permissions".into(),
                        vec![Entry {
                            id: "/opt/example/application".into(),
                            name: "Example application".into(),
                            description: "clipboard-history · /opt/example/application".into(),
                            enabled: true,
                            properties: [("permission".into(), "clipboard-history".into())].into(),
                        }],
                    );
                    state.snapshot.set(snapshot);
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(
                        size.width as u32,
                        size.height as u32,
                        1.0,
                        theme.colors.surface,
                        theme.colors,
                    );
                    let widget = ScrollView::controlled(
                        Style {
                            size: fixed(size.width, size.height),
                            ..Default::default()
                        },
                        ScrollController::default(),
                    )
                    .child(build(&state));
                    let scene = Renderer::new().render(Box::new(widget), size, &mut recorder);
                    assert!(
                        scene.on_key_at(4).is_some(),
                        "saved permission Reset control is focusable"
                    );
                    assert!(
                        (0..size.height as u32)
                            .step_by(5)
                            .any(|y| (0..size.width as u32).step_by(5).any(|x| scene
                                .focus_hit_test(Point {
                                    x: x as f32,
                                    y: y as f32
                                })
                                == Some(0))),
                        "collection toggle is visible"
                    );
                    let display = recorder.finish();
                    assert!(display.items.len() > 20);
                    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                    raster.render(&display, &Damage::Full);
                    assert_ne!(raster.pixmap().pixel(10, 10).unwrap().alpha(), 0);
                    if let Ok(prefix) = std::env::var("COCONUT_CLIPBOARD_SETTINGS_SNAPSHOT") {
                        raster
                            .pixmap()
                            .save_png(format!("{prefix}-{variant}-{}.png", size.width))
                            .unwrap();
                    }
                });
            }
        }
    }
}
