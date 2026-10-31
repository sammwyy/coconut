use crate::view::{self, Actions, Snapshot};
use coconut_integration_clipboard::client::Client;
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_widgets::ScrollController;
use std::{cell::Cell, rc::Rc};

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
}

fn snapshot(client: &Client, page: u32) -> Result<Snapshot, String> {
    let (available, enabled, message, revision) =
        client.status().map_err(|error| error.to_string())?;
    let (_, current) = client.current().map_err(|error| error.to_string())?;
    let mut entries = client
        .list(page * PAGE, PAGE + 1)
        .map_err(|error| error.to_string())?;
    let has_next = entries.len() > PAGE as usize;
    entries.truncate(PAGE as usize);
    Ok(Snapshot {
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
    fn load(
        self: &Rc<Self>,
        action: impl FnOnce(&Client) -> Result<(), String> + Send + 'static,
        close: Option<Rc<dyn Fn()>>,
    ) {
        if self.busy.peek() {
            return;
        }
        self.busy.set(true);
        let page = self.page.get();
        let request = self.request.get() + 1;
        self.request.set(request);
        let weak = Rc::downgrade(self);
        self.app.spawn_background(
            move || {
                let client = Client::session().map_err(|error| error.to_string())?;
                action(&client)?;
                snapshot(&client, page)
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
                            state.snapshot.set(snapshot);
                            if let Some(close) = close {
                                close();
                            }
                        }
                        Err(error) => {
                            eprintln!("clipboard popup: {error}");
                            let mut snapshot = state.snapshot.peek();
                            snapshot.available = false;
                            snapshot.message = error;
                            state.snapshot.set(snapshot);
                        }
                    }
                    state.busy.set(false);
                });
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
                    state.load(|_| Ok(()), None);
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
            });
            state.load(|_| Ok(()), None);
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
            refresh: Rc::new(move || refresh.load(|_| Ok(()), None)),
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
        )
    }
}
