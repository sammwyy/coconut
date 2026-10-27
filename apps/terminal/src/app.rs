use crate::{
    config::Config,
    island::{self, Mode, Orientation},
    screen::{Metrics, Selection, TerminalScreen},
    shortcuts::{Action, Binding},
    terminal::{Event, Session, SessionOptions},
    ui::*,
};
use creamui_core::{
    layout::{Dimension, Style},
    BoxedWidget, Rect, Size, StateStyle, Styled, Widget,
};
use creamui_reactive::Signal;
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers},
    AppHandle, WindowHandle,
};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{Icon, RawButton, RawTextInput, RawView, ScrollController, Symbol, Text};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc, Mutex,
    },
};

/// Height of the tab/header strip above the terminal area.
const HEADER: f32 = 44.0;

#[derive(Clone)]
struct Tab {
    session: Rc<RefCell<Session>>,
    selection: Signal<Option<Selection>>,
    island_scroll: ScrollController,
    panes: PaneTree,
    active_pane: Signal<u64>,
    broadcast_input: Signal<bool>,
}

#[derive(Clone)]
struct Pane {
    id: u64,
    session: Rc<RefCell<Session>>,
    selection: Signal<Option<Selection>>,
    island_scroll: ScrollController,
}

#[derive(Clone, Copy)]
enum SplitOrientation {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
enum PaneTree {
    Pane(Pane),
    Split {
        orientation: SplitOrientation,
        ratio: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    fn pane(&self, id: u64) -> Option<Pane> {
        match self {
            Self::Pane(pane) => (pane.id == id).then(|| pane.clone()),
            Self::Split { first, second, .. } => first.pane(id).or_else(|| second.pane(id)),
        }
    }

    fn pane_for_session(&self, session_id: u64) -> Option<Pane> {
        match self {
            Self::Pane(pane) => (pane.session.borrow().id == session_id).then(|| pane.clone()),
            Self::Split { first, second, .. } => first
                .pane_for_session(session_id)
                .or_else(|| second.pane_for_session(session_id)),
        }
    }

    fn panes(&self, result: &mut Vec<Pane>) {
        match self {
            Self::Pane(pane) => result.push(pane.clone()),
            Self::Split { first, second, .. } => {
                first.panes(result);
                second.panes(result);
            }
        }
    }

    fn contains(&self, id: u64) -> bool {
        self.pane(id).is_some()
    }

    fn split(&mut self, id: u64, orientation: SplitOrientation, pane: Pane) -> bool {
        match self {
            Self::Pane(current) if current.id == id => {
                let first = Box::new(Self::Pane(current.clone()));
                *self = Self::Split {
                    orientation,
                    ratio: 0.5,
                    first,
                    second: Box::new(Self::Pane(pane)),
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                first.split(id, orientation, pane.clone()) || second.split(id, orientation, pane)
            }
        }
    }

    fn without(self, id: u64) -> Option<Self> {
        match self {
            Self::Pane(pane) => (pane.id != id).then_some(Self::Pane(pane)),
            Self::Split {
                orientation,
                ratio,
                first,
                second,
            } => match (first.without(id), second.without(id)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    orientation,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(tree), None) | (None, Some(tree)) => Some(tree),
                (None, None) => None,
            },
        }
    }

    fn resize(&mut self, id: u64, amount: f32) -> bool {
        let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        else {
            return false;
        };
        if first.resize(id, amount) || second.resize(id, amount) {
            return true;
        }
        if first.contains(id) {
            *ratio = (*ratio + amount).clamp(0.15, 0.85);
            true
        } else if second.contains(id) {
            *ratio = (*ratio - amount).clamp(0.15, 0.85);
            true
        } else {
            false
        }
    }

    fn resize_sessions(&self, rows: u16, columns: u16) -> Vec<anyhow::Result<()>> {
        match self {
            Self::Pane(pane) => vec![pane.session.borrow_mut().resize(rows, columns)],
            Self::Split {
                orientation,
                ratio,
                first,
                second,
            } => {
                let (first_rows, first_columns, second_rows, second_columns) = match orientation {
                    SplitOrientation::Horizontal => {
                        let first_columns = if columns > 1 {
                            ((columns as f32 * ratio).round() as u16).clamp(1, columns - 1)
                        } else {
                            1
                        };
                        (
                            rows,
                            first_columns,
                            rows,
                            columns.saturating_sub(first_columns).max(1),
                        )
                    }
                    SplitOrientation::Vertical => {
                        let first_rows = if rows > 1 {
                            ((rows as f32 * ratio).round() as u16).clamp(1, rows - 1)
                        } else {
                            1
                        };
                        (
                            first_rows,
                            columns,
                            rows.saturating_sub(first_rows).max(1),
                            columns,
                        )
                    }
                };
                let mut result = first.resize_sessions(first_rows, first_columns);
                result.extend(second.resize_sessions(second_rows, second_columns));
                result
            }
        }
    }
}

impl Tab {
    fn new(pane: Pane) -> Self {
        Self {
            session: pane.session.clone(),
            selection: pane.selection.clone(),
            island_scroll: pane.island_scroll.clone(),
            active_pane: Signal::new(pane.id),
            broadcast_input: Signal::new(false),
            panes: PaneTree::Pane(pane),
        }
    }

    fn all_panes(&self) -> Vec<Pane> {
        let mut panes = Vec::new();
        self.panes.panes(&mut panes);
        panes
    }

    fn focus_pane(&mut self, id: u64) -> bool {
        let Some(pane) = self.panes.pane(id) else {
            return false;
        };
        self.session = pane.session;
        self.selection = pane.selection;
        self.island_scroll = pane.island_scroll;
        self.active_pane.set(id);
        true
    }

