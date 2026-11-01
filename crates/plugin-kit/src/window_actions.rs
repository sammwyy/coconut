use crate::{launch_app, pins::Pins, AppCatalog, AppEntry, SharedState};
use coconut_api::desktop::{DesktopIntegration, OpenWindow, WindowAction};
use creamui_core::BoxedWidget;
use creamui_render::{WindowHandle, WindowOptions};
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, ContextMenuItem, Dialog, DialogContent,
    DialogFooter, DialogHeader, DialogMessage, ModalActions,
};
use std::cell::RefCell;
use std::{path::Path, rc::Rc};

#[derive(Clone)]
pub struct WindowActions(pub Rc<dyn DesktopIntegration>);

pub fn application_for_window(apps: &[AppEntry], window: &OpenWindow) -> Option<AppEntry> {
    let identity = window.app_name.trim_end_matches(".desktop");
    if identity.is_empty() {
        return apps
            .iter()
            .find(|app| {
                window.title == app.name || window.title.ends_with(&format!(" - {}", app.name))
            })
            .cloned();
    }

    apps.iter()
        .find(|app| {
            app.desktop_file
                .as_ref()
                .and_then(|path| path.file_stem())
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.eq_ignore_ascii_case(identity))
        })
        .or_else(|| {
            apps.iter().find(|app| {
                app.exec
                    .first()
                    .and_then(|program| Path::new(program).file_name())
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(identity))
            })
        })
        .cloned()
}

pub fn window_menu(
    window: &OpenWindow,
    shared: &SharedState,
    activate: Rc<dyn Fn(String)>,
    host: &crate::InteractionHost,
) -> Vec<ContextMenuItem> {
    let mut items = Vec::new();
    let id = window.id.clone();
    if let Some(actions) = shared.get::<WindowActions>() {
        for (key, label, action) in [
            ("restore", "Restore", WindowAction::Restore),
            ("minimize", "Minimize", WindowAction::Minimize),
            (
                "maximize",
                "Maximize / Restore size",
                WindowAction::ToggleMaximize,
            ),
        ] {
            if actions.0.supports_window_action(action) {
                let backend = actions.0.clone();
                let id = id.clone();
                items.push(ContextMenuItem::new(key, label, move || {
                    backend.window_action(&id, action)
                }));
            }
        }
    } else {
        items.push(ContextMenuItem::new("restore", "Restore", move || {
            activate(id.clone())
        }));
    }
    if let Some(app) = shared
        .get::<AppCatalog>()
        .and_then(|catalog| application_for_window(&catalog.apps(), window))
    {
        let open = app.clone();
        items.push(ContextMenuItem::new("open", "Open new window", move || {
            launch_app(&open)
        }));
        if let (Some(pins), Some(path)) = (shared.get::<Pins>(), app.desktop_file) {
            let pinned = pins.contains(&path);
            let busy = pins.busy();
            items.push(
                ContextMenuItem::new(
                    "pin",
                    if pinned {
                        "Unpin application"
                    } else {
                        "Pin application"
                    },
                    move || {
                        if pinned {
                            pins.unpin(path.clone());
                        } else {
                            pins.pin(vec![path.clone()]);
                        }
                    },
                )
                .enabled(!busy),
            );
        }
    }
    if let Some(actions) = shared.get::<WindowActions>() {
        let closing: Vec<_> = [
            ("close", "Close window", WindowAction::Close),
            (
                "force-close",
                "Force quit application",
                WindowAction::ForceClose,
            ),
        ]
        .into_iter()
        .filter(|(_, _, action)| actions.0.supports_window_action(*action))
        .map(|(key, label, action)| {
            let backend = actions.0.clone();
            let id = window.id.clone();
            let parent = host.window();
            let enabled = action != WindowAction::ForceClose || parent.is_some();
            ContextMenuItem::new(key, label, move || {
                if action == WindowAction::ForceClose {
                    if let Some(parent) = &parent {
                        let backend = backend.clone();
                        let id = id.clone();
                        confirm_force_quit(
                            parent,
                            Rc::new(move || backend.window_action(&id, action)),
                        );
                    }
                } else {
                    backend.window_action(&id, action);
                }
            })
            .enabled(enabled)
        })
        .collect();
        if !closing.is_empty() {
            items.push(ContextMenuItem::separator());
            items.extend(closing);
        }
    }
    items
}

