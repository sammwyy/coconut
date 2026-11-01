use crate::{applications::app_from_desktop_entry, AppEntry, PluginInitContext};
use coconut_core::{PinnedApplication, PinnedAppsStore};
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
};

#[derive(Clone)]
pub struct PinnedApp {
    pub pin: PinnedApplication,
    pub application: Option<AppEntry>,
}

#[derive(Clone)]
pub struct Pins(Rc<State>);

struct State {
    app: AppHandle,
    store: PinnedAppsStore,
    entries: Signal<Vec<PinnedApp>>,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
    loading: Cell<bool>,
    loaded: Signal<bool>,
    queue: RefCell<VecDeque<Edit>>,
    theme: RefCell<String>,
    refresh_pending: Cell<bool>,
}

enum Edit {
    Pin(Vec<PathBuf>),
    Unpin(PathBuf),
}

impl Pins {
    pub fn ensure(ctx: &PluginInitContext) -> Self {
        ctx.shared.get_or_insert_with(|| {
            let state = Self(Rc::new(State {
                app: ctx.app.clone(),
                store: PinnedAppsStore::default(),
                entries: Signal::new(Vec::new()),
                error: Signal::new(None),
                busy: Signal::new(false),
                loading: Cell::new(false),
                loaded: Signal::new(false),
                queue: RefCell::new(VecDeque::new()),
                theme: RefCell::new(coconut_core::ShellConfig::load().appearance.icon_theme),
                refresh_pending: Cell::new(false),
            }));
            state.refresh();
            state
        })
    }

    pub fn entries(&self) -> Vec<PinnedApp> {
        self.0.entries.get()
    }
    pub fn error(&self) -> Option<String> {
        self.0.error.get()
    }
    pub fn busy(&self) -> bool {
        self.0.busy.get()
    }
    pub fn loaded(&self) -> bool {
        self.0.loaded.get()
    }
    pub fn dismiss_error(&self) {
        self.0.error.set(None);
    }

    pub fn contains(&self, path: &std::path::Path) -> bool {
        self.entries()
            .iter()
            .any(|entry| entry.pin.source == path || entry.pin.desktop_file == path)
    }

    pub fn pin(&self, paths: Vec<PathBuf>) {
        self.0.queue.borrow_mut().push_back(Edit::Pin(paths));
        self.run_next();
    }

    pub fn unpin(&self, source: PathBuf) {
        self.0.queue.borrow_mut().push_back(Edit::Unpin(source));
        self.run_next();
    }

    pub fn set_icon_theme(&self, theme: String) {
        *self.0.theme.borrow_mut() = theme;
        self.refresh();
    }

    pub fn refresh(&self) {
        if self.0.loading.get() {
            self.0.refresh_pending.set(true);
            return;
        }
        self.load(None);
    }

    fn run_next(&self) {
        if self.0.loading.get() {
            return;
        }
        let edit = self.0.queue.borrow_mut().pop_front();
        if let Some(edit) = edit {
            self.load(Some(edit));
        }
    }

    fn load(&self, edit: Option<Edit>) {
        self.0.loading.set(true);
        let editing = edit.is_some();
        if editing {
            self.0.busy.set(true);
        }
        let store = self.0.store.clone();
        let theme = self.0.theme.borrow().clone();
        let state = self.clone();
        self.0.app.spawn_background(
            move || {
                let saved = match edit {
                    Some(Edit::Pin(paths)) => store.pin(&paths),
                    Some(Edit::Unpin(source)) => store.unpin(&source),
                    None => store.load(),
                }?;
                let icons = crate::IconResolver::new(theme);
                let entries = saved
                    .applications
                    .into_iter()
                    .map(|pin| {
                        let application =
                            match crate::DesktopEntry::parse_file(&pin.desktop_file) {
                                Ok(entry) => app_from_desktop_entry(entry),
                                Err(error) => {
                                    eprintln!(
                                        "pinned-apps: cannot read {}: {error}",
                                        pin.desktop_file.display()
                                    );
                                    None
                                }
                            }
                            .map(|mut app| {
                                app.icon = app.icon_name.as_deref().and_then(|name| {
                                    icons.load(crate::IconRequest::named(name, 48))
                                });
                                app
                            });
                        PinnedApp { pin, application }
                    })
                    .collect::<Vec<_>>();
                Ok::<_, std::io::Error>(entries)
            },
            move |result| {
                state.0.loaded.set(true);
                match result {
                    Ok(entries) => {
                        state.0.entries.set(entries);
                        state.0.error.set(None);
                    }
                    Err(error) => {
                        eprintln!("pinned-apps: {error}");
                        state.0.error.set(Some(error.to_string()));
                    }
                }
                state.0.loading.set(false);
                if editing {
                    state.0.busy.set(false);
                }
                if state.0.refresh_pending.replace(false) && state.0.queue.borrow().is_empty() {
                    state.refresh();
                } else {
                    state.run_next();
                }
            },
        );
    }
}
