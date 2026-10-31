use crate::view::{self, Actions, Snapshot};
use coconut_integration_clipboard::client::Client;
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_widgets::{ScrollController, TextController};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const PAGE: u32 = 20;

pub(super) struct ClipboardPanel {
    pub app: AppHandle,
}

struct State {
    app: AppHandle,
    snapshot: Signal<Snapshot>,
    busy: Signal<bool>,
    page: Cell<u32>,
    request: Cell<u64>,
    scroll: ScrollController,
    presentation: view::Presentation,
    query: RefCell<String>,
    reload_pending: Cell<bool>,
}

fn snapshot(client: &Client, page: u32, query: &str) -> Result<Snapshot, String> {
    let (available, enabled, message, revision) =
        client.status().map_err(|error| error.to_string())?;
    let (_, current) = client.current().map_err(|error| error.to_string())?;
    let mut entries = client
        .search(query, page * PAGE, PAGE + 1)
        .map_err(|error| error.to_string())?;
    let has_next = entries.len() > PAGE as usize;
    entries.truncate(PAGE as usize);
    Ok(Snapshot {
        loading: false,
        available,
        enabled,
        revision,
        current: view::preview(&current, 160),
        entries,
        has_next,
        message,
    })
}

impl State {
    fn reload(self: &Rc<Self>) {
        self.load(|_| Ok(()), None);
    }

    fn load(
        self: &Rc<Self>,
        action: impl FnOnce(&Client) -> Result<(), String> + Send + 'static,
        close: Option<Rc<dyn Fn()>>,
    ) {
        if self.busy.peek() {
            self.reload_pending.set(true);
            return;
        }
        self.reload_pending.set(false);
        self.busy.set(true);
        let page = self.page.get();
        let query = self.query.borrow().clone();
        let requested_query = query.clone();
        let request = self.request.get() + 1;
        self.request.set(request);
        let weak = Rc::downgrade(self);
        self.app.spawn_background(
            move || {
                let client = Client::session().map_err(|error| error.to_string())?;
                action(&client)?;
                snapshot(&client, page, &query)
            },
            move |result: Result<Snapshot, String>| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state.request.get() != request {
                    return;
                }
                creamui_reactive::batch(|| {
                    match result {
                        Ok(snapshot) => {
                            if state.page.get() == page && *state.query.borrow() == requested_query
                            {
                                if snapshot.entries.is_empty() && page > 0 {
                                    state.page.set(page - 1);
                                    state.reload_pending.set(true);
                                } else {
                                    state.snapshot.set(snapshot);
                                }
                            } else {
                                state.reload_pending.set(true);
                            }
                            if let Some(close) = close {
                                close();
                            }
                        }
                        Err(error) => {
                            eprintln!("clipboard popup: {error}");
                            let mut snapshot = state.snapshot.peek();
                            snapshot.loading = false;
                            snapshot.available = false;
                            snapshot.message = error;
                            state.snapshot.set(snapshot);
                        }
                    }
                    state.busy.set(false);
                });
                if state.reload_pending.replace(false) {
                    state.reload();
                }
            },
        );
    }

    fn watch(self: &Rc<Self>, revision: u64) {
        let weak = Rc::downgrade(self);
        self.app.spawn_background(
            move || {
                let result = Client::session().and_then(|client| client.wait_changed(revision));
                if result.is_err() {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                result
            },
            move |result| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let next = result.as_ref().copied().unwrap_or(revision);
                if result.is_err() || next != state.snapshot.peek().revision {
                    state.reload();
                }
                state.watch(next);
            },
        );
    }
}

impl Panel for ClipboardPanel {
    fn id(&self) -> &'static str {
        "clipboard"
    }
    fn title(&self) -> &'static str {
        "Clipboard history"
    }
    fn size(&self) -> Size {
        Size {
            width: 400.0,
            height: 540.0,
        }
    }
    fn autofocus(&self) -> bool {
        true
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        let state = ctx.scratch.get_or_insert_with(|| {
            let state = Rc::new(State {
                app: self.app.clone(),
                snapshot: Signal::new(Snapshot::default()),
                busy: Signal::new(false),
                page: Cell::new(0),
                request: Cell::new(0),
                scroll: ScrollController::default(),
                presentation: view::Presentation {
                    query: TextController::default(),
                    confirm_clear: Signal::new(false),
                },
                query: RefCell::new(String::new()),
                reload_pending: Cell::new(false),
            });
            let weak = Rc::downgrade(&state);
            state.presentation.query.on_change(move |_, next| {
                if let Some(state) = weak.upgrade() {
                    *state.query.borrow_mut() = next.to_owned();
                    state.page.set(0);
                    state.scroll.set(0.0);
                    state.reload();
                }
                Some(next.to_owned())
            });
            state.reload();
            state.watch(0);
            state
        });
        let snapshot = state.snapshot.get();
        let enabled = snapshot.enabled;
        let pause = state.clone();
        let clear = state.clone();
        let restore = state.clone();
        let remove = state.clone();
        let navigate = state.clone();
        let refresh = state.clone();
        let close = ctx.close.clone();
        let actions = Actions {
            refresh: Rc::new(move || refresh.reload()),
            pause: Rc::new(move || {
                pause.load(
                    move |client| {
                        client
                            .set_enabled(!enabled)
                            .map_err(|error| error.to_string())
                    },
                    None,
                )
            }),
            clear: Rc::new(move || {
                clear.page.set(0);
                clear.load(
                    |client| client.clear().map_err(|error| error.to_string()),
                    None,
                )
            }),
            restore: Rc::new(move |id| {
                restore.load(
                    move |client| client.restore(id).map_err(|error| error.to_string()),
                    Some(close.clone()),
                )
            }),
            remove: Rc::new(move |id| {
                remove.load(
                    move |client| {
                        client
                            .remove(id)
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    },
                    None,
                )
            }),
            navigate: Rc::new(move |delta| {
                if navigate.busy.peek() {
                    return;
                }
                navigate
                    .page
                    .set(navigate.page.get().saturating_add_signed(delta));
                navigate.scroll.set(0.0);
                navigate.load(|_| Ok(()), None);
            }),
        };
        view::build(
            ctx.size,
            &snapshot,
            state.busy.get(),
            state.page.get(),
            state.scroll.clone(),
            &actions,
            &state.presentation,
        )
    }
}
