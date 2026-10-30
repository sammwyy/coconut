mod commands;
mod routes;
#[cfg(test)]
mod tests;
mod views;

use crate::{
    conversion::{self, Category},
    engine::{format_number, Angle, Context},
    entry::Entry,
    navigation::{Mode, DRAWER_WIDTH},
    session::{Memory, Session},
};
use creamui_core::{
    layout::{self, AlignItems, Dimension, FlexDirection, LengthPercentage, Style},
    BoxedWidget, Size, Styled,
};
use creamui_reactive::{batch, Signal};
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers},
    WindowHandle,
};
use creamui_router::{Router, RouterOutlet, RouterProvider};
use creamui_theme::use_theme;
use creamui_widgets::{
    AppTitlebar, Button, ButtonSize, ButtonState, ButtonVariant, CommandPalette,
    CommandPaletteController, CommandPaletteItem, Popover, RawButton, RawView, ScrollController,
    ScrollView, SegmentedControl, Select, SelectController, Text,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone)]
pub enum Action {
    Insert(String),
    Function(String),
    Suffix(String),
    Evaluate,
    Clear,
    Backspace,
    Sign,
    Recall,
    Memory(Memory),
    Navigate(Mode),
    Angle(Angle),
    History,
    ClearHistory,
    Copy,
    Paste,
    Commands,
    Dismiss,
    Drawer,
    SwapUnits,
    Unit { source: bool, index: usize },
}
#[derive(Clone, Copy)]
struct Geometry {
    height: f32,
    gap: f32,
    key_height: f32,
}

struct Preview {
    expression: String,
    mode: Mode,
    context: Context,
    units: (usize, usize),
    result: Result<f64, String>,
}

