use coconut_core::{ordered_islands, BarEdit, BarKind, DockConfig, ShellConfig};
use creamui_core::{BoxedWidget, Key, Point, Size};
use creamui_reactive::Signal;
use creamui_render::{ContextMenuHost, PopupOptions, WindowHandle};
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, ContextMenuItem, Dialog, DialogContent,
    DialogFooter, DialogHeader, DialogMessage,
};
use std::rc::Rc;

pub struct BarItem {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const BAR_ITEMS: &[BarItem] = &[
    BarItem {
        id: "app_launcher",
        label: "App launcher",
        description: "Browse and search your applications",
    },
    BarItem {
        id: "open_windows",
        label: "Open windows",
        description: "Switch between running applications",
    },
    BarItem {
        id: "workspaces",
        label: "Workspaces",
        description: "Navigate your desktop workspaces",
    },
    BarItem {
        id: "clock",
        label: "Clock",
        description: "Time, date and calendar",
    },
    #[cfg(target_os = "linux")]
    BarItem {
        id: "clipboard",
        label: "Clipboard history",
        description: "Find and reuse recently copied text",
    },
    BarItem {
        id: "current_playing",
        label: "Now playing",
        description: "Media controls; appears while media is playing",
    },
    BarItem {
        id: "weather",
        label: "Weather",
        description: "Current conditions and forecast",
    },
    BarItem {
        id: "control_center",
        label: "Control center",
        description: "Quick access to system controls",
    },
    BarItem {
        id: "tray.wifi",
        label: "Wi-Fi",
        description: "Wireless network status and connections",
    },
    BarItem {
        id: "tray.bluetooth",
        label: "Bluetooth",
        description: "Bluetooth status and devices",
    },
    BarItem {
        id: "tray.battery",
        label: "Battery",
        description: "Battery status and power controls",
    },
    BarItem {
        id: "tray.volume",
        label: "Volume",
        description: "Sound level and output devices",
    },
    BarItem {
        id: "tray.brightness",
        label: "Brightness",
        description: "Adjust display brightness",
    },
];

pub fn item_label(id: &str) -> &str {
    BAR_ITEMS
        .iter()
        .find(|item| item.id == id)
        .map_or(id, |item| item.label)
}

#[derive(Clone, Copy)]
pub enum Target {
    Bar,
    Group(usize),
    Item { group: usize, item: usize },
}

pub type EditHandler = Rc<dyn Fn(BarEdit)>;

pub fn content_menu(
    kind: BarKind,
    bar: &DockConfig,
    target: Target,
    edit: EditHandler,
    settings: Rc<dyn Fn(String)>,
) -> Vec<ContextMenuItem> {
    let mut items = Vec::new();
    let group = match target {
        Target::Group(group) | Target::Item { group, .. } => Some(group),
        Target::Bar => None,
    };
    let route = group.map_or_else(
        || kind.route().to_owned(),
        |group| format!("{}&group={group}", kind.route()),
    );
    if let Target::Item { group, item } = target {
        if let Some(entry) = bar
            .sections
            .get(group)
            .and_then(|section| ordered_islands(&section.islands).get(item).copied())
        {
            if let Some(route) = item_settings_route(&entry.id) {
                let settings = settings.clone();
                items.push(ContextMenuItem::new(
                    "item-settings",
                    "Item settings…",
                    move || settings(route.into()),
                ));
            }
            items.push(ContextMenuItem::new(
                "remove-item",
                "Remove item",
                move || edit(BarEdit::RemoveItem { group, item }),
            ));
        }
    } else if let Target::Group(group) = target {
        if group < bar.sections.len() {
            let open = settings.clone();
            let route = route.clone();
            items.push(ContextMenuItem::new(
                "edit-group",
                "Edit group…",
                move || open(route.clone()),
            ));
            items.push(ContextMenuItem::new(
                "remove-group",
                "Remove group",
                move || edit(BarEdit::RemoveGroup(group)),
            ));
            return items;
        }
    }
    if !items.is_empty() {
        items.push(ContextMenuItem::separator());
    }
    items.push(ContextMenuItem::new(
        "settings",
        if kind == BarKind::Statusbar {
            "Bar settings…"
        } else {
            "Dock settings…"
        },
        move || settings(route.clone()),
    ));
    items
}

pub fn item_settings_route(id: &str) -> Option<&'static str> {
    match id {
        "app_launcher" => Some("/desktop/widgets/app_launcher"),
        "open_windows" => Some("/desktop/widgets/open_windows"),
        "clock" => Some("/desktop/widgets"),
        "clipboard" => Some("/privacy/clipboard"),
        "control_center" | "tray.wifi" | "tray.bluetooth" | "tray.battery" | "tray.volume"
        | "tray.brightness" => Some("/desktop/tray"),
        _ => None,
    }
}