    fn input_panes(&self) -> Vec<Pane> {
        if self.broadcast_input.peek() {
            self.all_panes()
        } else {
            self.panes
                .pane(self.active_pane.peek())
                .into_iter()
                .collect()
        }
    }
}
#[derive(Clone)]
pub struct State {
    tabs: Signal<Vec<Tab>>,
    active: Signal<usize>,
    next_id: Rc<std::cell::Cell<u64>>,
    revision: Signal<u64>,
    error: Signal<Option<String>>,
    sender: SyncSender<Event>,
    cwd: PathBuf,
    command: Vec<String>,
    config: Signal<Config>,
    bindings: Rc<RefCell<Vec<(Action, Binding)>>>,
    metrics: Signal<Metrics>,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    clipboard: Rc<RefCell<Option<arboard::Clipboard>>>,
    ctrl_held: Signal<bool>,
    window_focused: Signal<bool>,
    session_file: Rc<RefCell<Option<PathBuf>>>,
}
impl State {
    pub fn new(cwd: PathBuf, command: Vec<String>, sender: SyncSender<Event>) -> Self {
        Self {
            tabs: Signal::new(Vec::new()),
            active: Signal::new(0),
            next_id: Rc::new(std::cell::Cell::new(1)),
            revision: Signal::new(0),
            error: Signal::new(None),
            sender,
            cwd,
            command,
            config: Signal::new(Config::default()),
            bindings: Rc::new(RefCell::new(
                Config::default().bindings().expect("default shortcuts"),
            )),
            metrics: Signal::new(Metrics::load(
                &Config::default().font,
                Config::default().appearance.font_size,
            )),
            window: Rc::new(RefCell::new(None)),
            clipboard: Rc::new(RefCell::new(None)),
            ctrl_held: Signal::new(false),
            window_focused: Signal::new(true),
            session_file: Rc::new(RefCell::new(None)),
        }
    }
    pub fn add(&self) {
        self.add_with(None, None);
    }
    fn add_with(&self, restored_cwd: Option<PathBuf>, manual_title: Option<String>) {
        let active_cwd =
            restored_cwd.or_else(|| self.current().map(|tab| tab.session.borrow().cwd.clone()));
        match self.spawn_pane(active_cwd, manual_title) {
            Ok(pane) => {
                let index = self.tabs.peek().len();
                self.tabs.update(|tabs| tabs.push(Tab::new(pane)));
                self.active.set(index);
                self.error.set(None);
                self.persist_session();
            }
            Err(error) => self.show_error(error),
        }
    }
    fn spawn_pane(
        &self,
        active_cwd: Option<PathBuf>,
        manual_title: Option<String>,
    ) -> Result<Pane, String> {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let config = self.config.peek();
        let (command, cwd, options) = if self.command.is_empty() {
            match config.active_profile() {
                Ok(profile) => {
                    let cwd = active_cwd.unwrap_or_else(|| {
                        profile
                            .initial_directory
                            .as_ref()
                            .map(|path| {
                                if path.is_absolute() {
                                    path.clone()
                                } else {
                                    self.cwd.join(path)
                                }
                            })
                            .unwrap_or_else(|| self.cwd.clone())
                    });
                    (
                        config.profile_command(),
                        cwd,
                        SessionOptions {
                            environment: profile.environment.clone(),
                            scrollback: config.scrollback,
                            theme: profile.theme,
                        },
                    )
                }
                Err(error) => (Err(error), self.cwd.clone(), SessionOptions::default()),
            }
        } else {
            (
                Ok(self.command.clone()),
                self.cwd.clone(),
                SessionOptions {
                    scrollback: config.scrollback,
                    theme: config
                        .active_profile()
                        .map(|profile| profile.theme)
                        .unwrap_or_default(),
                    ..Default::default()
                },
            )
        };
        let result = command.and_then(|command| {
            Session::spawn_with(id, &cwd, &command, &options, self.sender.clone())
                .map_err(|e| format!("{e:#}"))
        });
        let mut session = result.map_err(|error| format!("Couldn't open terminal: {error:#}"))?;
        session.manual_title = manual_title;
        Ok(Pane {
            id,
            session: Rc::new(RefCell::new(session)),
            selection: Signal::new(None),
            island_scroll: ScrollController::new(0.0),
        })
    }
    fn close(&self, index: usize) {
        let mut tabs = self.tabs.peek();
        if index >= tabs.len() {
            return;
        }
        tabs.remove(index);
        if tabs.is_empty() {
            if let Some(window) = self.window.borrow().as_ref() {
                window.close();
            }
        }
        let old = self.active.peek();
        let next = if index < old {
            old - 1
        } else {
            old.min(tabs.len().saturating_sub(1))
        };
        self.tabs.set(tabs);
        self.active.set(next);
        self.persist_session();
    }
    fn split_pane(&self, orientation: SplitOrientation) {
        let Some(tab) = self.current() else {
            return;
        };
        let focused = tab.active_pane.peek();
        let cwd = tab.session.borrow().cwd.clone();
        let pane = match self.spawn_pane(Some(cwd), None) {
            Ok(pane) => pane,
            Err(error) => {
                self.show_error(error);
                return;
            }
        };
        let pane_id = pane.id;
        let active = self.active.peek();
        // `focus_pane` writes `active_pane`. That signal can synchronously rebuild the
        // window, so keep the `tabs` borrow and the resulting redraw in one batch.
        creamui_reactive::batch(|| {
            self.tabs.update(|tabs| {
                if let Some(tab) = tabs.get_mut(active) {
                    if tab.panes.split(focused, orientation, pane) {
                        tab.focus_pane(pane_id);
                    }
                }
            });
            self.revision.update(|value| *value = value.wrapping_add(1));
        });
    }
    fn close_pane(&self) {
        let Some(tab) = self.current() else {
            return;
        };
        let focused = tab.active_pane.peek();
        let mut panes = tab.all_panes();
        if panes.len() <= 1 {
            self.close(self.active.peek());
            return;
        }
        panes.retain(|pane| pane.id != focused);
        let next = panes.first().map(|pane| pane.id);
        let active = self.active.peek();
        creamui_reactive::batch(|| {
            self.tabs.update(|tabs| {
                if let Some(tab) = tabs.get_mut(active) {
                    if let Some(tree) = tab.panes.clone().without(focused) {
                        tab.panes = tree;
                        if let Some(next) = next {
                            tab.focus_pane(next);
                        }
                    }
                }
            });
            self.revision.update(|value| *value = value.wrapping_add(1));
        });
    }
    fn focus_pane(&self, id: u64) {
        self.window_focused.set(true);
        let active = self.active.peek();
        let mut changed = false;
        creamui_reactive::batch(|| {
            self.tabs.update(|tabs| {
                if let Some(tab) = tabs.get_mut(active) {
                    changed = tab.focus_pane(id);
                }
            });
            if changed {
                self.revision.update(|value| *value = value.wrapping_add(1));
            }
        });
    }
    fn focus_relative_pane(&self, direction: isize) {
        let Some(tab) = self.current() else {
            return;
        };
        let panes = tab.all_panes();
        let Some(index) = panes
            .iter()
            .position(|pane| pane.id == tab.active_pane.peek())
        else {
            return;
        };
        let target = (index as isize + direction).rem_euclid(panes.len() as isize) as usize;
        self.focus_pane(panes[target].id);
    }
    fn resize_pane(&self, amount: f32) {
        let Some(tab) = self.current() else {
            return;
        };
        let active = self.active.peek();
        let focused = tab.active_pane.peek();
        let mut changed = false;
        self.tabs.update(|tabs| {
            if let Some(tab) = tabs.get_mut(active) {
                changed = tab.panes.resize(focused, amount);
            }
        });
        if changed {
            self.revision.update(|value| *value = value.wrapping_add(1));
        }
    }
    pub fn cleanup(&self) {
        self.persist_session();
        self.tabs.set(Vec::new());
        self.window.borrow_mut().take();
    }
    pub fn enable_session_restore(&self) {
        *self.session_file.borrow_mut() = Some(crate::session::path());
    }
    pub fn restore_session(&self) -> bool {
        if !self.config.peek().restore_session || !self.command.is_empty() {
            return false;
        }
        let Some(path) = self.session_file.borrow().clone() else {
            return false;
        };
        let saved = match crate::session::load(&path) {
            Ok(Some(saved)) => saved,
            Ok(None) => return false,
            Err(error) => {
                self.show_error(format!("Couldn't restore session: {error}"));
                return false;
            }
        };
        let start = self.tabs.peek().len();
        for tab in saved.tabs {
            if tab.cwd.is_dir() {
                self.add_with(Some(tab.cwd), tab.manual_title);
            }
        }
        let restored = self.tabs.peek().len().saturating_sub(start);
        if restored == 0 {
            return false;
        }
        self.active.set(start + saved.active.min(restored - 1));
        self.persist_session();
        true
    }
    fn persist_session(&self) {
        if !self.config.peek().restore_session {
            return;
        }
        let Some(path) = self.session_file.borrow().clone() else {
            return;
        };
        let tabs = self.tabs.peek();
        let session = crate::session::SavedSession {
            active: self.active.peek().min(tabs.len().saturating_sub(1)),
            tabs: tabs
                .iter()
                .map(|tab| {
                    let session = tab.session.borrow();
                    crate::session::SavedTab {
                        cwd: session.cwd.clone(),
                        manual_title: session.manual_title.clone(),
                    }
                })
                .collect(),
        };
        if let Err(error) = crate::session::save(&path, &session) {
            self.show_error(format!("Couldn't save session: {error}"));
        }
    }
    fn current(&self) -> Option<Tab> {
        self.tabs.peek().get(self.active.peek()).cloned()
    }
    fn report(&self, result: anyhow::Result<()>) {
        if let Err(e) = result {
            let message = e.to_string();
            if self.error.peek().as_ref() != Some(&message) {
                self.error.set(Some(message));
            }
        }
    }
    fn paste(&self) {
        self.ensure_clipboard();
        let text = self
            .clipboard
            .borrow_mut()
            .as_mut()
            .and_then(|c| c.get_text().ok());
        if let Some(text) = text {
            self.paste_text(&text);
        }
    }
    fn paste_text(&self, text: &str) {
        if let Some(tab) = self.current() {
            if tab.session.borrow().control_mode == Some(Mode::Search) {
                self.edit_search(
                    |search| {
                        let range = search.selection.range();
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        search.query.replace_range(range.clone(), &text);
                        search.cursor = range.start + text.len();
                        search.selection = creamui_widgets::TextSelection {
                            anchor: search.cursor,
                            focus: search.cursor,
                        };
                        search.current = None;
                    },
                    true,
                );
                return;
            }
            if self.config.peek().confirm_multiline_paste
                && (text.contains('\n') || text.contains('\r'))
            {
                tab.session.borrow_mut().pending_paste = Some(text.into());
                self.activate_mode(Mode::PasteConfirm);
                return;
            }
            self.paste_into_panes(&tab, text);
        }
    }
    fn confirm_paste(&self) {
        let Some(tab) = self.current() else {
            return;
        };
        let text = {
            let mut session = tab.session.borrow_mut();
            let Some(text) = session.pending_paste.take() else {
                return;
            };
            session.control_mode = None;
            text
        };
        self.paste_into_panes(&tab, &text);
    }
    fn paste_into_panes(&self, tab: &Tab, text: &str) {
        let results = tab
            .input_panes()
            .into_iter()
            .map(|pane| {
                let result = {
                    let mut session = pane.session.borrow_mut();
                    let bracketed = session.parser.screen().bracketed_paste();
                    session.parser.screen_mut().set_scrollback(0);
                    session.write(crate::input::paste(text, bracketed))
                };
                (pane, result)
            })
            .collect();
        self.input_changed_many(results);
    }
    /// Signals can redraw synchronously: callers must release the session first.
    fn input_changed(&self, tab: &Tab, result: anyhow::Result<()>) {
        self.input_changed_many(vec![(
            Pane {
                id: tab.active_pane.peek(),
                session: tab.session.clone(),
                selection: tab.selection.clone(),
                island_scroll: tab.island_scroll.clone(),
            },
            result,
        )]);
    }
    fn input_changed_many(&self, results: Vec<(Pane, anyhow::Result<()>)>) {
        creamui_reactive::batch(|| {
            for (pane, result) in results {
                pane.selection.set(None);
                self.report(result);
            }
            self.revision.update(|v| *v = v.wrapping_add(1));
        });
    }
    fn ensure_clipboard(&self) {
        if self.clipboard.borrow().is_none() {
            match arboard::Clipboard::new() {
                Ok(c) => *self.clipboard.borrow_mut() = Some(c),
                Err(e) => self.error.set(Some(format!("Clipboard unavailable: {e}"))),
            }
        }
    }
    fn copy(&self) {
        if let Some(tab) = self.current() {
            if let Some(selection) = tab.selection.peek() {
                let text = selection.text(tab.session.borrow().parser.screen());
                if !text.is_empty() {
                    self.ensure_clipboard();
                    if let Some(c) = self.clipboard.borrow_mut().as_mut() {
                        if let Err(e) = c.set_text(text) {
                            self.error.set(Some(e.to_string()));
                        }
                    }
                }
            }
        }
    }
    pub fn key(&self, event: KeyEvent, m: Modifiers) -> bool {
        self.ctrl_held.set(m.ctrl);
        self.window_focused.set(true);
        if !event.pressed {
            return true;
        }
        if let Some(mode) = self.current().and_then(|t| t.session.borrow().control_mode) {
            if event.key == Key::Escape {
                self.close_mode();
                return true;
            }
            match mode {
                Mode::Search => match event.key {
                    Key::Enter | Key::Up | Key::Down => {
                        self.navigate_search(
                            event.key == Key::Up || event.key == Key::Enter && m.shift,
                        );
                    }
                    _ => {
                        let action = self
                            .bindings
                            .borrow()
                            .iter()
                            .find(|(_, binding)| binding.matches(&event.key, m))
                            .map(|(action, _)| *action);
                        if action == Some(Action::Search) {
                            return true;
                        }
                        if action == Some(Action::Paste)
                            || m.ctrl
                                && matches!(&event.key, Key::Character(s) if s.eq_ignore_ascii_case("v"))
                        {
                            self.paste();
                            return true;
                        }
                        if let Some(action) = action.filter(|a| *a != Action::Copy) {
                            self.dispatch(action);
                            return true;
                        }
                        if let Some(key) = search_key(&event.key) {
                            if let Some(editor) = self.search_editor() {
                                if let Some(handler) = editor.on_key() {
                                    handler(creamui_core::KeyInput {
                                        key,
                                        modifiers: creamui_core::Modifiers {
                                            ctrl: m.ctrl,
                                            shift: m.shift,
                                            alt: m.alt,
                                            logo: m.logo,
                                        },
                                    });
                                }
                            }
                        }
                    }
                },
                Mode::PasteConfirm => {
                    if event.key == Key::Enter {
                        self.confirm_paste();
                    }
                }
                Mode::Palette => match event.key {
                    Key::Enter => {
                        if let Some(action) = self.palette_actions().into_iter().next() {
                            self.close_mode();
                            self.dispatch(action);
                        }
                    }
                    _ => {
                        let action = self
                            .bindings
                            .borrow()
                            .iter()
                            .find(|(_, binding)| binding.matches(&event.key, m))
                            .map(|(action, _)| *action);
                        if action == Some(Action::Palette) {
                            return true;
                        }
                        if let Some(key) = search_key(&event.key) {
                            if let Some(editor) = self.palette_editor() {
                                if let Some(handler) = editor.on_key() {
                                    handler(creamui_core::KeyInput {
                                        key,
                                        modifiers: creamui_core::Modifiers {
                                            ctrl: m.ctrl,
                                            shift: m.shift,
                                            alt: m.alt,
                                            logo: m.logo,
                                        },
                                    });
                                }
                            }
                        }
                    }
                },
                Mode::RenameTab => match event.key {
                    Key::Enter => self.rename_tab(),
                    _ => {
                        if let Some(key) = search_key(&event.key) {
                            if let Some(editor) = self.palette_editor() {
                                if let Some(handler) = editor.on_key() {
                                    handler(creamui_core::KeyInput {
                                        key,
                                        modifiers: creamui_core::Modifiers {
                                            ctrl: m.ctrl,
                                            shift: m.shift,
                                            alt: m.alt,
                                            logo: m.logo,
                                        },
                                    });
                                }
                            }
                        }
                    }
                },
                Mode::SaveOutput => match event.key {
                    Key::Enter => self.save_output(),
                    _ => {
                        if let Some(key) = search_key(&event.key) {
                            if let Some(editor) = self.palette_editor() {
                                if let Some(handler) = editor.on_key() {
                                    handler(creamui_core::KeyInput {
                                        key,
                                        modifiers: creamui_core::Modifiers {
                                            ctrl: m.ctrl,
                                            shift: m.shift,
                                            alt: m.alt,
                                            logo: m.logo,
                                        },
                                    });
                                }
                            }
                        }
                    }
                },
            }
            return true;
        }
        let action = self
            .bindings
            .borrow()
            .iter()
            .find(|(_, binding)| binding.matches(&event.key, m))
            .map(|(action, _)| *action);
        if let Some(action) = action {
            self.dispatch(action);
            return true;
        }
        if let Some(tab) = self.current() {
            let mut sent = false;
            let results = tab
                .input_panes()
                .into_iter()
                .filter_map(|pane| {
                    let result = {
                        let mut session = pane.session.borrow_mut();
                        let bytes = crate::input::encode(
                            &event.key,
                            m,
                            session.parser.screen().application_cursor(),
                        );
                        if bytes.is_empty() {
                            return None;
                        }
                        sent = true;
                        session.parser.screen_mut().set_scrollback(0);
                        session.write(bytes)
                    };
                    Some((pane, result))
                })
                .collect();
            if sent {
                self.input_changed_many(results);
            }
        }
        true
    }
    pub fn configure(&self, config: Config) {
        match config.bindings() {
            Ok(bindings) => {
                let metrics = Metrics::load(&config.font, config.appearance.font_size);
                creamui_reactive::batch(|| {
                    if !config.island.enabled {
                        for tab in self.tabs.peek() {
                            for pane in tab.all_panes() {
                                let mut session = pane.session.borrow_mut();
                                if session.control_mode == Some(Mode::Search) {
                                    session.set_mode(None);
                                }
                            }
                        }
                    }
                    *self.bindings.borrow_mut() = bindings;
                    self.config.set(config);
                    self.metrics.set(metrics);
                });
                if !self.config.peek().restore_session {
                    if let Some(path) = self.session_file.borrow().as_ref() {
                        if let Err(error) = crate::session::clear(path) {
                            self.show_error(format!("Couldn't clear saved session: {error}"));
                        }
                    }
                }
            }
            Err(error) => self.show_error(error),
        }
    }
    pub fn show_error(&self, message: String) {
        self.error.set(Some(message));
    }
    pub fn set_window_focused(&self, focused: bool) {
        self.window_focused.set(focused);
    }
    fn open_target(&self, target: String) {
        let target = if target.starts_with('/') || target.starts_with("~/") {
            let path = if let Some(rest) = target.strip_prefix("~/") {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(rest)
            } else {
                PathBuf::from(target)
            };
            if !path.exists() {
                self.show_error(format!("File does not exist: {}", path.display()));
                return;
            }
            path.to_string_lossy().into_owned()
        } else {
            target
        };
        match crate::backends::open_url(&target).spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(error) => self.show_error(format!("Couldn't open target: {error}")),
        }
    }
    fn notify_completed_commands(
        &self,
        pane: &Pane,
        completed: Vec<crate::terminal::CompletedCommand>,
    ) {
        let config = self.config.peek();
        let session = pane.session.borrow();
        let directory = session.cwd.display().to_string();
        for command in completed.into_iter().filter(|command| {
            should_notify_for_command(&config, self.window_focused.peek(), command.duration)
        }) {
            let status = command.status.map_or_else(
                || "finished".into(),
                |status| format!("finished ({status})"),
            );
            crate::backends::notify(
                "Command finished",
                &format!(
                    "{status} in {directory} after {}s",
                    command.duration.as_secs()
                ),
            );
        }
    }
    fn dispatch(&self, action: Action) {
        match action {
            Action::NewTab => self.add(),
            Action::NewWindow => self.new_window(),
            Action::CloseTab => self.close(self.active.peek()),
            Action::NextTab => self.switch(1),
            Action::PreviousTab => self.switch(-1),
            Action::SelectTab(index) => {
                if (1..=self.tabs.peek().len()).contains(&index) {
                    self.active.set(index - 1);
                    self.persist_session();
                }
            }
            Action::Copy => self.copy(),
            Action::Paste => self.paste(),
            Action::Search => self.activate_mode(Mode::Search),
            Action::Palette => self.activate_mode(Mode::Palette),
            Action::RenameTab => self.activate_mode(Mode::RenameTab),
            Action::SaveOutput => self.activate_mode(Mode::SaveOutput),
            Action::SplitHorizontal => self.split_pane(SplitOrientation::Horizontal),
            Action::SplitVertical => self.split_pane(SplitOrientation::Vertical),
            Action::ClosePane => self.close_pane(),
            Action::FocusNextPane => self.focus_relative_pane(1),
            Action::FocusPreviousPane => self.focus_relative_pane(-1),
            Action::GrowPane => self.resize_pane(0.05),
            Action::ShrinkPane => self.resize_pane(-0.05),
            Action::PreviousCommand | Action::NextCommand => {
                if let Some(tab) = self.current() {
                    if tab
                        .session
                        .borrow_mut()
                        .navigate_command(action == Action::PreviousCommand)
                    {
                        self.revision.update(|value| *value = value.wrapping_add(1));
                    }
                }
            }
            Action::ToggleBroadcastInput => {
                if let Some(tab) = self.current() {
                    tab.broadcast_input.update(|enabled| *enabled = !*enabled);
                    self.revision.update(|value| *value = value.wrapping_add(1));
                }
            }
            Action::Settings => open_settings(),
            Action::ScrollUp | Action::ScrollDown => {
                if let Some(tab) = self.current() {
                    let mut s = tab.session.borrow_mut();
                    if s.parser.screen().alternate_screen() {
                        return;
                    }
                    let rows = s.parser.screen().size().0 as usize;
                    let old = s.parser.screen().scrollback();
                    let offset = if action == Action::ScrollUp {
                        old + rows
                    } else {
                        old.saturating_sub(rows)
                    };
                    s.parser.screen_mut().set_scrollback(offset);
                    drop(s);
                    self.input_changed(&tab, Ok(()));
                }
            }
        }
    }
    fn edit_search(&self, edit: impl FnOnce(&mut crate::search::Search), refresh: bool) {
        if let Some(tab) = self.current() {
            {
                let mut session = tab.session.borrow_mut();
                let Session { search, parser, .. } = &mut *session;
                edit(search);
                if refresh {
                    search.refresh(parser.screen());
                    search.reveal(parser.screen_mut());
                }
            }
            self.revision.update(|v| *v = v.wrapping_add(1));
        }
    }
    fn activate_mode(&self, mode: Mode) {
        if !self.config.peek().island.enabled && mode == Mode::Search {
            return;
        }
        if let Some(tab) = self.current() {
            let mut session = tab.session.borrow_mut();
            session.set_mode(Some(mode));
            if mode == Mode::SaveOutput {
                session.palette.query = session
                    .cwd
                    .join("terminal-output.txt")
                    .to_string_lossy()
                    .into_owned();
                session.palette.cursor = session.palette.query.len();
                session.palette.selection = creamui_widgets::TextSelection {
                    anchor: 0,
                    focus: session.palette.cursor,
                };
            }
            drop(session);
            creamui_reactive::batch(|| {
                tab.island_scroll.set(0.0);
                self.revision.update(|v| *v = v.wrapping_add(1));
            });
        }
    }
    fn close_mode(&self) {
        if let Some(tab) = self.current() {
            tab.session.borrow_mut().set_mode(None);
            self.revision.update(|v| *v = v.wrapping_add(1));
        }
    }
    fn rename_tab(&self) {
        if let Some(tab) = self.current() {
            let mut session = tab.session.borrow_mut();
            let title = session.palette.query.trim();
            session.manual_title = (!title.is_empty()).then(|| title.to_string());
            session.control_mode = None;
            drop(session);
            self.persist_session();
            self.revision.update(|value| *value = value.wrapping_add(1));
        }
    }
    fn save_output(&self) {
        let Some(tab) = self.current() else {
            return;
        };
        let result = {
            let mut session = tab.session.borrow_mut();
            let raw_path = session.palette.query.trim();
            if raw_path.is_empty() {
                Err(anyhow::anyhow!("Enter a file path for terminal output"))
            } else {
                let path = PathBuf::from(raw_path);
                let path = if path.is_absolute() {
                    path
                } else {
                    session.cwd.join(path)
                };
                let result = crate::terminal::save_output(&path, session.parser.screen());
                if result.is_ok() {
                    session.control_mode = None;
                }
                result
            }
        };
        if result.is_ok() {
            self.revision.update(|value| *value = value.wrapping_add(1));
        }
        self.report(result);
    }
    fn navigate_search(&self, previous: bool) {
        if let Some(tab) = self.current() {
            {
                let mut session = tab.session.borrow_mut();
                let Session { search, parser, .. } = &mut *session;
                search.navigate(parser.screen_mut(), previous);
            }
            self.revision.update(|v| *v = v.wrapping_add(1));
        }
    }
    // Native key callbacks run outside the window's context scope. Keep
    // construction of the editing handler independent of rendering hooks.
    fn search_editor(&self) -> Option<RawTextInput> {
        let tab = self.current()?;
        let session = tab.session.borrow();
        let search = &session.search;
        let change = self.clone();
        let cursor = self.clone();
        let selection = self.clone();
        Some(
            RawTextInput::new(
                pad(
                    Style {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..fixed(180.0, 34.0)
                    },
                    10.0,
                    0.0,
                ),
                search.query.clone(),
                14.0,
                CLEAR,
                move |value| {
                    change.edit_search(
                        |search| {
                            search.query = value;
                            search.current = None;
                        },
                        true,
                    )
                },
            )
            .cursor(search.cursor, move |value| {
                cursor.edit_search(|s| s.cursor = value, false)
            })
            .selection(search.selection, move |value| {
                selection.edit_search(|s| s.selection = value, false)
            }),
        )
    }
    fn search_input(&self) -> Option<crate::search::SearchInput> {
        let editor = self.search_editor()?;
        let theme = use_theme();
        Some(crate::search::SearchInput(
            editor
                .color(theme.colors.text_primary)
                .placeholder("Search scrollback…", theme.colors.text_secondary)
                .background(theme.colors.surface)
                .corner_radius(6.0)
                .border(theme.colors.accent, 1.0)
                .selection_background(theme.colors.accent.mix(theme.colors.surface, 0.5)),
        ))
    }
    fn edit_palette(&self, edit: impl FnOnce(&mut crate::terminal::Palette)) {
        if let Some(tab) = self.current() {
            edit(&mut tab.session.borrow_mut().palette);
            self.revision.update(|value| *value = value.wrapping_add(1));
        }
    }
    fn palette_editor(&self) -> Option<RawTextInput> {
        let tab = self.current()?;
        let session = tab.session.borrow();
        let palette = &session.palette;
        let change = self.clone();
        let cursor = self.clone();
        let selection = self.clone();
        Some(
            RawTextInput::new(
                pad(
                    Style {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..fixed(220.0, 34.0)
                    },
                    10.0,
                    0.0,
                ),
                palette.query.clone(),
                14.0,
                CLEAR,
                move |value| change.edit_palette(|palette| palette.query = value),
            )
            .cursor(palette.cursor, move |value| {
                cursor.edit_palette(|palette| palette.cursor = value)
            })
            .selection(palette.selection, move |value| {
                selection.edit_palette(|palette| palette.selection = value)
            }),
        )
    }
    fn palette_input_with(&self, placeholder: &str) -> Option<crate::search::SearchInput> {
        let editor = self.palette_editor()?;
        let theme = use_theme();
        Some(crate::search::SearchInput(
            editor
                .color(theme.colors.text_primary)
                .placeholder(placeholder, theme.colors.text_secondary)
                .background(theme.colors.surface)
                .corner_radius(6.0)
                .border(theme.colors.accent, 1.0)
                .selection_background(theme.colors.accent.mix(theme.colors.surface, 0.5)),
        ))
    }
    fn palette_input(&self) -> Option<crate::search::SearchInput> {
        self.palette_input_with("Type an action…")
    }
    fn palette_actions(&self) -> Vec<Action> {
        let query = self
            .current()
            .map(|tab| tab.session.borrow().palette.query.to_lowercase())
            .unwrap_or_default();
        Action::all()
            .into_iter()
            .filter(|action| *action != Action::Palette)
            .filter(|action| {
                query.is_empty()
                    || action.label().to_lowercase().contains(&query)
                    || action.id().contains(&query)
            })
            .collect()
    }
    fn search_content(
        &self,
        orientation: Orientation,
        available_width: f32,
    ) -> Option<(BoxedWidget, Size)> {
        let editor = self.search_input()?;
        let tab = self.current()?;
        let session = tab.session.borrow();
        let search = &session.search;
        let label = if search.error.is_some() {
            "Invalid regex".into()
        } else if search.query.is_empty() {
            "".into()
        } else {
            format!(
                "{}/{}",
                search.current.map_or(0, |i| i + 1),
                search.matches.len()
            )
        };
        let regex = self.clone();
        let up = self.clone();
        let down = self.clone();
        let close = self.clone();
        let vertical = orientation == Orientation::Vertical;
        let input_width = (available_width - 32.0).clamp(1.0, 224.0);
        let control_width = if vertical { input_width } else { 30.0 };
        let preferred = if vertical {
            Size {
                width: 240.0,
                height: 238.0,
            }
        } else {
            Size {
                width: 560.0,
                height: 50.0,
            }
        };
        let mut editor = editor;
        if vertical {
            editor.0 = editor.0.layout(pad(fixed(input_width, 34.0), 10.0, 0.0));
        }
        let content = view(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..if vertical { column(6.0) } else { row(6.0) }
            },
            vec![
                Box::new(editor),
                Box::new(Text::secondary(label).font_size(12.0).layout(fixed(
                    if vertical { input_width } else { 82.0 },
                    if vertical { 22.0 } else { 34.0 },
                ))),
                button(
                    if search.regex { ".* ✓" } else { ".*" },
                    if vertical { input_width } else { 48.0 },
                    move || {
                        regex.edit_search(
                            |s| {
                                s.regex = !s.regex;
                                s.current = None;
                            },
                            true,
                        )
                    },
                ),
                button("↑", control_width, move || up.navigate_search(true)),
                button("↓", control_width, move || down.navigate_search(false)),
                button("×", control_width, move || close.close_mode()),
            ],
        );
        Some((content, preferred))
    }
    fn paste_confirmation_content(&self, orientation: Orientation) -> Option<(BoxedWidget, Size)> {
        let tab = self.current()?;
        let pending = tab.session.borrow().pending_paste.as_ref()?.clone();
        let lines = pending.matches('\n').count() + pending.matches('\r').count() + 1;
        let confirm = self.clone();
        let cancel = self.clone();
        let vertical = orientation == Orientation::Vertical;
        let preferred = if vertical {
            Size {
                width: 260.0,
                height: 154.0,
            }
        } else {
            Size {
                width: 460.0,
                height: 82.0,
            }
        };
        Some((
            view(
                if vertical { column(8.0) } else { row(8.0) },
                vec![
                    view(
                        Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            ..column(2.0)
                        },
                        vec![
                            Box::new(Text::new("Paste multiple lines?").font_size(14.0)),
                            Box::new(
                                Text::secondary(format!(
                                    "This will send {lines} lines to the terminal."
                                ))
                                .font_size(12.0),
                            ),
                        ],
                    ),
                    button("Cancel", if vertical { 244.0 } else { 68.0 }, move || {
                        cancel.close_mode()
                    }),
                    button("Paste", if vertical { 244.0 } else { 62.0 }, move || {
                        confirm.confirm_paste()
                    }),
                ],
            ),
            preferred,
        ))
    }
    fn palette_content(&self, orientation: Orientation) -> Option<(BoxedWidget, Size)> {
        let input = self.palette_input()?;
        let vertical = orientation == Orientation::Vertical;
        let actions = self.palette_actions();
        let preferred = Size {
            width: 440.0,
            height: if vertical { 360.0 } else { 282.0 },
        };
        let mut children: Vec<BoxedWidget> = vec![Box::new(input)];
        if actions.is_empty() {
            children.push(Box::new(
                Text::secondary("No matching actions.").font_size(12.0),
            ));
        } else {
            for action in actions.into_iter().take(8) {
                let invoke = self.clone();
                let shortcut = self
                    .bindings
                    .borrow()
                    .iter()
                    .find(|(candidate, _)| *candidate == action)
                    .map(|(_, binding)| binding.display())
                    .unwrap_or_else(|| "Unassigned".into());
                children.push(button(
                    &format!("{}  ·  {shortcut}", action.label()),
                    424.0,
                    move || {
                        invoke.close_mode();
                        invoke.dispatch(action);
                    },
                ));
            }
        }
        Some((view(column(6.0), children), preferred))
    }
    fn rename_tab_content(&self, orientation: Orientation) -> Option<(BoxedWidget, Size)> {
        let input = self.palette_input()?;
        let rename = self.clone();
        let clear = self.clone();
        let vertical = orientation == Orientation::Vertical;
        let preferred = if vertical {
            Size {
                width: 260.0,
                height: 150.0,
            }
        } else {
            Size {
                width: 460.0,
                height: 82.0,
            }
        };
        Some((
            view(
                if vertical { column(8.0) } else { row(8.0) },
                vec![
                    view(
                        Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            ..column(4.0)
                        },
                        vec![
                            Box::new(Text::secondary("Tab name").font_size(12.0)),
                            Box::new(input),
                        ],
                    ),
                    button("Clear", if vertical { 244.0 } else { 60.0 }, move || {
                        if let Some(tab) = clear.current() {
                            let mut session = tab.session.borrow_mut();
                            session.manual_title = None;
                            session.control_mode = None;
                        }
                        clear.persist_session();
                        clear
                            .revision
                            .update(|value| *value = value.wrapping_add(1));
                    }),
                    button("Save", if vertical { 244.0 } else { 56.0 }, move || {
                        rename.rename_tab()
                    }),
                ],
            ),
            preferred,
        ))
    }
    fn save_output_content(&self, orientation: Orientation) -> Option<(BoxedWidget, Size)> {
        let input = self.palette_input_with("Output file path…")?;
        let save = self.clone();
        let cancel = self.clone();
        let vertical = orientation == Orientation::Vertical;
        let preferred = if vertical {
            Size {
                width: 300.0,
                height: 154.0,
            }
        } else {
            Size {
                width: 500.0,
                height: 82.0,
            }
        };
        let width = if vertical { 284.0 } else { 56.0 };
        Some((
            view(
                if vertical { column(8.0) } else { row(8.0) },
                vec![
                    view(
                        Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            ..column(4.0)
                        },
                        vec![
                            Box::new(Text::secondary("Save terminal output").font_size(12.0)),
                            Box::new(input),
                        ],
                    ),
                    button("Cancel", if vertical { 284.0 } else { 68.0 }, move || {
                        cancel.close_mode()
                    }),
                    button("Save", width, move || save.save_output()),
                ],
            ),
            preferred,
        ))
    }
    fn control_island(&self, area: Rect) -> Option<BoxedWidget> {
        let config = self.config.get().island;
        let tab = self.current()?;
        let mode = tab.session.borrow().control_mode?;
        if !config.enabled && mode == Mode::Search {
            return None;
        }
        let (content, preferred) = match mode {
            Mode::Search => self.search_content(config.orientation, area.width)?,
            Mode::PasteConfirm => self.paste_confirmation_content(config.orientation)?,
            Mode::Palette => self.palette_content(config.orientation)?,
            Mode::RenameTab => self.rename_tab_content(config.orientation)?,
            Mode::SaveOutput => self.save_output_content(config.orientation)?,
        };
        Some(island::surface(
            &config,
            area,
            preferred,
            content,
            tab.island_scroll.clone(),
        ))
    }
    fn new_window(&self) {
        let cwd = self
            .current()
            .map(|tab| tab.session.borrow().cwd.clone())
            .unwrap_or_else(|| self.cwd.clone());
        let result = std::env::current_exe().and_then(|exe| {
            let mut command = std::process::Command::new(exe);
            command.arg("--working-directory").arg(cwd);
            if !self.command.is_empty() {
                command.arg("-e").args(&self.command);
            }
            command.spawn()
        });
        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(error) => self.show_error(format!("Couldn't open a new window: {error}")),
        }
    }
    fn switch(&self, direction: isize) {
        let n = self.tabs.peek().len();
        if n > 0 {
            self.active
                .set((self.active.peek() as isize + direction).rem_euclid(n as isize) as usize);
            self.persist_session();
        }
    }
    fn event(&self, event: Event) {
        let id = match &event {
            Event::Configuration(_) => {
                if let Event::Configuration(result) = event {
                    match result {
                        Ok(config) => self.configure(config),
                        Err(error) => self.show_error(error),
                    }
                }
                return;
            }
            Event::Output(id, _) | Event::Exited(id, _) => *id,
        };
        if let Some(pane) = self
            .tabs
            .peek()
            .iter()
            .find_map(|tab| tab.panes.pane_for_session(id))
        {
            match event {
                Event::Output(_, bytes) => {
                    let (result, cwd_changed, completed) = {
                        let mut session = pane.session.borrow_mut();
                        let previous_cwd = session.cwd.clone();
                        let result = session.process(&bytes);
                        let cwd_changed = session.cwd != previous_cwd;
                        let completed = session.take_completed_commands();
                        (result, cwd_changed, completed)
                    };
                    creamui_reactive::batch(|| {
                        pane.selection.set(None);
                        self.report(result);
                    });
                    self.notify_completed_commands(&pane, completed);
                    if cwd_changed {
                        self.persist_session();
                    }
                }
                Event::Exited(_, status) => pane.session.borrow_mut().finish(status),
                Event::Configuration(_) => unreachable!(),
            }
        }
    }
    fn pane_view(
        &self,
        tree: &PaneTree,
        focused: u64,
        show_focus: bool,
        metrics: Metrics,
    ) -> BoxedWidget {
        let appearance = self.config.peek().appearance.clone();
        match tree {
            PaneTree::Pane(pane) => {
                let focus = self.clone();
                let pane_id = pane.id;
                Box::new(TerminalScreen {
                    session: pane.session.clone(),
                    selection: pane.selection.clone(),
                    revision: self.revision.clone(),
                    metrics,
                    ctrl_held: self.ctrl_held.clone(),
                    open_target: {
                        let state = self.clone();
                        Some(Rc::new(move |target| state.open_target(target)))
                    },
                    focused: show_focus && pane_id == focused,
                    on_focus: Some(Rc::new(move || focus.focus_pane(pane_id))),
                    ligatures: appearance.ligatures,
                    background_opacity: appearance.opacity,
                })
            }
            PaneTree::Split {
                orientation,
                ratio,
                first,
                second,
            } => {
                let slot = |weight, child| {
                    view(
                        Style {
                            flex_grow: weight,
                            flex_basis: Dimension::Length(0.0),
                            min_size: creamui_core::layout::Size {
                                width: Dimension::Length(0.0),
                                height: Dimension::Length(0.0),
                            },
                            ..Default::default()
                        },
                        vec![child],
                    )
                };
                let style = match orientation {
                    SplitOrientation::Horizontal => row(1.0),
                    SplitOrientation::Vertical => column(1.0),
                };
                view(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Length(0.0),
                        },
                        ..style
                    },
                    vec![
                        slot(*ratio, self.pane_view(first, focused, show_focus, metrics)),
                        slot(
                            1.0 - ratio,
                            self.pane_view(second, focused, show_focus, metrics),
                        ),
                    ],
                )
            }
        }
    }
    pub fn build(&self, size: Size) -> BoxedWidget {
        self.revision.get();
        let theme = use_theme();
        let tabs = self.tabs.get();
        let active = self.active.get();
        let opacity = self.config.get().appearance.opacity;
        let error = self.error.get();
        let exited = tabs
            .get(active)
            .and_then(|t| t.session.borrow().exited.clone());
        let status = error
            .clone()
            .or_else(|| exited.map(|s| format!("{s} · Close this tab or open a new one.")));
        let screen_height =
            (size.height - HEADER - if status.is_some() { 36.0 } else { 0.0 }).max(1.0);
        let metrics = self.metrics.get();
        let (rows, cols) = metrics.size(size.width, screen_height);
        for tab in &tabs {
            for result in tab.panes.resize_sessions(rows, cols) {
                self.report(result);
            }
        }
        let mut children = vec![self.header(size.width, opacity)];
        if let Some(tab) = tabs.get(active) {
            children.push(self.pane_view(
                &tab.panes,
                tab.active_pane.get(),
                tab.all_panes().len() > 1,
                metrics,
            ));
        } else {
            let retry = self.clone();
            children.push(view(
                Style {
                    flex_grow: 1.0,
                    ..pad(column(16.0), 24.0, 24.0)
                },
                vec![
                    Box::new(Text::secondary("Open a terminal to start working.")),
                    button("+", 40.0, move || retry.add()),
                ],
            ));
        }
        if let Some(status) = status {
            let dismiss = self.clone();
            children.push(Box::new(
                RawView::new(pad(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Length(36.0),
                        },
                        ..row(8.0)
                    },
                    14.0,
                    0.0,
                ))
                .background(with_opacity(theme.colors.surface_elevated, opacity))
                .with_children(vec![
                    Box::new(Text::secondary(status).font_size(12.0).layout(Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..Default::default()
                    })),
                    button("×", 28.0, move || {
                        if error.is_some() {
                            dismiss.error.set(None)
                        } else {
                            dismiss.close(dismiss.active.peek())
                        }
                    }),
                ]),
            ));
        }
        if let Some(bar) = self.control_island(Rect {
            x: 0.0,
            y: HEADER,
            width: size.width,
            height: screen_height,
        }) {
            children.push(bar);
        }
        Box::new(
            RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(size.width),
                    height: Dimension::Length(size.height),
                },
                ..column(0.0)
            })
            .background(with_opacity(theme.colors.surface, opacity))
            .with_children(children),
        )
    }
    fn actions(&self) -> BoxedWidget {
        let theme = use_theme();
        let add = self.clone();
        view(
            fixed(70.0, 34.0),
            vec![
                Box::new(
                    RawButton::new(
                        Style {
                            justify_content: Some(creamui_core::layout::JustifyContent::Center),
                            ..fixed(34.0, 34.0)
                        },
                        open_settings,
                    )
                    .corner_radius(8.0)
                    .hover_style(StateStyle::new().background(theme.colors.surface_hover))
                    .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
                    .child(Box::new(
                        Icon::new(Symbol::Sliders, theme.colors.text_secondary).size(17.0),
                    )),
                ),
                button("+", 34.0, move || add.add()),
            ],
        )
    }
    fn tab_title(tab: &Tab) -> String {
        let session = tab.session.borrow();
        if let Some(title) = &session.manual_title {
            return title.clone();
        }
        let title = &session.parser.callbacks().title;
        if !title.is_empty() {
            title.clone()
        } else if session.exited.is_some() {
            "Exited".into()
        } else if let Some(name) = session.cwd.file_name() {
            name.to_string_lossy().into_owned()
        } else {
            "Terminal".into()
        }
    }
    fn header(&self, width: f32, opacity: u8) -> BoxedWidget {
        let theme = use_theme();
        let titlebar = creamui_widgets::AppTitlebar::new(width)
            .height(HEADER)
            .padding(8.0)
            .center_margin(86.0)
            .start_boxed(self.actions());
        let (start, end) = titlebar.reserved();
        let available = (width - 2.0 * (start.max(end) + 86.0)).max(1.0);
        let tabs = self.tabs.get();
        let active = self.active.get().min(tabs.len().saturating_sub(1));
        let center: BoxedWidget = if tabs.len() <= 1 {
            let title = tabs
                .first()
                .map(Self::tab_title)
                .unwrap_or_else(|| "Terminal".into());
            let max_chars = (available / 7.0).floor().max(1.0) as usize;
            let title = if title.chars().count() > max_chars {
                format!(
                    "{}…",
                    title
                        .chars()
                        .take(max_chars.saturating_sub(1))
                        .collect::<String>()
                )
            } else {
                title
            };
            Box::new(
                Text::new(title)
                    .font_size(13.0)
                    .text_align(creamui_core::TextAlign::Center)
                    .layout(fixed(available, HEADER)),
            )
        } else {
            let capacity = ((available - 64.0) / 92.0).floor().max(1.0) as usize;
            let paged = tabs.len() > capacity;
            let first = if paged {
                active / capacity * capacity
            } else {
                0
            };
            let last = (first + capacity).min(tabs.len());
            let count = (last - first).max(1);
            let maximum =
                ((available - if paged { 64.0 } else { 0.0 }) / count as f32 - 4.0).max(44.0);
            let mut strip = Vec::new();
            if paged {
                let state = self.clone();
                strip.push(button("‹", 28.0, move || state.switch(-1)));
            }
            for (index, tab) in tabs.iter().enumerate().take(last).skip(first) {
                let title = Self::tab_title(tab);
                let tab_width = (title.chars().count() as f32 * 7.0 + 44.0)
                    .clamp(76.0, 180.0)
                    .min(maximum);
                let max_chars = ((tab_width - 44.0) / 7.0).floor().max(1.0) as usize;
                let title = if title.chars().count() > max_chars {
                    format!(
                        "{}…",
                        title
                            .chars()
                            .take(max_chars.saturating_sub(1))
                            .collect::<String>()
                    )
                } else {
                    title
                };
                let choose = self.clone();
                let close = self.clone();
                strip.push(Box::new(
                    RawView::new(fixed(tab_width, 34.0))
                        .background(if index == active {
                            with_opacity(theme.colors.surface_elevated, opacity)
                        } else {
                            CLEAR
                        })
                        .corner_radius(8.0)
                        .with_children(vec![
                            Box::new(
                                RawButton::new(
                                    pad(fixed((tab_width - 28.0).max(0.0), 34.0), 8.0, 0.0),
                                    move || choose.active.set(index),
                                )
                                .corner_radius(8.0)
                                .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
                                .child(Box::new(
                                    Text::new(title).font_size(12.0).color(if index == active {
                                        theme.colors.text_primary
                                    } else {
                                        theme.colors.text_secondary
                                    }),
                                )),
                            ),
                            button("×", 28.0, move || close.close(index)),
                        ]),
                ) as BoxedWidget);
            }
            if paged {
                let state = self.clone();
                strip.push(button("›", 28.0, move || state.switch(1)));
            }
            view(fixed(available, 34.0), vec![view(row(4.0), strip)])
        };
        Box::new(titlebar.center_boxed(center).background(with_opacity(
            theme.colors.surface.mix(theme.colors.accent, 0.025),
            opacity,
        )))
    }
}