#[derive(Clone)]
pub struct State {
    pub session: Rc<RefCell<Session>>,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    pub router: Router,
    calculator: Rc<RefCell<Entry>>,
    conversions: Rc<RefCell<HashMap<Category, Entry>>>,
    units: Rc<HashMap<Category, (SelectController, SelectController)>>,
    revision: Signal<u64>,
    preview: Rc<RefCell<Option<Preview>>>,
    message: Signal<String>,
    drawer_open: Signal<bool>,
    history_open: Signal<bool>,
    palette_open: Signal<bool>,
    palette: CommandPaletteController,
    scroll: ScrollController,
    scientific_scroll: ScrollController,
    viewport: Rc<Cell<Size>>,
    pending_size: Rc<Cell<Option<(u32, u32)>>>,
    history_scroll: ScrollController,
    drawer_scroll: ScrollController,
    clipboard: Rc<RefCell<Option<arboard::Clipboard>>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            session: Rc::new(RefCell::new(Session::default())),
            window: Rc::new(RefCell::new(None)),
            router: routes::create(),
            calculator: Rc::new(RefCell::new(Entry::default())),
            conversions: Rc::new(RefCell::new(
                Category::ALL
                    .into_iter()
                    .map(|category| (category, Entry::default()))
                    .collect(),
            )),
            units: Rc::new(
                Category::ALL
                    .into_iter()
                    .map(|category| {
                        let (from, to) = category.defaults();
                        (
                            category,
                            (SelectController::new(from), SelectController::new(to)),
                        )
                    })
                    .collect(),
            ),
            revision: Signal::new(0),
            preview: Rc::new(RefCell::new(None)),
            message: Signal::new(String::new()),
            drawer_open: Signal::new(false),
            history_open: Signal::new(false),
            palette_open: Signal::new(false),
            palette: CommandPaletteController::new(),
            scroll: ScrollController::default(),
            scientific_scroll: ScrollController::default(),
            history_scroll: ScrollController::default(),
            drawer_scroll: ScrollController::default(),
            pending_size: Rc::new(Cell::new(None)),
            viewport: Rc::new(Cell::new(Size {
                width: 360.0,
                height: 520.0,
            })),
            clipboard: Rc::new(RefCell::new(None)),
        }
    }
}
impl State {
    pub fn mode(&self) -> Mode {
        Mode::from_path(&self.router.location().pathname)
    }
    pub fn entry(&self) -> Entry {
        if let Mode::Converter(category) = self.mode() {
            self.conversions.borrow()[&category].clone()
        } else {
            self.calculator.borrow().clone()
        }
    }
    fn edit_entry(
        &self,
        edit: impl FnOnce(&mut Entry) -> Result<(), String>,
    ) -> Result<(), String> {
        let result = if let Mode::Converter(category) = self.mode() {
            edit(self.conversions.borrow_mut().get_mut(&category).unwrap())
        } else {
            edit(&mut self.calculator.borrow_mut())
        };
        if result.is_ok() {
            self.message.set(String::new());
        }
        result
    }
    pub fn set_expression(&self, text: &str) {
        batch(|| {
            if let Err(error) = self.edit_entry(|entry| entry.set(text)) {
                self.message.set(error);
            }
            self.refresh();
        });
    }
    fn refresh(&self) {
        self.revision.update(|value| *value = value.wrapping_add(1));
    }
    pub fn minimum_size(&self) -> (u32, u32) {
        self.mode().sizing().minimum
    }
    pub fn apply_window_size(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_min_size(Some(self.minimum_size()));
        }
    }
    fn resize(&self, old_mode: Mode) {
        let before = old_mode.sizing();
        let after = self.mode().sizing();
        let viewport = self.viewport.get();
        let current = self.pending_size.get().unwrap_or((
            viewport.width.round() as u32,
            viewport.height.round() as u32,
        ));
        let next = before.transition(after, current);
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_min_size(Some(after.minimum));
            if next != current {
                self.pending_size.set(Some(next));
                window.resize(next.0, next.1);
            }
        }
    }
    fn selected_units(&self, category: Category) -> (usize, usize) {
        let (from, to) = &self.units[&category];
        (from.selected(), to.selected())
    }
    fn result(&self) -> Result<f64, String> {
        let entry = self.entry();
        let mode = self.mode();
        let context = self.session.borrow().context;
        let units = if let Mode::Converter(category) = mode {
            self.selected_units(category)
        } else {
            (0, 0)
        };
        let mut cached = self.preview.borrow_mut();
        if cached.as_ref().is_none_or(|cached| {
            cached.expression != entry.text
                || cached.mode != mode
                || cached.context != context
                || cached.units != units
        }) {
            let result = crate::engine::evaluate(entry.expression(), context).and_then(|value| {
                if let Mode::Converter(category) = mode {
                    conversion::convert(value, category, units.0, units.1)
                } else {
                    Ok(value)
                }
            });
            *cached = Some(Preview {
                expression: entry.text,
                mode,
                context,
                units,
                result,
            });
        }
        cached.as_ref().unwrap().result.clone()
    }
    fn clipboard(&self, value: Option<String>) -> Result<String, String> {
        let mut clipboard = self.clipboard.borrow_mut();
        if clipboard.is_none() {
            *clipboard = Some(
                arboard::Clipboard::new()
                    .map_err(|error| format!("Clipboard unavailable: {error}"))?,
            );
        }
        let clipboard = clipboard.as_mut().unwrap();
        if let Some(value) = value {
            clipboard
                .set_text(value)
                .map_err(|error| format!("Could not copy result: {error}"))?;
            Ok(String::new())
        } else {
            clipboard
                .get_text()
                .map_err(|error| format!("Could not paste: {error}"))
        }
    }
    pub fn run(&self, action: Action) {
        batch(|| {
            if !matches!(action, Action::Commands | Action::Dismiss) {
                self.palette_open.set(false);
            }
            if !matches!(action, Action::Drawer | Action::Dismiss) {
                self.drawer_open.set(false);
            }
            let mathematical = matches!(action, Action::Evaluate);
            let result = (|| -> Result<(), String> {
                match action {
                    Action::Insert(text) => self.edit_entry(|entry| entry.insert(&text))?,
                    Action::Function(name) => self.edit_entry(|entry| {
                        if entry.text.is_empty() {
                            entry.insert(&format!("{name}("))
                        } else {
                            entry.wrap(&format!("{name}("), ")")
                        }
                    })?,
                    Action::Suffix(suffix) => {
                        self.edit_entry(|entry| entry.wrap("(", &format!("){suffix}")))?
                    }
                    Action::Sign => self.edit_entry(|entry| entry.wrap("-(", ")"))?,
                    Action::Clear => self.edit_entry(|entry| {
                        entry.clear();
                        Ok(())
                    })?,
                    Action::Backspace => self.edit_entry(|entry| {
                        entry.backspace();
                        Ok(())
                    })?,
                    Action::Evaluate => {
                        let expression = self.entry().expression().to_owned();
                        let result = self.result()?;
                        if let Mode::Converter(category) = self.mode() {
                            let (from, to) = self.selected_units(category);
                            let units = category.units();
                            self.session.borrow_mut().record(
                                format!("{expression} {}", units[from].symbol),
                                result,
                                format!(
                                    "{} · {} → {}",
                                    category.name(),
                                    units[from].symbol,
                                    units[to].symbol
                                ),
                            );
                            self.edit_entry(|entry| {
                                entry.fresh = true;
                                Ok(())
                            })?;
                        } else {
                            self.session.borrow_mut().calculate(&expression)?;
                            self.edit_entry(|entry| {
                                entry.result(result);
                                Ok(())
                            })?;
                        }
                    }
                    Action::Memory(operation) => self
                        .session
                        .borrow_mut()
                        .store(self.entry().expression(), operation)?,
                    Action::Recall => {
                        let value = self.session.borrow().memory;
                        if let Some(value) = value {
                            self.edit_entry(|entry| entry.insert(&format!("({value})")))?;
                        }
                    }
                    Action::Navigate(mode) => {
                        let old = self.mode();
                        self.router
                            .replace(&mode.path())
                            .map_err(|error| error.to_string())?;
                        self.message.set(String::new());
                        self.close_units();
                        self.drawer_open.set(false);
                        self.resize(old);
                    }
                    Action::Drawer => {
                        self.drawer_open.set(!self.drawer_open.peek());
                    }
                    Action::Angle(angle) => self.session.borrow_mut().context.angle = angle,
                    Action::History => {
                        self.history_open.set(!self.history_open.peek());
                        self.history_scroll.set(0.0);
                    }
                    Action::ClearHistory => self.session.borrow_mut().history.clear(),
                    Action::Copy => {
                        self.clipboard(Some(self.result()?.to_string()))?;
                        self.message.set("Result copied".into());
                    }
                    Action::Paste => {
                        let text = self.clipboard(None)?;
                        let candidate = text.trim();
                        self.session.borrow().preview(candidate)?;
                        self.edit_entry(|entry| entry.set(candidate))?;
                    }
                    Action::Commands => {
                        self.palette.reset();
                        self.palette_open.set(true);
                    }
                    Action::Dismiss => {
                        if self.palette_open.peek() {
                            self.palette_open.set(false);
                        } else if self.units_open() {
                            self.close_units();
                        } else if self.drawer_open.peek() {
                            self.drawer_open.set(false);
                        } else {
                            self.history_open.set(false);
                        }
                    }
                    Action::SwapUnits => {
                        if let Mode::Converter(category) = self.mode() {
                            let value = self.result();
                            let (from, to) = &self.units[&category];
                            let previous = from.peek_selected();
                            from.select(to.peek_selected());
                            to.select(previous);
                            if let Ok(value) = value {
                                self.edit_entry(|entry| {
                                    entry.result(value);
                                    entry.previous = None;
                                    Ok(())
                                })?;
                            }
                            self.message.set(String::new());
                        }
                    }
                    Action::Unit { source, index } => {
                        if let Mode::Converter(category) = self.mode() {
                            if index >= category.units().len() {
                                return Err("Unknown unit".into());
                            }
                            let (from, to) = &self.units[&category];
                            if source {
                                from.select(index)
                            } else {
                                to.select(index)
                            };
                            self.message.set(String::new());
                        }
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("ccalc: {error}");
                if !mathematical {
                    self.message.set(error);
                }
            }
            self.refresh();
        });
        if self.palette_open.peek() {
            if let Some(window) = self.window.borrow().as_ref() {
                window.focus_text_input();
            }
        }
    }
    fn close_units(&self) {
        for (from, to) in self.units.values() {
            from.set_open(false);
            to.set_open(false);
        }
    }
    fn units_open(&self) -> bool {
        self.units
            .values()
            .any(|(from, to)| from.peek_open() || to.peek_open())
    }
}
