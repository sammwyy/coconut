mod discovery;
mod menu;
mod operations;
mod view;

use discovery::{inspect, properties, Snapshot};

use creamui_core::{BoxedWidget, Key, Point, Size, Styled};
use creamui_reactive::{batch, Signal};
use creamui_render::{ContextMenuHost, PopupOptions, WindowHandle};
use creamui_widgets::{
    ButtonVariant, ContextMenuItem, Dialog, DialogContent, DialogHeader, DialogMessage,
    TextController, TextInput,
};
use delib::{desktop_entry::DesktopEntry, mimeapps::MimeApps};
pub use operations::open_in_explorer;
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileMenuTarget {
    Item { path: PathBuf, directory: bool },
    Background(PathBuf),
}
impl FileMenuTarget {
    pub fn path(&self) -> &Path {
        match self {
            Self::Item { path, .. } | Self::Background(path) => path,
        }
    }
    fn directory(&self) -> PathBuf {
        match self {
            Self::Item {
                path,
                directory: false,
            } => path.parent().unwrap_or(path).to_path_buf(),
            _ => self.path().to_path_buf(),
        }
    }
}

#[derive(Clone)]
pub struct FileMenuOptions {
    pub open: Rc<dyn Fn(PathBuf, bool)>,
    pub open_label: String,
    pub refresh: Rc<dyn Fn()>,
    pub extras: Vec<ContextMenuItem>,
    pub editable: bool,
    pub show_properties: bool,
}
impl FileMenuOptions {
    pub fn new(open: impl Fn(PathBuf, bool) + 'static, refresh: impl Fn() + 'static) -> Self {
        Self {
            open: Rc::new(open),
            open_label: "Open".into(),
            refresh: Rc::new(refresh),
            extras: Vec::new(),
            editable: true,
            show_properties: true,
        }
    }
    pub fn open_label(mut self, label: impl Into<String>) -> Self {
        self.open_label = label.into();
        self
    }
    pub fn extras(mut self, items: Vec<ContextMenuItem>) -> Self {
        self.extras = items;
        self
    }
    pub fn properties(mut self, visible: bool) -> Self {
        self.show_properties = visible;
        self
    }
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }
}
#[derive(Clone)]
enum FormKind {
    NewFile,
    NewFolder,
    Rename,
}
#[derive(Clone)]
struct Form {
    kind: FormKind,
    path: PathBuf,
    window: WindowHandle,
    refresh: Rc<dyn Fn()>,
}
#[derive(Clone)]
enum Panel {
    Form(Form),
    Information { title: String, body: String },
}

