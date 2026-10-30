mod actions;
mod chrome;
mod files;
mod keyboard;
mod search;
mod view;

use crate::{
    commands::{self, Command},
    screen::EditorScreen,
    ui::*,
};
use crate::{
    document::{self, Document, Language, MarkdownMode, ViewMode},
    editor::Editor,
    search::{self as search_engine, SearchOptions},
};
use creamui_core::{
    layout::{self, Dimension, Style},
    BoxedWidget, Size, Styled,
};
use creamui_reactive::Signal;
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers},
    AppHandle, WindowHandle,
};
use creamui_theme::use_theme;
use creamui_widgets::{AppTitlebar, Popover, RawView, Text, TextController, TextInput};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

pub type Tab = Rc<RefCell<Editor>>;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    None,
    Find,
    Replace,
    GoTo,
    Palette,
    Help,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Popup {
    None,
    Menu,
    Renderer,
    Mode,
    Encoding,
}
struct Workspace {
    tabs: Vec<(u64, Tab)>,
    active: usize,
    next_id: u64,
}
use crate::screen::Highlights as Matches;
#[derive(Clone)]
struct SearchRequest {
    generation: u64,
    tab: u64,
    revision: u64,
    text: ropey::Rope,
    query: String,
    options: SearchOptions,
}
#[derive(Clone)]
pub struct State {
    pub app: AppHandle,
    workspace: Rc<RefCell<Workspace>>,
    pub revision: Signal<u64>,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    last_title: Rc<RefCell<String>>,
    pub editor_focused: Rc<Cell<bool>>,
    panel: Signal<Panel>,
    popup: Signal<Popup>,
    popup_index: Signal<usize>,
    palette: creamui_widgets::CommandPaletteController,
    menu_anchor: Rc<Cell<creamui_core::Rect>>,
    render_anchor: Rc<Cell<creamui_core::Rect>>,
    mode_anchor: Rc<Cell<creamui_core::Rect>>,
    encoding_anchor: Rc<Cell<creamui_core::Rect>>,

