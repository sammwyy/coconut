mod loading;
mod menus;
use crate::{
    config::ExplorerConfig,
    context::WindowContext,
    filesystem as files,
    icons::Icons,
    navigation::{DirectoryView, Location, Tab},
    portal::{PortalSelection, PortalType},
    window::{WindowMode, WindowRequest},
};
use creamui_image::ImageData;
use creamui_reactive::{batch, Signal};
use creamui_render::{
    platform::{KeyEvent, Modifiers},
    WindowHandle,
};
use creamui_widgets::{ScrollController, TextController};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct BrowserState {
    pub(crate) file_menu: coconut_file_menu::FileMenuController,
    pub(crate) context: WindowContext,
    pub(crate) tabs: Signal<Vec<Tab>>,
    pub(crate) active: Signal<usize>,
    pub(crate) pins: Signal<Vec<PathBuf>>,
    pub(crate) recent: Signal<Vec<PathBuf>>,
    pub(crate) places: Rc<Vec<files::Place>>,
    pub(crate) drives: Signal<Vec<files::Drive>>,
    pub(crate) icons: Icons,
    application_icons: Rc<RefCell<HashMap<String, Option<ImageData>>>>,
    previews: Rc<RefCell<HashMap<PathBuf, Option<ImageData>>>>,
    preview_queue: Rc<RefCell<VecDeque<PathBuf>>>,
    preview_pending: Rc<RefCell<HashSet<PathBuf>>>,
    preview_running: Rc<Cell<bool>>,
    preview_revision: Signal<u64>,
    pub(crate) name: TextController,
    pub(crate) message: Signal<Option<String>>,
    pub(crate) overwrite: Signal<Option<PathBuf>>,
    pub(crate) last_click: Rc<RefCell<Option<(PathBuf, Instant)>>>,
    pub(crate) sidebar_scroll: ScrollController,
    pub(crate) window: Rc<RefCell<Option<WindowHandle>>>,
    pub(crate) result: Arc<Mutex<Option<PortalSelection>>>,
    pub(crate) explorer: ExplorerConfig,
    directory_job_running: Rc<Cell<bool>>,
    pub(crate) search_active: Signal<bool>,
    pub(crate) address_editing: Signal<bool>,
}
impl BrowserState {
    #[cfg(test)]
    pub(crate) fn new(request: WindowRequest) -> Self {
        let state = Self::new_deferred(request);
        state.load_directories();
        state
    }
    pub(crate) fn new_deferred(request: WindowRequest) -> Self {
        let explorer = crate::config::Config::load().explorer;
        let location = request
            .start_directory
            .as_ref()
            .map(|p| Location::Directory(start_directory(p)))
            .unwrap_or_else(|| {
                if request.mode == WindowMode::FileManager {
                    Location::Home
                } else {
                    Location::Directory(files::home())
                }
            });
        let places = files::places();
        let pins = files::read_paths(&files::state_file("pinned")).unwrap_or_else(|| {
            places
                .iter()
                .filter(|p| ["Desktop", "Documents", "Downloads"].contains(&p.label.as_str()))
                .map(|p| p.path.clone())
                .collect()
        });
        let name = match &request.mode {
            WindowMode::Portal(portal) => portal.suggested_name.clone(),
            WindowMode::FileManager => String::new(),
        };
        Self {
            file_menu: coconut_file_menu::FileMenuController::new(),
            context: WindowContext::new(request),
            tabs: Signal::new(vec![Tab::new(location, &explorer)]),
            active: Signal::new(0),
            pins: Signal::new(pins),
            recent: Signal::new(
                files::read_paths(&files::state_file("recent"))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|p| p.is_file())
                    .take(12)
                    .collect(),
            ),
            places: Rc::new(places),
            drives: Signal::new(files::drives()),
            icons: Icons::load(),
            application_icons: Rc::new(RefCell::new(HashMap::new())),
            previews: Rc::new(RefCell::new(HashMap::new())),
            preview_queue: Rc::new(RefCell::new(VecDeque::new())),
            preview_pending: Rc::new(RefCell::new(HashSet::new())),
            preview_running: Rc::new(Cell::new(false)),
            preview_revision: Signal::new(0),
            name: TextController::new(name),
            message: Signal::new(None),
            overwrite: Signal::new(None),
            last_click: Rc::new(RefCell::new(None)),
            sidebar_scroll: ScrollController::new(0.0),
            window: Rc::new(RefCell::new(None)),
            result: Arc::new(Mutex::new(None)),
            explorer,
            directory_job_running: Rc::new(Cell::new(false)),
            search_active: Signal::new(false),
            address_editing: Signal::new(false),
        }
    }
    pub(crate) fn preview(&self, path: &Path) -> Option<ImageData> {
        self.preview_revision.get();
        if !coconut_thumbnails::Thumbnailer::supports(path) {
            return None;
        }
        if let Some(cached) = self.previews.borrow().get(path) {
            return cached.clone();
        }
        if self.window.borrow().is_some()
            && self.preview_pending.borrow_mut().insert(path.to_path_buf())
        {
            let mut queue = self.preview_queue.borrow_mut();
            if queue.len() >= 64 {
                if let Some(obsolete) = queue.pop_front() {
                    self.preview_pending.borrow_mut().remove(&obsolete);
                }
            }
            queue.push_back(path.to_path_buf());
            drop(queue);
            self.load_next_preview();
        }
        None
    }

    pub(crate) fn application_icon(&self, path: &Path) -> Option<ImageData> {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        self.application_icons
            .borrow()
            .get(mime.essence_str())
            .cloned()
            .flatten()
    }

    fn load_next_preview(&self) {
        if self.preview_running.get() {
            return;
        }
        let Some(path) = self.preview_queue.borrow_mut().pop_front() else {
            return;
        };
        let Some(window) = self.window.borrow().clone() else {
            return;
        };
        self.preview_running.set(true);
        let state = self.clone();
        let requested = path.clone();
        window.app().spawn_background(
            move || {
                coconut_thumbnails::Thumbnailer::new()
                    .and_then(|generator| generator.get(&path, 160))
            },
            move |result| {
                let image = match result {
                    Ok(Some(bytes)) => ImageData::from_bytes(&bytes).ok(),
                    Ok(None) => None,
                    Err(error) => {
                        eprintln!("cfiles: thumbnail {}: {error}", requested.display());
                        None
                    }
                };
                let mut previews = state.previews.borrow_mut();
                if previews.len() >= 256 {
                    previews.clear();
                }
                previews.insert(requested.clone(), image);
                drop(previews);
                state.preview_pending.borrow_mut().remove(&requested);
                state.preview_running.set(false);
                state
                    .preview_revision
                    .update(|revision| *revision = revision.wrapping_add(1));
                state.load_next_preview();
            },
        );
    }
    pub(crate) fn toggle_search(&self) {
        let active = !self.search_active.peek();
        self.search_active.set(active);
        if active {
            self.address_editing.set(false);
            if let Some(window) = self.window.borrow().as_ref() {
                window.focus_text_input();
            }
        } else {
            self.edit(|tab| tab.search.set_value(""));
        }
    }
    pub(crate) fn cancel_search(&self) {
        self.search_active.set(false);
        self.edit(|tab| tab.search.set_value(""));
    }
    pub(crate) fn close_search_if_empty(&self) {
        if self.search_active.peek() && self.tab().search.peek().trim().is_empty() {
            self.cancel_search();
        }
    }
    pub(crate) fn begin_address_edit(&self) {
        self.address_editing.set(true);
        if let Some(window) = self.window.borrow().as_ref() {
            window.focus_text_input();
        }
    }
    pub(crate) fn cancel_address_edit(&self) {
        self.edit(|tab| tab.address.set_value(tab.location().address()));
        self.address_editing.set(false);
    }
    pub(crate) fn open_settings(&self) {
        match std::env::current_exe().and_then(|exe| Command::new(exe).arg("--settings").spawn()) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(error) => self
                .message
                .set(Some(format!("Couldn't open settings: {error}"))),
        }
    }
    pub(crate) fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if self.file_menu.handle_key(&event.key, event.pressed) {
            return true;
        }
        if self.file_menu.has_panel() || !event.pressed {
            return false;
        }
        let action = crate::shortcuts::Action::Search;
        let binding = crate::config::Config::load()
            .shortcuts
            .get(action.id())
            .cloned()
            .unwrap_or_else(|| action.default_binding().into());
        if crate::shortcuts::Binding::parse(&binding)
            .is_ok_and(|binding| binding.matches(&event.key, modifiers))
        {
            self.toggle_search();
            return true;
        }
        false
    }
    pub(crate) fn tab(&self) -> Tab {
        self.tabs.get()[self.active.get()].clone()
    }
    pub(crate) fn edit(&self, edit: impl FnOnce(&mut Tab)) {
        let index = self.active.peek();
        // Tab edits can write to nested text/scroll signals. Defer their
        // subscribers until Signal::update releases the tab list's borrow.
        batch(|| {
            self.tabs.update(|tabs| {
                let tab = &mut tabs[index];
                let previous_view = tab.view;
                edit(tab);
                if tab.view == DirectoryView::Columns && previous_view != tab.view {
                    tab.reload();
                }
            });
            self.load_directories();
        });
    }
    pub(crate) fn navigate(&self, location: Location) {
        self.file_menu.close_menu();
        batch(|| {
            self.message.set(None);
            self.overwrite.set(None);
            *self.last_click.borrow_mut() = None;
            self.edit(|t| t.navigate(location));
        });
    }
    pub(crate) fn navigate_column(&self, path: PathBuf) {
        self.file_menu.close_menu();
        batch(|| {
            self.message.set(None);
            self.overwrite.set(None);
            *self.last_click.borrow_mut() = None;
            self.edit(|tab| tab.navigate_column(path));
        });
    }
    pub(crate) fn travel(&self, forward: bool) {
        self.file_menu.close_menu();
        batch(|| {
            self.message.set(None);
            self.overwrite.set(None);
            *self.last_click.borrow_mut() = None;
            self.edit(|t| t.travel(forward));
        });
    }
    pub(crate) fn switch(&self, index: usize) {
        self.file_menu.close_menu();
        batch(|| {
            self.active.set(index);
            self.message.set(None);
            self.overwrite.set(None);
            *self.last_click.borrow_mut() = None;
        });
    }
    pub(crate) fn add_tab(&self) {
        batch(|| {
            self.tabs
                .update(|tabs| tabs.push(Tab::new(Location::Home, &self.explorer)));
            self.switch(self.tabs.peek().len() - 1);
        });
    }
    pub(crate) fn close_tab(&self, index: usize) {
        let mut tabs = self.tabs.peek();
        if tabs.len() == 1 {
            return;
        }
        tabs.remove(index);
        let active = self.active.peek();
        batch(|| {
            self.switch(if index < active {
                active - 1
            } else {
                active.min(tabs.len() - 1)
            });
            self.tabs.set(tabs);
        });
    }
    pub(crate) fn submit_address(&self) {
        let tab = self.tab();
        let value = tab.address.value();
        let value = value.trim();
        if value == "Home" {
            self.navigate(Location::Home);
            self.address_editing.set(false);
            return;
        }
        let base = match tab.location() {
            Location::Home => files::home(),
            Location::Directory(p) => p.clone(),
        };
        let path = if value == "~" {
            files::home()
        } else if let Some(rest) = value.strip_prefix("~/") {
            files::home().join(rest)
        } else if value.starts_with("file://") {
            match url::Url::parse(value)
                .ok()
                .and_then(|u| u.to_file_path().ok())
            {
                Some(p) => p,
                None => {
                    self.message.set(Some("Enter a local folder path.".into()));
                    return;
                }
            }
        } else {
            base.join(value)
        };
        match fs::canonicalize(path) {
            Ok(path) if path.is_dir() => {
                self.navigate(Location::Directory(path));
                self.address_editing.set(false);
            }
            _ => self.message.set(Some(
                "Folder not found. Check the address and try again.".into(),
            )),
        }
    }
    pub(crate) fn toggle_pin(&self, path: PathBuf) {
        let mut pins = self.pins.peek();
        if let Some(index) = pins.iter().position(|p| p == &path) {
            pins.remove(index);
        } else {
            pins.push(path);
        }
        match files::write_paths(&files::state_file("pinned"), &pins) {
            Ok(()) => {
                self.pins.set(pins);
                self.message.set(None);
            }
            Err(error) => self
                .message
                .set(Some(format!("Could not save pinned folders: {error}"))),
        }
    }
    pub(crate) fn select(&self, entry: files::Entry) {
        let double = self
            .last_click
            .borrow()
            .as_ref()
            .is_some_and(|(path, time)| {
                path == &entry.path && time.elapsed() < Duration::from_millis(450)
            });
        *self.last_click.borrow_mut() = Some((entry.path.clone(), Instant::now()));
        self.edit(|t| t.selected = Some(entry.path.clone()));
        self.message.set(None);
        self.overwrite.set(None);
        if !entry.directory && self.context.is_save() {
            self.name.set_value(entry.name());
        }
        if double {
            self.activate(entry.path, entry.directory);
        }
    }
    pub(crate) fn activate(&self, path: PathBuf, directory: bool) {
        if directory {
            if self.tab().view == DirectoryView::Columns {
                self.navigate_column(path);
            } else {
                self.navigate(Location::Directory(path));
            }
        } else if !path.is_file() {
            self.message
                .set(Some("This file is no longer available.".into()));
        } else if self.context.is_manager() {
            match open_with_default_application(&path) {
                Ok(_) => self.remember(path),
                Err(error) => self
                    .message
                    .set(Some(format!("Could not open file: {error}"))),
            }
        } else if self.context.portal_type() == Some(PortalType::OpenFile)
            && self.context.accepts(&path, false)
        {
            self.finish(path);
        }
    }
    pub(crate) fn remember(&self, path: PathBuf) {
        let mut recent = self.recent.peek();
        recent.retain(|p| p != &path);
        recent.insert(0, path);
        recent.truncate(12);
        if let Err(error) = files::write_paths(&files::state_file("recent"), &recent) {
            eprintln!("cfiles: could not save recent files: {error}");
        }
        self.recent.set(recent);
    }
    pub(crate) fn finish(&self, path: PathBuf) {
        if path.is_file() {
            self.remember(path.clone());
        }
        *self.result.lock().expect("browser result") = Some(PortalSelection {
            path,
            current_filter: self.context.portal().and_then(|p| p.current_filter()),
        });
        self.close();
    }
    pub(crate) fn close(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.close();
        }
    }
    pub(crate) fn accept(&self) {
        let mut tab = self.tab();
        tab.selected = self.visible_selection(&tab);
        match self.context.portal_type() {
            None => {
                if let Some(path) = tab.selected {
                    let directory = path.is_dir();
                    self.activate(path, directory);
                }
            }
            Some(kind @ (PortalType::OpenFile | PortalType::SelectFolder)) => {
                let directories_only = kind == PortalType::SelectFolder;
                let path = if directories_only {
                    tab.selected.clone().or_else(|| match tab.location() {
                        Location::Directory(p) => Some(p.clone()),
                        _ => None,
                    })
                } else {
                    tab.selected.clone()
                };
                if let Some(path) = path {
                    if (directories_only && path.is_dir()) || (!directories_only && path.is_file())
                    {
                        self.finish(path);
                    } else if path.is_dir() {
                        self.navigate(Location::Directory(path));
                    } else {
                        self.message
                            .set(Some("This item is no longer available.".into()));
                    }
                }
            }
            Some(PortalType::SaveFile) => {
                let Location::Directory(directory) = tab.location() else {
                    return;
                };
                let name = self.name.value();
                if !valid_filename(&name) {
                    self.message.set(Some(
                        "Enter a file name; /, NUL, . and .. are not allowed.".into(),
                    ));
                    return;
                }
                let path = directory.join(name);
                if path.is_dir() {
                    self.message
                        .set(Some("A folder already has that name.".into()));
                    return;
                }
                if path.symlink_metadata().is_ok() && self.overwrite.peek().as_ref() != Some(&path)
                {
                    self.overwrite.set(Some(path));
                    self.message.set(Some(
                        "This file already exists. Choose Replace to confirm.".into(),
                    ));
                    return;
                }
                if !directory.is_dir() {
                    self.message.set(Some(
                        "The destination folder is no longer available.".into(),
                    ));
                    return;
                }
                self.finish(path);
            }
        }
    }

    pub(crate) fn visible_selection(&self, tab: &Tab) -> Option<PathBuf> {
        tab.selected
            .as_ref()
            .filter(|path| {
                files::name(path)
                    .to_lowercase()
                    .contains(&tab.search.value().to_lowercase())
                    && self.context.accepts(path, path.is_dir())
            })
            .cloned()
    }
}

/// Opens a file through the desktop's MIME Apps association, rather than
/// delegating policy to an opaque `xdg-open` subprocess.
fn open_with_default_application(path: &Path) -> Result<(), String> {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mime = mime.essence_str();
    let (entry, command) = delib::mimeapps::MimeApps::default()
        .command_for(mime, &[path.to_path_buf()])
        .ok_or_else(|| format!("No default application is associated with {mime}."))?;
    let (program, arguments) = command
        .split_first()
        .ok_or_else(|| "The selected application has an empty launch command.".to_owned())?;
    Command::new(program)
        .args(arguments)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            format!(
                "Could not start {}: {error}",
                entry.name(None).unwrap_or(program)
            )
        })
}
pub(crate) fn valid_filename(name: &str) -> bool {
    !name.trim().is_empty() && ![".", ".."].contains(&name) && !name.contains(['/', '\0'])
}
fn start_directory(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .ok()
        .and_then(|p| {
            if p.is_dir() {
                Some(p)
            } else {
                p.parent().map(Path::to_path_buf)
            }
        })
        .unwrap_or_else(files::home)
}

#[cfg(test)]
mod tests;