#[derive(Clone)]
pub struct FileMenuController {
    menu: ContextMenuHost,
    generation: Rc<Cell<u64>>,
    pending: Rc<Cell<bool>>,
    panel: Signal<Option<Panel>>,
    name: TextController,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
}
impl Default for FileMenuController {
    fn default() -> Self {
        Self {
            menu: ContextMenuHost::new(),
            generation: Rc::new(Cell::new(0)),
            pending: Rc::new(Cell::new(false)),
            panel: Signal::new(None),
            name: TextController::new(""),
            error: Signal::new(None),
            busy: Signal::new(false),
        }
    }
}
impl FileMenuController {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel_pending(&self) {
        if self.pending.get() && !self.menu.is_open() {
            self.close_menu();
        }
    }
    pub fn close_menu(&self) {
        self.pending.set(false);
        self.generation.set(self.generation.get().wrapping_add(1));
        self.menu.close();
    }
    pub fn parent_focus_lost(&self) {
        if !self.menu.uses_native_popup() {
            self.close_menu();
        }
    }
    pub fn dismiss(&self) {
        self.close_menu();
        if !self.busy.peek() {
            batch(|| {
                self.panel.set(None);
                self.error.set(None);
            });
        }
    }
    pub fn open(
        &self,
        window: &WindowHandle,
        point: Point,
        target: FileMenuTarget,
        options: FileMenuOptions,
    ) {
        if self.busy.peek() || self.panel.peek().is_some() {
            return;
        }
        self.close_menu();
        self.pending.set(true);
        let generation = self.generation.get();
        let controller = self.clone();
        let popup = PopupOptions::at_cursor(window.clone(), point);
        let ready_window = window.clone();
        let inspect_target = target.clone();
        let initial = self.items(window, target.clone(), options.clone(), Snapshot::default());
        self.menu.open(popup, initial);
        window.app().spawn_background(
            move || inspect(&inspect_target),
            move |result| {
                if controller.generation.get() != generation {
                    return;
                }
                controller.pending.set(false);
                if !controller.menu.is_open() {
                    return;
                }
                match result {
                    Ok(snapshot) => {
                        let items = controller.items(&ready_window, target, options, snapshot);
                        controller.update_details(generation, items);
                    }
                    Err(error) => eprintln!(
                        "file-menu: could not inspect {}: {error}",
                        target.path().display()
                    ),
                }
            },
        );
    }
    fn update_details(&self, generation: u64, items: Vec<ContextMenuItem>) {
        if self.generation.get() != generation || !self.menu.is_open() {
            return;
        }
        for item in items
            .into_iter()
            .filter(|item| matches!(item.id.as_str(), "open-with" | "paste"))
        {
            self.menu.controller().update_item(item);
        }
    }
    pub fn perform(
        &self,
        window: &WindowHandle,
        refresh: impl Fn() + 'static,
        work: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        if self.busy.peek() {
            return;
        }
        self.busy.set(true);
        let controller = self.clone();
        window.app().spawn_background(work, move |result| {
            refresh();
            batch(|| {
                controller.busy.set(false);
                match result {
                    Ok(()) => {
                        controller.panel.set(None);
                        controller.error.set(None);
                    }
                    Err(error) => {
                        eprintln!("file-menu: {error}");
                        if matches!(controller.panel.peek(), Some(Panel::Form(_))) {
                            controller.error.set(Some(error));
                        } else {
                            controller.information("Could not complete action", error);
                        }
                    }
                }
            });
        });
    }
    fn submit(&self) {
        let Some(Panel::Form(form)) = self.panel.peek() else {
            return;
        };
        let name = self.name.peek();
        if !operations::valid_name(&name) {
            self.error
                .set(Some("Enter a file name without /, NUL, . or ..".into()));
            return;
        }
        let refresh = form.refresh;
        self.perform(
            &form.window,
            move || refresh(),
            move || {
                let result = match form.kind {
                    FormKind::NewFile => operations::create(&form.path, &name, false),
                    FormKind::NewFolder => operations::create(&form.path, &name, true),
                    FormKind::Rename => operations::rename(&form.path, &name),
                };
                result.map_err(|error| error.to_string())
            },
        );
    }
    fn information(&self, title: impl Into<String>, body: impl Into<String>) {
        self.panel.set(Some(Panel::Information {
            title: title.into(),
            body: body.into(),
        }));
    }
    pub fn handle_key(&self, key: &creamui_render::platform::Key, pressed: bool) -> bool {
        if pressed && self.pending.get() && *key == creamui_render::platform::Key::Escape {
            self.close_menu();
            return true;
        }
        if !pressed {
            return self.menu.is_open() || self.panel.peek().is_some();
        }
        if self.panel.peek().is_some() {
            if *key == creamui_render::platform::Key::Escape {
                self.dismiss();
                return true;
            }
            return false;
        }
        if self.menu.is_open() {
            use creamui_render::platform::Key as NativeKey;
            let key = match key {
                NativeKey::Escape => Key::Escape,
                NativeKey::Enter => Key::Enter,
                NativeKey::Tab => Key::Tab,
                NativeKey::Up => Key::Up,
                NativeKey::Down => Key::Down,
                NativeKey::Left => Key::Left,
                NativeKey::Right => Key::Right,
                NativeKey::Home => Key::Home,
                NativeKey::End => Key::End,
                NativeKey::Space => Key::Char(' '),
                _ => return true,
            };
            return self.menu.handle_key(&key);
        }
        false
    }
    pub fn has_panel(&self) -> bool {
        self.panel.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_details_update_actions_without_reopening_dismissed_menus() {
        let controller = FileMenuController::new();
        let menu = controller.menu.controller();
        let pasted = Rc::new(Cell::new(false));
        let action = pasted.clone();
        let update = ContextMenuItem::new("paste", "Paste", move || action.set(true));
        menu.open(
            Point::default(),
            vec![ContextMenuItem::new("paste", "Paste", || panic!()).enabled(false)],
        );
        controller.update_details(controller.generation.get(), vec![update.clone()]);
        for key in [Key::Down, Key::Enter] {
            menu.handle_key(&key);
        }
        assert!(pasted.get());
        assert!(!menu.is_open());
        controller.update_details(controller.generation.get(), vec![update]);
        assert!(!menu.is_open());
    }

    #[test]
    fn native_menu_routing_and_pending_cancellation_do_not_require_ui_context() {
        use creamui_render::platform::Key as NativeKey;
        let controller = FileMenuController::new();
        let activated = Rc::new(Cell::new(false));
        let action = activated.clone();
        controller.menu.controller().open(
            Point::default(),
            vec![ContextMenuItem::new("open", "Open", move || {
                action.set(true)
            })],
        );
        assert!(controller.handle_key(&NativeKey::Down, true));
        assert!(controller.handle_key(&NativeKey::Enter, true));
        assert!(activated.get());
        controller.pending.set(true);
        let generation = controller.generation.get();
        assert!(controller.handle_key(&NativeKey::Escape, true));
        assert!(!controller.pending.get());
        assert_ne!(generation, controller.generation.get());
    }
}