#[derive(Clone)]
struct Undo {
    kind: BarKind,
    before: DockConfig,
    after: DockConfig,
}

#[derive(Clone)]
pub struct BarEditor {
    pub menu: ContextMenuHost,
    pub status: Signal<String>,
    pub busy: Signal<bool>,
    error: Signal<Option<String>>,
    undo: Signal<Option<Undo>>,
    native_errors: bool,
}

impl Default for BarEditor {
    fn default() -> Self {
        Self {
            menu: ContextMenuHost::new(),
            status: Signal::new(String::new()),
            busy: Signal::new(false),
            error: Signal::new(None),
            undo: Signal::new(None),
            native_errors: false,
        }
    }
}

impl BarEditor {
    pub fn for_shell() -> Self {
        Self {
            native_errors: true,
            ..Self::default()
        }
    }

    pub fn report_error(&self, error: String) {
        self.status.set(error.clone());
        self.error.set(Some(error));
    }

    pub fn handler(
        &self,
        window: WindowHandle,
        kind: BarKind,
        expected: DockConfig,
        saved: Rc<dyn Fn(ShellConfig)>,
    ) -> EditHandler {
        let editor = self.clone();
        Rc::new(move |edit| editor.apply(&window, kind, expected.clone(), edit, saved.clone()))
    }

    pub fn can_undo(&self, kind: BarKind, current: &DockConfig) -> bool {
        self.undo
            .get()
            .is_some_and(|undo| undo.kind == kind && undo.after == *current)
    }

    pub fn undo(
        &self,
        window: &WindowHandle,
        kind: BarKind,
        current: &DockConfig,
        saved: Rc<dyn Fn(ShellConfig)>,
    ) {
        if let Some(undo) = self
            .undo
            .peek()
            .filter(|undo| undo.kind == kind && undo.after == *current)
        {
            self.apply(
                window,
                kind,
                current.clone(),
                BarEdit::Restore(undo.before),
                saved,
            );
        }
    }

    fn apply(
        &self,
        window: &WindowHandle,
        kind: BarKind,
        expected: DockConfig,
        edit: BarEdit,
        saved: Rc<dyn Fn(ShellConfig)>,
    ) {
        if self.busy.peek() {
            return;
        }
        self.busy.set(true);
        self.status.set("Saving changes…".into());
        let editor = self.clone();
        let restoring = matches!(edit, BarEdit::Restore(_));
        let parent = window.clone();
        window.app().spawn_background(
            move || {
                let mut config = ShellConfig::read_existing().map_err(|error| error.to_string())?;
                edit.apply_checked(kind, &expected, &mut config)?;
                config
                    .save()
                    .map_err(|error| format!("Could not save the bar: {error}"))?;
                if let Err(error) = coconut_core::ipc::publish_shell_config(&config) {
                    eprintln!("bar editor: could not notify the shell: {error}");
                }
                Ok((config, expected))
            },
            move |result: Result<(ShellConfig, DockConfig), String>| {
                creamui_reactive::batch(|| {
                    editor.busy.set(false);
                    match result {
                        Ok((config, before)) => {
                            editor.undo.set((!restoring).then(|| Undo {
                                kind,
                                before,
                                after: kind.get(&config).clone(),
                            }));
                            editor.status.set(
                                if restoring {
                                    "Change undone"
                                } else {
                                    "Changes saved"
                                }
                                .into(),
                            );
                            saved(config);
                        }
                        Err(error) => {
                            eprintln!("bar editor: {error}");
                            editor.report_error(error.clone());
                            if editor.native_errors {
                                editor.error.set(None);
                                show_error_window(&parent, error);
                            }
                        }
                    }
                });
            },
        );
    }

    pub fn open(&self, window: WindowHandle, point: Point, items: Vec<ContextMenuItem>) {
        self.menu
            .open(PopupOptions::at_cursor(window, point), items);
    }

    pub fn handle_key(&self, key: &Key, pressed: bool) -> bool {
        if !pressed {
            return false;
        }
        if self.error.peek().is_some() && *key == Key::Escape {
            self.error.set(None);
            return true;
        }
        self.menu.handle_key(key)
    }

    pub fn overlays(&self, size: Size) -> Vec<BoxedWidget> {
        let overlay = creamui_reactive::with_context_scope(|| {
            let mut theme = creamui_theme::use_theme();
            theme.colors.surface_elevated.a = 255;
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            self.menu.overlay(size)
        });
        let mut overlays = overlay.into_iter().collect::<Vec<_>>();
        if let Some(error) = self.error.get() {
            let editor = self.clone();
            overlays.push(Box::new(error_dialog(
                error,
                Rc::new(move || editor.error.set(None)),
                (size.width - 32.0).clamp(240.0, 440.0),
            )));
        }
        overlays
    }
}