fn with_opacity(color: Color, opacity: u8) -> Color {
    Color::rgba(
        color.r,
        color.g,
        color.b,
        (color.a as u16 * opacity as u16 / 100) as u8,
    )
}

fn should_notify_for_command(
    config: &Config,
    window_focused: bool,
    duration: std::time::Duration,
) -> bool {
    config.notify_on_command_completion
        && !window_focused
        && duration
            >= std::time::Duration::from_secs(config.command_completion_threshold_seconds.into())
}

fn search_key(key: &Key) -> Option<creamui_core::Key> {
    use creamui_core::Key as K;
    Some(match key {
        Key::Character(s) => K::Char(s.chars().next()?),
        Key::Space => K::Char(' '),
        Key::Backspace => K::Backspace,
        Key::Delete => K::Delete,
        Key::Left => K::Left,
        Key::Right => K::Right,
        Key::Home => K::Home,
        Key::End => K::End,
        _ => return None,
    })
}

pub fn open_settings() {
    match std::env::current_exe()
        .and_then(|exe| std::process::Command::new(exe).arg("--settings").spawn())
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => eprintln!("Couldn't open settings: {e}"),
    }
}
pub fn watch(app: AppHandle, events: Arc<Mutex<Receiver<Event>>>, state: State) {
    let next_app = app.clone();
    let next_events = events.clone();
    app.spawn_background(
        move || {
            let receiver = events.lock().unwrap();
            let Ok(first) = receiver.recv() else {
                return Vec::new();
            };
            let mut batch = vec![first];
            // Coalesce bursts, bound each update so output cannot starve UI input.
            batch.extend(receiver.try_iter().take(63));
            batch
        },
        move |batch| {
            if batch.is_empty() {
                return;
            }
            creamui_reactive::batch(|| {
                for event in batch {
                    state.event(event);
                }
                state.revision.update(|v| *v = v.wrapping_add(1));
            });
            watch(next_app, next_events, state);
        },
    );
}