    pub query: TextController,
    replacement: TextController,
    prompt: TextController,
    options: Signal<SearchOptions>,
    pub matches: Rc<RefCell<Option<Matches>>>,
    search_generation: Rc<Cell<u64>>,
    search_request: Rc<RefCell<Option<SearchRequest>>>,
    search_running: Rc<Cell<bool>>,
    markdown_running: Rc<Cell<bool>>,
    pub message: Signal<String>,
    clipboard: Rc<RefCell<Option<arboard::Clipboard>>>,
    busy: Signal<usize>,
    confirming: Signal<Option<u64>>,
    quitting: Rc<Cell<bool>>,
    pub digits: Rc<RefCell<String>>,
}
impl State {
    pub fn new(app: AppHandle) -> Self {
        let prompt = TextController::default();
        let palette = creamui_widgets::CommandPaletteController::with_query(prompt.clone());
        let state = Self {
            app,
            workspace: Rc::new(RefCell::new(Workspace {
                tabs: vec![(1, Rc::new(RefCell::new(Editor::new(Document::default()))))],
                active: 0,
                next_id: 2,
            })),
            revision: Signal::new(0),
            window: Rc::new(RefCell::new(None)),
            last_title: Rc::new(RefCell::new(String::new())),
            editor_focused: Rc::new(Cell::new(true)),
            panel: Signal::new(Panel::None),
            popup: Signal::new(Popup::None),
            popup_index: Signal::new(0),
            palette,
            menu_anchor: Rc::new(Cell::new(creamui_core::Rect::default())),
            render_anchor: Rc::new(Cell::new(creamui_core::Rect::default())),
            mode_anchor: Rc::new(Cell::new(creamui_core::Rect::default())),
            encoding_anchor: Rc::new(Cell::new(creamui_core::Rect::default())),

            query: TextController::default(),
            replacement: TextController::default(),
            prompt,
            options: Signal::new(SearchOptions::default()),
            matches: Rc::new(RefCell::new(None)),
            search_generation: Rc::new(Cell::new(0)),
            search_request: Rc::new(RefCell::new(None)),
            search_running: Rc::new(Cell::new(false)),
            markdown_running: Rc::new(Cell::new(false)),
            message: Signal::new("Ready".into()),
            clipboard: Rc::new(RefCell::new(None)),
            busy: Signal::new(0),
            confirming: Signal::new(None),
            quitting: Rc::new(Cell::new(false)),
            digits: Rc::new(RefCell::new(String::new())),
        };
        let search = state.clone();
        state.query.on_change(move |_, next| {
            search.search(next.to_owned());
            Some(next.to_owned())
        });
        state
    }
    pub fn current(&self) -> (u64, Tab) {
        let workspace = self.workspace.borrow();
        workspace.tabs[workspace.active].clone()
    }
    fn tab(&self, id: u64) -> Option<Tab> {
        self.workspace
            .borrow()
            .tabs
            .iter()
            .find(|(tab, _)| *tab == id)
            .map(|(_, editor)| editor.clone())
    }
    pub fn refresh(&self) {
        self.index_markdown();
        let title = self.current().1.borrow().document.title();
        if let Some(window) = self.window.borrow().as_ref() {
            if *self.last_title.borrow() != title {
                window.set_title(&title);
                *self.last_title.borrow_mut() = title;
            }
        }
        self.revision
            .update(|revision| *revision = revision.wrapping_add(1));
    }
    fn index_markdown(&self) {
        if self.markdown_running.get() {
            return;
        }
        let (id, tab) = self.current();
        let editor = tab.borrow();
        if editor.document.markdown_mode() == MarkdownMode::Raw
            || editor.document.view.bytes()
            || editor.document.markdown_revision == Some(editor.document.revision)
        {
            return;
        }
        let text = editor.document.text.clone();
        let revision = editor.document.revision;
        drop(editor);
        self.markdown_running.set(true);
        let state = self.clone();
        self.app.spawn_background(
            move || crate::markdown::BlockIndex::build(&text),
            move |index| {
                state.markdown_running.set(false);
                if let Some(tab) = state.tab(id) {
                    let mut editor = tab.borrow_mut();
                    if editor.document.revision == revision {
                        editor.document.markdown_index = std::sync::Arc::new(index);
                        editor.document.markdown_revision = Some(revision);
                    }
                }
                state.refresh();
            },
        );
    }
    fn focus_editor(&self) {
        self.editor_focused.set(self.panel.peek() == Panel::None);
        if let Some(window) = self.window.borrow().as_ref() {
            window.focus_text_input();
        }
    }
    fn error(&self, error: String) {
        eprintln!("cedit: {error}");
        self.message.set(error);
        self.refresh();
    }
    pub fn edit(&self, edit: impl FnOnce(&mut Editor) -> Result<(), String>) {
        if self.busy.peek() > 0 {
            return;
        }
        let (_, tab) = self.current();
        let previous = tab.borrow().document.revision;
        let result = edit(&mut tab.borrow_mut());
        if let Err(error) = result {
            self.error(error);
        }
        if tab.borrow().document.revision != previous {
            self.search(self.query.peek());
        }
        self.refresh();
    }
    fn add(&self, document: Document, replace_empty: bool) {
        {
            let mut workspace = self.workspace.borrow_mut();
            let id = workspace.next_id;
            workspace.next_id += 1;
            if replace_empty
                && workspace.tabs.len() == 1
                && workspace.tabs[0].1.borrow().document.path.is_none()
                && workspace.tabs[0].1.borrow().document.text.len_chars() == 0
                && !workspace.tabs[0].1.borrow().document.dirty()
            {
                workspace.tabs.clear();
            }
            workspace
                .tabs
                .push((id, Rc::new(RefCell::new(Editor::new(document)))));
            workspace.active = workspace.tabs.len() - 1;
        }
        self.search(self.query.peek());
        self.refresh();
        self.focus_editor();
    }
    fn activate(&self, index: usize) {
        {
            let mut workspace = self.workspace.borrow_mut();
            workspace.active = index.min(workspace.tabs.len() - 1);
        }
        self.digits.borrow_mut().clear();
        self.search(self.query.peek());
        self.refresh();
        self.focus_editor();
    }
}