pub fn key_input(
    key: &creamui_render::platform::Key,
    modifiers: creamui_render::platform::Modifiers,
) -> Option<creamui_core::KeyInput> {
    use creamui_render::platform::Key as NativeKey;
    let key = match key {
        NativeKey::Character(text) => Key::Char(text.chars().next()?),
        NativeKey::Space => Key::Char(' '),
        NativeKey::Escape => Key::Escape,
        NativeKey::Enter => Key::Enter,
        NativeKey::Tab => Key::Tab,
        NativeKey::Up => Key::Up,
        NativeKey::Down => Key::Down,
        NativeKey::Left => Key::Left,
        NativeKey::Right => Key::Right,
        NativeKey::Home => Key::Home,
        NativeKey::End => Key::End,
        NativeKey::Backspace => Key::Backspace,
        NativeKey::Delete => Key::Delete,
        NativeKey::PageUp => Key::PageUp,
        NativeKey::PageDown => Key::PageDown,
        _ => return None,
    };
    Some(creamui_core::KeyInput {
        key,
        modifiers: creamui_core::Modifiers {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            logo: modifiers.logo,
        },
    })
}

fn error_dialog(message: String, dismiss: Rc<dyn Fn()>, width: f32) -> Dialog {
    let backdrop = dismiss.clone();
    let header = dismiss.clone();
    Dialog::custom(move || backdrop())
        .width(width)
        .header(Box::new(
            DialogHeader::new("Could not change the bar").on_close(move || header()),
        ))
        .content(Box::new(
            DialogContent::new().child(Box::new(DialogMessage::new(message))),
        ))
        .footer(Box::new(DialogFooter::new().child(Box::new(
            Button::styled(
                ButtonVariant::Secondary,
                ButtonSize::Sm,
                "Close",
                ButtonState::Normal,
                move || dismiss(),
            ),
        ))))
}

fn show_error_window(parent: &WindowHandle, message: String) {
    let window = Rc::new(std::cell::RefCell::new(None::<WindowHandle>));
    let ready = window.clone();
    parent.app().append_window(
        creamui_render::WindowOptions {
            title: "Bar settings".into(),
            width: 480,
            height: 240,
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
            let window = window.clone();
            Box::new(error_dialog(
                message.clone(),
                Rc::new(move || {
                    if let Some(window) = window.borrow().as_ref() {
                        window.close();
                    }
                }),
                440.0,
            ))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_widgets::ContextMenuController;
    use std::cell::RefCell;

    #[test]
    fn contextual_actions_stay_short_and_remove_only_the_target() {
        creamui_reactive::with_context_scope(|| {
            let edits = Rc::new(RefCell::new(Vec::new()));
            let changed = edits.clone();
            let items = content_menu(
                BarKind::Dockbar,
                &DockConfig::default(),
                Target::Item { group: 0, item: 1 },
                Rc::new(move |edit| changed.borrow_mut().push(edit)),
                Rc::new(|_| {}),
            );
            assert!(items.len() <= 4);
            assert!(items.iter().all(|item| item.children.is_empty()));
            let remove = items
                .into_iter()
                .find(|item| item.id == "remove-item")
                .unwrap();
            let controller = ContextMenuController::new();
            controller.open(Point::default(), vec![remove]);
            controller.handle_key(&Key::Home);
            controller.handle_key(&Key::Enter);
            assert!(matches!(
                &edits.borrow()[0],
                BarEdit::RemoveItem { group: 0, item: 1 }
            ));
            let group = content_menu(
                BarKind::Statusbar,
                &DockConfig::default(),
                Target::Group(0),
                Rc::new(|_| {}),
                Rc::new(|_| {}),
            );
            assert_eq!(group.len(), 2);
            assert!(group.iter().all(|item| item.children.is_empty()));
        });
    }

    #[test]
    fn settings_opens_the_matching_content_route() {
        creamui_reactive::with_context_scope(|| {
            for kind in [BarKind::Statusbar, BarKind::Dockbar] {
                let route = Rc::new(RefCell::new(String::new()));
                let opened = route.clone();
                let menu = ContextMenuController::new();
                menu.open(
                    Point::default(),
                    content_menu(
                        kind,
                        &DockConfig::default(),
                        Target::Bar,
                        Rc::new(|_| {}),
                        Rc::new(move |value| *opened.borrow_mut() = value),
                    ),
                );
                menu.handle_key(&Key::End);
                menu.handle_key(&Key::Enter);
                assert_eq!(*route.borrow(), kind.route());
            }
        });
    }
}