/// Keep settings saved by a separate --settings process live in every window.
pub fn watch_configuration(sender: SyncSender<Event>) {
    let mut previous = crate::config::fingerprint();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let current = crate::config::fingerprint();
        if current != previous {
            previous = current;
            if sender.send(Event::Configuration(Config::load())).is_err() {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Point, Renderer};
    use creamui_render::platform::Key;
    use creamui_render::{
        CompositorControls, Damage, Rasterizer, SceneRecorder, WindowDecorationMode,
        WindowDecorations,
    };
    use creamui_theme::{Theme, ThemeProvider};

    #[test]
    fn completion_notifications_require_an_unfocused_long_running_command() {
        let config = Config::default();
        assert!(!should_notify_for_command(
            &config,
            true,
            std::time::Duration::from_secs(60),
        ));
        assert!(!should_notify_for_command(
            &config,
            false,
            std::time::Duration::from_secs(9),
        ));
        assert!(should_notify_for_command(
            &config,
            false,
            std::time::Duration::from_secs(10),
        ));
        let mut disabled = config;
        disabled.notify_on_command_completion = false;
        assert!(!should_notify_for_command(
            &disabled,
            false,
            std::time::Duration::from_secs(60),
        ));
    }

    #[test]
    fn panes_split_focus_resize_and_close() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let state = State::new(
                crate::terminal::idle_cwd(),
                crate::terminal::idle_command(),
                tx,
            );
            state.add();
            let first = state.current().unwrap().session.borrow().id;

            state.dispatch(Action::SplitHorizontal);
            let tab = state.current().unwrap();
            assert_eq!(tab.all_panes().len(), 2);
            let second = tab.session.borrow().id;
            assert_ne!(second, first);

            state.dispatch(Action::ToggleBroadcastInput);
            let tab = state.current().unwrap();
            assert!(tab.broadcast_input.peek());
            assert_eq!(tab.input_panes().len(), 2);
            for pane in tab.all_panes() {
                let mut session = pane.session.borrow_mut();
                session.resize(2, 20).unwrap();
                session.process(b"one\r\ntwo\r\nthree").unwrap();
                session.parser.screen_mut().set_scrollback(1);
            }
            state.key(
                KeyEvent {
                    key: Key::Character("x".into()),
                    pressed: true,
                    synthetic: false,
                },
                Modifiers::default(),
            );
            assert!(tab.all_panes().iter().all(|pane| pane
                .session
                .borrow()
                .parser
                .screen()
                .scrollback()
                == 0));

            state.dispatch(Action::GrowPane);
            let tab = state.current().unwrap();
            let PaneTree::Split { ratio, .. } = &tab.panes else {
                panic!("expected a split pane tree");
            };
            assert!(*ratio < 0.5);

            state.dispatch(Action::FocusPreviousPane);
            assert_eq!(state.current().unwrap().session.borrow().id, first);
            state.dispatch(Action::SplitVertical);
            assert_eq!(state.current().unwrap().all_panes().len(), 3);
            let _tree = state.build(Size {
                width: 900.0,
                height: 640.0,
            });

            state.dispatch(Action::ClosePane);
            assert_eq!(state.current().unwrap().all_panes().len(), 2);
            state.dispatch(Action::ClosePane);
            assert_eq!(state.current().unwrap().all_panes().len(), 1);
            state.cleanup();
        });
    }

    #[test]
    fn island_modes_reposition_live_and_all_controls_remain_clickable() {
        use crate::island::{Horizontal, Vertical};
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let state = State::new(
                crate::terminal::idle_cwd(),
                crate::terminal::idle_command(),
                tx,
            );
            state.add();
            let tab = state.current().unwrap();
            tab.session
                .borrow_mut()
                .process(b"entry 1\r\nentry 2\r\nentry 3")
                .unwrap();
            for orientation in [Orientation::Horizontal, Orientation::Vertical] {
                for horizontal in [Horizontal::Left, Horizontal::Center, Horizontal::Right] {
                    for vertical in [Vertical::Top, Vertical::Center, Vertical::Bottom] {
                        let mut config = state.config.peek();
                        config.island = island::Config {
                            enabled: true,
                            orientation,
                            horizontal,
                            vertical,
                        };
                        state.configure(config.clone());
                        state.activate_mode(Mode::Search);
                        state.edit_search(
                            |s| {
                                s.query = "entry [0-9]".into();
                                s.regex = false;
                            },
                            true,
                        );
                        // Status messages reduce the usable area and must remain clear.
                        state.show_error("Example status".into());
                        let size = Size {
                            width: 640.0,
                            height: 420.0,
                        };
                        let mut recorder = SceneRecorder::new();
                        recorder.begin(640, 420, 1.0, CLEAR, theme.colors);
                        let scene = Renderer::new().render(state.build(size), size, &mut recorder);
                        let preferred = if orientation == Orientation::Horizontal {
                            Size {
                                width: 560.0,
                                height: 50.0,
                            }
                        } else {
                            Size {
                                width: 240.0,
                                height: 238.0,
                            }
                        };
                        let bounds = config.island.bounds(
                            Rect {
                                x: 0.0,
                                y: HEADER,
                                width: size.width,
                                height: size.height - HEADER - 36.0,
                            },
                            preferred,
                        );
                        let regex = if orientation == Orientation::Horizontal {
                            Point {
                                x: bounds.x + bounds.width - 140.0,
                                y: bounds.y + 25.0,
                            }
                        } else {
                            Point {
                                x: bounds.x + bounds.width / 2.0,
                                y: bounds.y + 93.0,
                            }
                        };
                        scene
                            .hit_test(regex)
                            .expect("regex control at selected anchor")();
                        assert!(tab.session.borrow().search.regex);
                        assert_eq!(tab.session.borrow().search.matches.len(), 3);
                        let close = if orientation == Orientation::Horizontal {
                            Point {
                                x: bounds.x + bounds.width - 23.0,
                                y: bounds.y + 25.0,
                            }
                        } else {
                            Point {
                                x: bounds.x + bounds.width / 2.0,
                                y: bounds.y + bounds.height - 25.0,
                            }
                        };
                        if let Some(dir) = std::env::var_os("CTERM_SNAPSHOT_DIR") {
                            let mut raster = Rasterizer::new(640, 420);
                            raster.render(&recorder.finish(), &Damage::Full);
                            raster
                                .pixmap()
                                .save_png(PathBuf::from(dir).join(format!(
                                    "island-{orientation:?}-{horizontal:?}-{vertical:?}.png"
                                )))
                                .unwrap();
                        }
                        scene
                            .hit_test(close)
                            .expect("close control at selected anchor")();
                        assert_eq!(tab.session.borrow().control_mode, None);
                        assert!(tab.session.borrow().search.matches.is_empty());
                    }
                }
            }
            state.cleanup();
        });
    }

    #[test]
    fn disabling_island_closes_modes_in_all_tabs_and_blocks_activation() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        for _ in 0..2 {
            state.add();
            state.dispatch(Action::Search);
            state.paste_text("query");
        }
        let mut config = state.config.peek();
        config.island.enabled = false;
        state.configure(config.clone());
        for tab in state.tabs.peek() {
            let session = tab.session.borrow();
            assert_eq!(session.control_mode, None);
            assert!(session.search.matches.is_empty());
            assert_eq!(session.search.query, "query");
        }
        state.dispatch(Action::Search);
        assert_eq!(state.current().unwrap().session.borrow().control_mode, None);
        config.island.enabled = true;
        state.configure(config);
        state.dispatch(Action::Search);
        assert_eq!(
            state.current().unwrap().session.borrow().control_mode,
            Some(Mode::Search)
        );
        state.cleanup();
    }

    #[test]
    fn native_search_keys_work_outside_the_window_context() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        let build = state.clone();
        let effect = creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            creamui_reactive::create_effect(move || {
                let _ = build.build(Size {
                    width: 640.0,
                    height: 420.0,
                });
            })
        });
        assert!(!creamui_reactive::in_context_scope());
        let tab = state.current().unwrap();
        tab.session
            .borrow_mut()
            .process("café\r\ncafé".as_bytes())
            .unwrap();
        let key = |key, modifiers| {
            state.key(
                KeyEvent {
                    key,
                    pressed: true,
                    synthetic: false,
                },
                modifiers,
            )
        };
        key(
            Key::Character("F".into()),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
        );
        for ch in "café".chars() {
            key(Key::Character(ch.to_string()), Modifiers::default());
        }
        assert_eq!(tab.session.borrow().search.query, "café");
        assert_eq!(tab.session.borrow().search.matches.len(), 2);
        key(Key::Enter, Modifiers::default());
        assert_eq!(tab.session.borrow().search.current, Some(0));
        key(Key::Left, Modifiers::default());
        key(Key::Delete, Modifiers::default());
        assert_eq!(tab.session.borrow().search.query, "caf");
        key(Key::Backspace, Modifiers::default());
        assert_eq!(tab.session.borrow().search.query, "ca");
        key(
            Key::Character("a".into()),
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        state.paste_text("café");
        assert_eq!(tab.session.borrow().search.query, "café");
        key(Key::Escape, Modifiers::default());
        assert_eq!(tab.session.borrow().control_mode, None);
        drop(effect);
        state.cleanup();
    }

    #[test]
    fn search_shortcut_editor_navigation_and_tab_state_survive_reactive_redraws() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let state = State::new(
                crate::terminal::idle_cwd(),
                crate::terminal::idle_command(),
                tx,
            );
            state.configure(Config::parse("[shortcuts]\nsearch = 'F6'").unwrap());
            state.add();
            let tab = state.current().unwrap();
            let build = state.clone();
            let effect = creamui_reactive::create_effect(move || {
                let _ = build.build(Size {
                    width: 640.0,
                    height: 420.0,
                });
            });
            for i in 0..50 {
                tab.session
                    .borrow_mut()
                    .process(format!("entry {i}\r\n").as_bytes())
                    .unwrap();
            }
            let key = |key, modifiers| {
                state.key(
                    KeyEvent {
                        key,
                        pressed: true,
                        synthetic: false,
                    },
                    modifiers,
                )
            };
            key(Key::F6, Modifiers::default());
            for ch in "entry".chars() {
                key(Key::Character(ch.to_string()), Modifiers::default());
            }
            assert_eq!(tab.session.borrow().search.query, "entry");
            assert_eq!(tab.session.borrow().search.matches.len(), 50);
            key(Key::Enter, Modifiers::default());
            assert_eq!(tab.session.borrow().search.current, Some(0));
            assert!(tab.session.borrow().parser.screen().scrollback() > 0);
            key(Key::Up, Modifiers::default());
            assert_eq!(tab.session.borrow().search.current, Some(49));
            key(
                Key::Character("a".into()),
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            );
            state.paste_text("entry 4");
            assert_eq!(tab.session.borrow().search.query, "entry 4");
            assert_eq!(tab.session.borrow().search.matches.len(), 11);

            // New output updates matches while the editor stays open.
            let id = tab.session.borrow().id;
            state.event(Event::Output(id, b"entry 400\r\n".to_vec()));
            assert_eq!(tab.session.borrow().search.matches.len(), 12);

            for (width, height) in [(640, 420), (360, 280)] {
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                let mut recorder = SceneRecorder::new();
                recorder.begin(width, height, 1.0, CLEAR, theme.colors);
                let scene = Renderer::new().render(state.build(size), size, &mut recorder);
                if let Some(dir) = std::env::var_os("CTERM_SNAPSHOT_DIR") {
                    let mut raster = Rasterizer::new(width, height);
                    raster.render(&recorder.finish(), &Damage::Full);
                    raster
                        .pixmap()
                        .save_png(PathBuf::from(dir).join(format!("search-{width}.png")))
                        .unwrap();
                }
                assert!(scene
                    .hit_test(Point {
                        x: width as f32 / 2.0 + ((width as f32 - 16.0).min(560.0)) / 2.0 - 24.0,
                        y: height as f32 - 30.0,
                    })
                    .is_some());
            }
            state.dispatch(Action::NewTab);
            assert_eq!(state.current().unwrap().session.borrow().control_mode, None);
            state.dispatch(Action::PreviousTab);
            assert_eq!(
                state.current().unwrap().session.borrow().control_mode,
                Some(Mode::Search)
            );
            key(Key::Escape, Modifiers::default());
            assert_eq!(tab.session.borrow().control_mode, None);
            assert_eq!(tab.session.borrow().search.query, "entry 4");
            drop(effect);
            state.cleanup();
        });
    }

    #[test]
    fn terminal_input_and_scrolling_can_trigger_an_immediate_reactive_redraw() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let state = State::new(
                crate::terminal::idle_cwd(),
                crate::terminal::idle_command(),
                tx,
            );
            state.add();
            let renders = Rc::new(std::cell::Cell::new(0));
            let render_state = state.clone();
            let count = renders.clone();
            let effect = creamui_reactive::create_effect(move || {
                if let Some(tab) = render_state.current() {
                    tab.selection.get();
                }
                let _tree = render_state.build(Size {
                    width: 800.0,
                    height: 600.0,
                });
                count.set(count.get() + 1);
            });
            state.key(
                KeyEvent {
                    key: Key::Character("x".into()),
                    pressed: true,
                    synthetic: false,
                },
                Modifiers::default(),
            );
            state.key(
                KeyEvent {
                    key: Key::Enter,
                    pressed: true,
                    synthetic: false,
                },
                Modifiers::default(),
            );
            state.paste_text("pasted text\n");
            state.dispatch(Action::ScrollUp);
            let tab = state.current().unwrap();
            let screen = TerminalScreen {
                session: tab.session.clone(),
                selection: tab.selection.clone(),
                revision: state.revision.clone(),
                metrics: state.metrics.peek(),
                ctrl_held: state.ctrl_held.clone(),
                open_target: None,
                focused: false,
                on_focus: None,
                ligatures: true,
                background_opacity: 100,
            };
            creamui_core::Widget::on_scroll(&screen).unwrap()(-40.0);
            let id = tab.session.borrow().id;
            state.event(Event::Output(id, b"output".to_vec()));
            assert!(
                renders.get() > 3,
                "input should actually redraw the subscribed UI"
            );
            drop(effect);
            state.cleanup();
        });
    }

    #[test]
    fn clicking_terminal_content_can_redraw_synchronously() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
            creamui_reactive::provide_context(Signal::new(WindowDecorations::default()));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let state = State::new(
                crate::terminal::idle_cwd(),
                crate::terminal::idle_command(),
                tx,
            );
            state.add();

            let renders = Rc::new(std::cell::Cell::new(0));
            let render_state = state.clone();
            let count = renders.clone();
            let effect = creamui_reactive::create_effect(move || {
                let _tree = render_state.build(Size {
                    width: 800.0,
                    height: 600.0,
                });
                count.set(count.get() + 1);
            });

            let tab = state.current().unwrap();
            let terminal = state.pane_view(
                &tab.panes,
                tab.active_pane.peek(),
                false,
                state.metrics.peek(),
            );
            let drag_start = terminal.on_drag_start().unwrap();
            drag_start(
                Point { x: 10.0, y: 10.0 },
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
            );

            assert_eq!(
                renders.get(),
                2,
                "the click redraws after `tabs` is released"
            );
            drop(effect);
            state.cleanup();
        });
    }

    #[test]
    fn multiline_paste_requires_confirmation_and_can_be_disabled() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        let tab = state.current().unwrap();
        state.paste_text("first\nsecond");
        assert_eq!(tab.session.borrow().control_mode, Some(Mode::PasteConfirm));
        assert_eq!(
            tab.session.borrow().pending_paste.as_deref(),
            Some("first\nsecond")
        );
        state.key(
            KeyEvent {
                key: Key::Enter,
                pressed: true,
                synthetic: false,
            },
            Modifiers::default(),
        );
        assert_eq!(tab.session.borrow().control_mode, None);
        assert!(tab.session.borrow().pending_paste.is_none());
        state.paste_text("single line");
        assert_eq!(tab.session.borrow().control_mode, None);
        let mut config = state.config.peek();
        config.confirm_multiline_paste = false;
        state.configure(config);
        state.paste_text("without\nconfirmation");
        assert_eq!(tab.session.borrow().control_mode, None);
        state.cleanup();
    }

    #[test]
    fn command_palette_filters_and_invokes_the_first_matching_action() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        state.activate_mode(Mode::Palette);
        state.edit_palette(|palette| palette.query = "new tab".into());
        assert_eq!(state.palette_actions(), vec![Action::NewTab]);
        state.key(
            KeyEvent {
                key: Key::Enter,
                pressed: true,
                synthetic: false,
            },
            Modifiers::default(),
        );
        assert_eq!(state.tabs.peek().len(), 2);
        assert_eq!(state.current().unwrap().session.borrow().control_mode, None);
        state.cleanup();
    }

    #[test]
    fn tab_names_can_be_renamed_and_restored_to_the_automatic_title() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        let tab = state.current().unwrap();
        tab.session
            .borrow_mut()
            .process(b"\x1b]2;automatic\x07")
            .unwrap();
        state.dispatch(Action::RenameTab);
        assert_eq!(tab.session.borrow().control_mode, Some(Mode::RenameTab));
        state.edit_palette(|palette| palette.query = "Deploy logs".into());
        state.rename_tab();
        assert_eq!(State::tab_title(&tab), "Deploy logs");
        state.dispatch(Action::RenameTab);
        state.edit_palette(|palette| palette.query.clear());
        state.rename_tab();
        assert_eq!(State::tab_title(&tab), "automatic");
        state.cleanup();
    }

    #[test]
    fn sessions_restore_tabs_directories_and_manual_titles() {
        let root = std::env::temp_dir().join(format!(
            "cterm-app-session-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let file = root.join("session.toml");
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(crate::terminal::idle_cwd(), Vec::new(), tx);
        *state.session_file.borrow_mut() = Some(file.clone());
        state.add();
        state.current().unwrap().session.borrow_mut().manual_title = Some("First".into());
        state.add();
        state.current().unwrap().session.borrow_mut().manual_title = Some("Second".into());
        state.active.set(0);
        state.cleanup();

        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let restored = State::new(crate::terminal::idle_cwd(), Vec::new(), tx);
        *restored.session_file.borrow_mut() = Some(file.clone());
        let did_restore = restored.restore_session();
        assert!(did_restore);
        assert_eq!(restored.tabs.peek().len(), 2);
        assert_eq!(restored.active.peek(), 0);
        assert_eq!(State::tab_title(&restored.tabs.peek()[0]), "First");
        assert_eq!(State::tab_title(&restored.tabs.peek()[1]), "Second");
        restored.cleanup();

        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let explicit = State::new(crate::terminal::idle_cwd(), vec!["echo".into()], tx);
        *explicit.session_file.borrow_mut() = Some(file.clone());
        assert!(!explicit.restore_session());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn terminal_output_can_be_saved_from_its_control() {
        let path = std::env::temp_dir().join(format!(
            "cterm-output-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ));
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        let tab = state.current().unwrap();
        tab.session
            .borrow_mut()
            .process(b"saved terminal output")
            .unwrap();
        state.dispatch(Action::SaveOutput);
        assert_eq!(tab.session.borrow().control_mode, Some(Mode::SaveOutput));
        state.edit_palette(|palette| palette.query = path.to_string_lossy().into_owned());
        state.save_output();
        assert_eq!(tab.session.borrow().control_mode, None);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("saved terminal output"));
        std::fs::remove_file(path).unwrap();
        state.cleanup();
    }

    #[test]
    fn single_session_has_centered_title_and_actions_opposite_window_controls() {
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let decorations = Signal::new(WindowDecorations::default());
                creamui_reactive::provide_context(decorations.clone());
                let (tx, _rx) = std::sync::mpsc::sync_channel(128);
                let state = State::new(
                    crate::terminal::idle_cwd(),
                    crate::terminal::idle_command(),
                    tx,
                );
                state.add();
                state
                    .current()
                    .unwrap()
                    .session
                    .borrow_mut()
                    .process(b"\x1b]2;~/Projects\x07")
                    .unwrap();
                for (name, x, menu) in [("right", 872, 25.0), ("left", 8, 939.0)] {
                    decorations.set(WindowDecorations {
                        mode: WindowDecorationMode::Hybrid,
                        controls: CompositorControls {
                            x,
                            y: 8,
                            width: 112,
                            height: 28,
                        },
                        ..Default::default()
                    });
                    let size = Size {
                        width: 1000.0,
                        height: 680.0,
                    };
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(1000, 680, 1.0, CLEAR, theme.colors);
                    let scene = Renderer::new().render(state.build(size), size, &mut recorder);
                    assert_eq!(scene.focus_hit_test(Point { x: menu, y: 22.0 }), Some(0));
                    assert!(scene.focus_id_at(1).is_some());
                    assert!(
                        scene.focus_id_at(2).is_none(),
                        "a single session has no tab or close-tab button"
                    );
                    assert!(scene.hit_test(Point { x: 500.0, y: 22.0 }).is_none());
                    if let Some(dir) = std::env::var_os("CTERM_SNAPSHOT_DIR") {
                        let mut raster = Rasterizer::new(1000, 680);
                        raster.render(&recorder.finish(), &Damage::Full);
                        raster
                            .pixmap()
                            .save_png(
                                PathBuf::from(dir).join(format!("{appearance}-single-{name}.png")),
                            )
                            .unwrap();
                    }
                }
                state.cleanup();
            });
        }
    }
    #[test]
    fn custom_shortcuts_reload_without_replacing_sessions_and_indices_select_tabs() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        state.add();
        let id = state.current().unwrap().session.borrow().id;
        let mut config = Config::default();
        config
            .shortcuts
            .insert("new_tab".into(), "Ctrl+Alt+T".into());
        state.event(Event::Configuration(Ok(config)));
        assert_eq!(state.current().unwrap().session.borrow().id, id);
        let event = |key| KeyEvent {
            key,
            pressed: true,
            synthetic: false,
        };
        state.key(
            event(Key::Character("T".into())),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(state.tabs.peek().len(), 2);
        state.key(
            event(Key::Character("T".into())),
            Modifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(state.tabs.peek().len(), 3);
        state.key(
            event(Key::Character("1".into())),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(state.active.peek(), 0);
        state.key(
            event(Key::Tab),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(state.active.peek(), 2);
        state.key(
            event(Key::Tab),
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        assert_eq!(state.active.peek(), 0);
        state.event(Event::Configuration(Err("invalid config".into())));
        assert_eq!(state.error.peek(), Some("invalid config".into()));
        state.key(
            event(Key::Character("T".into())),
            Modifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(state.tabs.peek().len(), 4);
        state.cleanup();
    }

    #[test]
    fn tab_strip_reserves_compositor_controls_and_renders_terminal() {
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let decorations = Signal::new(WindowDecorations::default());
                creamui_reactive::provide_context(decorations.clone());
                let (tx, _rx) = std::sync::mpsc::sync_channel(128);
                let state = State::new(
                    crate::terminal::idle_cwd(),
                    crate::terminal::idle_command(),
                    tx,
                );
                state.add();
                state.current().unwrap().session.borrow_mut().process(b"\x1b]2;~/Projects\x07\x1b[32msammy\x1b[0m \x1b[34m~/Projects\x1b[0m $ cargo build\r\n   \x1b[32mCompiling\x1b[0m cterm v0.1.0\r\n    \x1b[32mFinished\x1b[0m dev profile in 0.42s\r\n\r\n$ ").unwrap();
                state.add();
                state.add();
                state.add();
                state.active.set(0);
                for (width, height) in [(1000, 680), (480, 420)] {
                    for (name, mode, x) in [
                        ("right", WindowDecorationMode::Hybrid, width - 128),
                        ("left", WindowDecorationMode::Hybrid, 8),
                        ("client", WindowDecorationMode::Client, 0),
                        ("pending", WindowDecorationMode::Pending, 0),
                    ] {
                        decorations.set(WindowDecorations {
                            mode,
                            controls: CompositorControls {
                                x,
                                y: 12,
                                width: 112,
                                height: 28,
                            },
                            ..Default::default()
                        });
                        let size = Size {
                            width: width as f32,
                            height: height as f32,
                        };
                        let mut recorder = SceneRecorder::new();
                        recorder.begin(width as u32, height as u32, 1.0, CLEAR, theme.colors);
                        let scene = Renderer::new().render(state.build(size), size, &mut recorder);
                        if mode == WindowDecorationMode::Hybrid {
                            for px in (x..x + 112).step_by(8) {
                                assert!(
                                    scene
                                        .hit_test(Point {
                                            x: px as f32,
                                            y: 26.0
                                        })
                                        .is_none(),
                                    "{name}/{width}/{px}"
                                );
                            }
                        }
                        let list = recorder.finish();
                        if let Some(dir) = std::env::var_os("CTERM_SNAPSHOT_DIR") {
                            let mut raster = Rasterizer::new(width as u32, height as u32);
                            raster.render(&list, &Damage::Full);
                            raster
                                .pixmap()
                                .save_png(
                                    PathBuf::from(dir)
                                        .join(format!("{appearance}-{name}-{width}.png")),
                                )
                                .unwrap();
                        }
                    }
                }
                state.cleanup();
            });
        }
    }
    #[test]
    fn closing_tabs_keeps_the_active_session_and_shortcuts_switch_tabs() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let state = State::new(
            crate::terminal::idle_cwd(),
            crate::terminal::idle_command(),
            tx,
        );
        state.add();
        state.add();
        state.add();
        let id = state.current().unwrap().session.borrow().id;
        state.close(0);
        assert_eq!(state.current().unwrap().session.borrow().id, id);
        state.key(
            KeyEvent {
                key: Key::Tab,
                pressed: true,
                synthetic: false,
            },
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        assert_eq!(state.active.peek(), 0);
        state.close(1);
        assert_eq!(state.active.peek(), 0);
        state.cleanup();
    }
}