fn force_quit_dialog(dismiss: Rc<dyn Fn()>, action: Rc<dyn Fn()>) -> Dialog {
    let backdrop = dismiss.clone();
    let cancel = dismiss.clone();
    Dialog::custom(move || backdrop()).width(412.0)
        .header(Box::new(DialogHeader::new("Force quit application?").compact()))
        .content(Box::new(DialogContent::new().compact().child(Box::new(DialogMessage::new("All windows belonging to this application will close. Unsaved changes will be lost.")))))
        .footer(Box::new(DialogFooter::new().compact().child(Box::new(ModalActions::new()
            .child(Button::styled(ButtonVariant::Secondary, ButtonSize::Sm, "Cancel", ButtonState::Normal, move || cancel()))
            .child(Button::styled(ButtonVariant::Destructive, ButtonSize::Sm, "Force quit", ButtonState::Normal, move || { dismiss(); action(); }))))))
}

fn confirm_force_quit(parent: &WindowHandle, action: Rc<dyn Fn()>) {
    let window = Rc::new(RefCell::new(None::<WindowHandle>));
    let ready = window.clone();
    parent.app().append_window(
        WindowOptions {
            title: "Force quit application".into(),
            width: 460,
            height: 280,
            resizable: false,
            theme: parent.theme(),
            ..Default::default()
        },
        parent.theme().colors.surface,
        move |handle| {
            let close = handle.clone();
            handle.on_keyboard_input(move |event, _| {
                if event.pressed && event.key == creamui_render::platform::Key::Escape {
                    close.close();
                    return true;
                }
                false
            });
            *ready.borrow_mut() = Some(handle);
        },
        move |_| -> BoxedWidget {
            let close = window.clone();
            Box::new(force_quit_dialog(
                Rc::new(move || {
                    if let Some(window) = close.borrow().as_ref() {
                        window.close();
                    }
                }),
                action.clone(),
            ))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_widgets::ContextMenuController;

    struct Backend(Rc<RefCell<Vec<(String, WindowAction)>>>);
    impl DesktopIntegration for Backend {
        fn prepare_window(&self, _: &WindowHandle) {}
        fn windows(&self) -> Vec<OpenWindow> {
            Vec::new()
        }
        fn activate_window(&self, _: &str) {
            panic!("Restore must not toggle minimization")
        }
        fn supports_window_action(&self, action: WindowAction) -> bool {
            !matches!(action, WindowAction::ForceClose)
        }
        fn window_action(&self, id: &str, action: WindowAction) {
            self.0.borrow_mut().push((id.into(), action));
        }
    }
    fn window() -> OpenWindow {
        OpenWindow {
            id: "window-42".into(),
            app_name: "org.example.Editor".into(),
            title: "Notes".into(),
            icon_path: None,
            active: true,
        }
    }
    fn app() -> AppEntry {
        AppEntry {
            desktop_file: Some("/usr/share/applications/org.example.Editor.desktop".into()),
            name: "Example editor".into(),
            exec: vec!["editor".into()],
            terminal: false,
            category: "Development".into(),
            icon_name: None,
            icon: None,
        }
    }
    #[test]
    fn menus_dispatch_to_the_selected_window_without_ui_context() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let shared = SharedState::default();
        shared.insert(WindowActions(Rc::new(Backend(calls.clone()))));
        for (index, expected) in [
            WindowAction::Restore,
            WindowAction::Minimize,
            WindowAction::ToggleMaximize,
            WindowAction::Close,
        ]
        .into_iter()
        .enumerate()
        {
            let items = window_menu(
                &window(),
                &shared,
                Rc::new(|_| panic!("unexpected activation")),
                &crate::InteractionHost::new(),
            );
            assert!(items.iter().all(|item| item.children.is_empty()));
            assert!(!items.iter().any(|item| item.id == "force-close"));
            let menu = ContextMenuController::new();
            menu.open(Default::default(), items);
            for _ in 0..=index {
                menu.handle_key(&creamui_core::Key::Down);
            }
            menu.handle_key(&creamui_core::Key::Enter);
            assert_eq!(
                calls.borrow().last(),
                Some(&(String::from("window-42"), expected))
            );
        }
    }
    #[test]
    fn force_quit_confirmation_keeps_both_actions_visible_and_cancel_is_non_destructive() {
        use creamui_core::{Point, Renderer, Size};
        use creamui_render::{Damage, Rasterizer, SceneRecorder};
        use creamui_theme::{Theme, ThemeProvider};
        use std::cell::Cell;
        for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            for width in [360.0, 460.0] {
                creamui_reactive::with_context_scope(|| {
                    crate::design::load_fonts();
                    creamui_reactive::provide_context(ThemeProvider::new(theme));
                    let closed = Rc::new(Cell::new(0));
                    let forced = Rc::new(Cell::new(0));
                    let close = closed.clone();
                    let force = forced.clone();
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(width as u32, 240, 1.0, theme.colors.surface, theme.colors);
                    let size = Size {
                        width,
                        height: 240.0,
                    };
                    let scene = Renderer::new().render(
                        Box::new(force_quit_dialog(
                            Rc::new(move || close.set(close.get() + 1)),
                            Rc::new(move || force.set(force.get() + 1)),
                        )),
                        size,
                        &mut recorder,
                    );
                    let mut points = [None, None];
                    for y in 0..240 {
                        for x in 0..width as u32 {
                            let point = Point {
                                x: x as f32 + 0.5,
                                y: y as f32 + 0.5,
                            };
                            if let Some(index) =
                                scene.focus_hit_test(point).filter(|index| *index < 2)
                            {
                                points[index].get_or_insert(point);
                            }
                        }
                    }
                    scene
                        .hit_test(points[0].expect("Cancel remains visible"))
                        .unwrap()();
                    assert_eq!(closed.get(), 1);
                    assert_eq!(forced.get(), 0);
                    assert!(points[1].is_some(), "Force quit remains visible");
                    scene.on_key_at(1).unwrap()(creamui_core::KeyInput {
                        key: creamui_core::Key::Enter,
                        modifiers: Default::default(),
                    });
                    assert_eq!(closed.get(), 2);
                    assert_eq!(forced.get(), 1);
                    if let Ok(prefix) = std::env::var("COCONUT_SHELL_SNAPSHOT") {
                        let mut raster = Rasterizer::new(width as u32, 240);
                        raster.render(&recorder.finish(), &Damage::Full);
                        raster
                            .pixmap()
                            .save_png(format!("{prefix}-force-quit-{variant}-{width}.png"))
                            .unwrap();
                    }
                });
            }
        }
    }

    #[test]
    fn application_identity_matches_desktop_ids_and_executables_without_title_guessing() {
        let catalog = AppCatalog::from_entries(vec![app()]);
        for identity in ["org.example.Editor", "org.example.Editor.desktop", "EDITOR"] {
            let mut window = window();
            window.app_name = identity.into();
            assert_eq!(
                application_for_window(&catalog.apps(), &window)
                    .unwrap()
                    .name,
                "Example editor"
            );
        }
        let mut unknown = window();
        unknown.app_name = "other".into();
        unknown.title = "Example editor".into();
        assert!(application_for_window(&catalog.apps(), &unknown).is_none());
    }
}
