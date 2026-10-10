use coconut_api::settings::{Action, SettingsIntegration, Snapshot, Value};
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_widgets::{SelectController, TextController};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
pub struct State {
    pub snapshot: Signal<Snapshot>,
    pub status: Signal<String>,
    pub(crate) busy: Signal<bool>,
    preview_generation: Rc<Cell<u64>>,
    app: Rc<RefCell<Option<AppHandle>>>,
    backend: Arc<dyn SettingsIntegration>,
    selects: Rc<RefCell<BTreeMap<String, SelectController>>>,
    pending: Rc<RefCell<BTreeMap<String, Value>>>,
    accounts: Rc<RefCell<Option<Signal<Vec<crate::services::accounts::Account>>>>>,
    pub search: TextController,
    pub hostname: TextController,
    pub timezone: TextController,
    pub printer_name: TextController,
    pub printer_uri: TextController,
    pub username: TextController,
    pub real_name: TextController,
    pub administrator: Signal<bool>,
    pub password: TextController,
    pub confirmation: TextController,
    pub rules: TextController,
}
impl State {
    pub fn new(backend: Arc<dyn SettingsIntegration>) -> Self {
        Self {
            snapshot: Signal::new(Snapshot::default()),
            status: Signal::new(String::new()),
            busy: Signal::new(false),
            preview_generation: Rc::new(Cell::new(0)),
            app: Rc::new(RefCell::new(None)),
            backend,
            selects: Rc::new(RefCell::new(BTreeMap::new())),
            pending: Rc::new(RefCell::new(BTreeMap::new())),
            accounts: Rc::new(RefCell::new(None)),
            search: TextController::new(""),
            hostname: TextController::new(""),
            timezone: TextController::new(""),
            printer_name: TextController::new(""),
            printer_uri: TextController::new(""),
            username: TextController::new(""),
            real_name: TextController::new(""),
            administrator: Signal::new(false),
            password: TextController::new(""),
            confirmation: TextController::new(""),
            rules: TextController::new(""),
        }
    }
    pub fn start(&self, app: AppHandle, accounts: Signal<Vec<crate::services::accounts::Account>>) {
        *self.app.borrow_mut() = Some(app);
        *self.accounts.borrow_mut() = Some(accounts);
        self.refresh();
    }
    fn accept(&self, snapshot: Snapshot) {
        if self.hostname.value().is_empty() {
            self.hostname.set_value(snapshot.fact("hostname"));
        }
        if self.timezone.value().is_empty() {
            if let Some(Value::Text(zone)) = snapshot
                .preferences
                .get("timezone")
                .and_then(|p| p.value.as_ref())
            {
                self.timezone.set_value(zone);
            }
        }
        self.snapshot.set(snapshot);
        self.watch_display_preview();
    }
    fn watch_display_preview(&self) {
        let generation = self.preview_generation.get().wrapping_add(1);
        self.preview_generation.set(generation);
        if !self
            .snapshot
            .peek()
            .entries("displays")
            .iter()
            .any(|display| {
                display
                    .properties
                    .get("pending-confirmation")
                    .is_some_and(|v| v == "true")
            })
        {
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let backend = self.backend.clone();
        let state = self.clone();
        app.spawn_background(
            move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                backend.display_snapshot()
            },
            move |displays| {
                if state.preview_generation.get() != generation {
                    return;
                }
                if displays.loaded {
                    state.accept_displays(displays);
                }
            },
        );
    }
    fn accept_displays(&self, displays: Snapshot) {
        let mut snapshot = self.snapshot.peek();
        snapshot
            .collections
            .insert("displays".into(), displays.entries("displays").to_vec());
        snapshot
            .preferences
            .retain(|key, _| !key.starts_with("display:"));
        snapshot.preferences.extend(displays.preferences);
        self.accept(snapshot);
    }
    pub fn refresh(&self) {
        if self.busy.peek() {
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.busy.set(true);
        let backend = self.backend.clone();
        let state = self.clone();
        app.spawn_background(
            move || {
                (
                    backend.snapshot(),
                    crate::services::accounts::list_accounts(),
                )
            },
            move |(snapshot, accounts)| {
                state.accept(snapshot);
                if let Some(target) = state.accounts.borrow().as_ref() {
                    target.set(accounts);
                }
                state.busy.set(false);
                state.flush_pending();
            },
        );
    }
    pub(crate) fn apply(&self, action: Action) {
        if self.busy.peek() {
            if let Action::Set { key, value } = action {
                self.pending.borrow_mut().insert(key, value);
            }
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.preview_generation
            .set(self.preview_generation.get().wrapping_add(1));
        self.busy.set(true);
        self.status.set("Applying changes…".into());
        let backend = self.backend.clone();
        let state = self.clone();
        let displays_only = matches!(
            &action,
            Action::ConfirmDisplayMode { .. } | Action::RevertDisplayMode { .. }
        ) || matches!(&action, Action::Set { key, .. } if key.starts_with("display:"));
        app.spawn_background(
            move || {
                let result = backend.apply(action);
                let snapshot = if displays_only {
                    backend.display_snapshot()
                } else {
                    backend.snapshot()
                };
                (
                    result,
                    snapshot,
                    (!displays_only).then(crate::services::accounts::list_accounts),
                )
            },
            move |(result, snapshot, accounts)| {
                if displays_only {
                    state.accept_displays(snapshot);
                } else {
                    state.accept(snapshot);
                }
                if let (Some(target), Some(accounts)) = (state.accounts.borrow().as_ref(), accounts)
                {
                    target.set(accounts);
                }
                state.busy.set(false);
                state
                    .status
                    .set(result.unwrap_or_else(|error| format!("Could not apply: {error}")));
                state.flush_pending();
            },
        );
    }
    fn flush_pending(&self) {
        let next = self.pending.borrow_mut().pop_first();
        if let Some((key, value)) = next {
            self.apply(Action::Set { key, value });
        }
    }
    pub(crate) fn select(&self, key: &str, index: usize) -> SelectController {
        let mut controllers = self.selects.borrow_mut();
        let controller = controllers
            .entry(key.into())
            .or_insert_with(|| SelectController::new(index));
        // Keep popup/search state across renders. Backend remains authoritative.
        if !controller.peek_open() && controller.peek_selected() != index {
            controller.select(index);
        }
        controller.clone()
    }
}
