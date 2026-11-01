mod drag;
mod presentation;
mod strip;
mod tools;

#[cfg(test)]
mod tests;

use coconut_core::{BarEdit, BarKind, DockConfig};
use coconut_plugin_kit::bar_editor::{BarEditor, Target};
use creamui_core::{Key, KeyInput, Rect};
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_widgets::{CommandPaletteController, ScrollController};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

pub use presentation::build;

#[derive(Clone)]
struct Draft {
    original: DockConfig,
    current: DockConfig,
    group_ids: Vec<u64>,
    item_ids: Vec<Vec<u64>>,
    next_id: u64,
}

impl Draft {
    fn new(bar: &DockConfig) -> Self {
        let mut draft = Self {
            original: bar.clone(),
            current: bar.clone(),
            group_ids: Vec::new(),
            item_ids: Vec::new(),
            next_id: 0,
        };
        draft.reset_ids();
        draft
    }

    fn allocate(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn reset_ids(&mut self) {
        self.group_ids.clear();
        self.item_ids.clear();
        for group in 0..self.current.sections.len() {
            let id = self.allocate();
            self.group_ids.push(id);
            let ids = (0..self.current.sections[group].islands.len())
                .map(|_| self.allocate())
                .collect();
            self.item_ids.push(ids);
        }
    }

    fn edit(&mut self, edit: &BarEdit) -> Result<(), String> {
        edit.apply(&mut self.current)?;
        match *edit {
            BarEdit::AddGroup => {
                let id = self.allocate();
                self.group_ids.push(id);
                self.item_ids.push(Vec::new());
            }
            BarEdit::RemoveGroup(group) => {
                self.group_ids.remove(group);
                self.item_ids.remove(group);
            }
            BarEdit::MoveGroup { from, to } => {
                let id = self.group_ids.remove(from);
                self.group_ids.insert(to, id);
                let ids = self.item_ids.remove(from);
                self.item_ids.insert(to, ids);
            }
            BarEdit::AddItem { group, at, .. } => {
                let id = self.allocate();
                self.item_ids[group].insert(at, id);
            }
            BarEdit::RemoveItem { group, item } => {
                self.item_ids[group].remove(item);
            }
            BarEdit::MoveItem {
                group,
                item,
                to_group,
                at,
            } => {
                let id = self.item_ids[group].remove(item);
                self.item_ids[to_group].insert(
                    if group == to_group && at > item {
                        at - 1
                    } else {
                        at
                    },
                    id,
                );
            }
            BarEdit::Restore(_) => self.reset_ids(),
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct State {
    draft: Signal<Option<Draft>>,
    pub persistence: BarEditor,
    tools: Rc<RefCell<Option<WindowHandle>>>,
    drag: Signal<Option<drag::Drag>>,
    bounds: Rc<RefCell<HashMap<Target, Rect>>>,
    viewport: Rc<Cell<Rect>>,
    catalog: Signal<Option<usize>>,
    search: CommandPaletteController,
    tab: Signal<usize>,
    scroll: ScrollController,
    appearance: coconut_plugin_kit::bar_editor::controls::AppearanceControls,
    requested: Rc<Cell<Option<(u32, u32)>>>,
    strips: Rc<RefCell<HashMap<usize, strip::Scroll>>>,
    clips: Rc<RefCell<HashMap<usize, Rect>>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            draft: Signal::new(None),
            persistence: BarEditor::default(),
            tools: Default::default(),
            drag: Signal::new(None),
            bounds: Default::default(),
            viewport: Rc::new(Cell::new(Rect::default())),
            catalog: Signal::new(None),
            search: CommandPaletteController::new(),
            tab: Signal::new(0),
            scroll: ScrollController::new(0.0),
            appearance: Default::default(),
            requested: Default::default(),
            strips: Default::default(),
            clips: Default::default(),
        }
    }
}

impl State {
    pub fn with_editor(editor: &BarEditor) -> Self {
        Self {
            persistence: editor.draft_editor(),
            ..Self::default()
        }
    }
    pub fn resize_bar(
        &self,
        window: &Rc<RefCell<Option<WindowHandle>>>,
        viewport: creamui_core::Size,
        bar: &DockConfig,
    ) {
        if let (Some(bar_window), Some(tools)) =
            (window.borrow().as_ref(), self.tools.borrow().as_ref())
        {
            if tools.theme() != bar_window.theme() {
                tools.set_theme(bar_window.theme());
            }
        }
        let cross = if self.is_active() {
            bar.thickness.max(if bar.position.is_vertical() {
                64.0
            } else {
                44.0
            }) + 32.0
        } else {
            bar.thickness.max(1.0)
        } + bar.margin.max(0.0);
        let size = if bar.position.is_vertical() {
            (cross.ceil() as u32, viewport.height as u32)
        } else {
            (viewport.width as u32, cross.ceil() as u32)
        };
        if self.requested.replace(Some(size)) != Some(size) {
            if let Some(window) = window.borrow().as_ref() {
                window.resize(size.0, size.1);
            }
        }
    }
    pub fn is_active(&self) -> bool {
        self.draft.get().is_some()
    }

    pub fn preview(&self, bar: &DockConfig) -> DockConfig {
        self.draft
            .get()
            .map_or_else(|| bar.clone(), |draft| draft.current)
    }

    pub fn begin(&self, bar: &DockConfig) {
        if !self.is_active() {
            self.persistence.dismiss_transients();
            self.persistence.status.set(String::new());
            self.draft.set(Some(Draft::new(bar)));
            self.scroll.set(0.0);
            self.tab.set(0);
            self.strips.borrow_mut().clear();
        }
    }

    pub fn discard(&self) {
        if self.persistence.busy.peek() {
            return;
        }
        self.drag.set(None);
        self.catalog.set(None);
        self.appearance.dismiss();
        self.persistence.dismiss_transients();
        self.draft.set(None);
        let window = self.tools.borrow_mut().take();
        if let Some(window) = window {
            window.close();
        }
    }

    fn replace(&self, current: DockConfig) {
        if self.persistence.busy.peek() {
            return;
        }
        self.draft.update(|draft| {
            if let Some(draft) = draft {
                draft.current = current;
                if draft.current.sections.len() != draft.group_ids.len()
                    || draft
                        .current
                        .sections
                        .iter()
                        .zip(&draft.item_ids)
                        .any(|(section, ids)| section.islands.len() != ids.len())
                {
                    draft.reset_ids();
                }
            }
        });
    }

    pub(super) fn apply(&self, edit: BarEdit) {
        if self.persistence.busy.peek() {
            return;
        }
        let Some(mut draft) = self.draft.peek() else {
            return;
        };
        if let BarEdit::RemoveGroup(group) = edit {
            if draft
                .current
                .sections
                .get(group)
                .is_some_and(|section| !section.islands.is_empty())
            {
                self.persistence
                    .report_error("Empty the section before removing it.".into());
                return;
            }
        }
        match draft.edit(&edit) {
            Ok(()) => self.draft.set(Some(draft)),
            Err(error) => self.persistence.report_error(error),
        }
    }

    pub(super) fn save(&self, parent: &WindowHandle, kind: BarKind) {
        let Some(draft) = self.draft.peek() else {
            return;
        };
        let state = self.clone();
        self.persistence.save_draft(
            parent,
            kind,
            draft.original,
            draft.current,
            Rc::new(move |_| state.discard()),
        );
    }

    pub(super) fn open_catalog(&self, group: usize) {
        if self.persistence.busy.peek() {
            return;
        }
        self.search.reset();
        self.catalog.set(Some(group));
    }

    pub fn handle_key(&self, input: KeyInput, pressed: bool, available: &[String]) -> bool {
        if !pressed || !self.is_active() {
            return false;
        }
        if self.drag.peek().is_some() && input.key == Key::Escape {
            self.drag.set(None);
            return true;
        }
        if self.catalog.peek().is_some() {
            let state = self.clone();
            return self
                .search
                .handle_key(input, &self.catalog_items(available), move || {
                    state.catalog.set(None)
                });
        }
        if input.key == Key::Escape {
            if !self.appearance.dismiss_picker() && !self.persistence.handle_key(&input.key, true) {
                self.discard();
            }
            return true;
        }
        self.persistence.handle_key(&input.key, true)
    }

    pub fn focus_lost(&self) {
        self.drag.set(None);
        self.persistence.menu.parent_focus_lost();
    }

    fn item_key(&self, target: Target, input: KeyInput) {
        let Some(draft) = self.draft.peek() else {
            return;
        };
        match target {
            Target::Item { group, item } => {
                if input.key == Key::Delete {
                    self.apply(BarEdit::RemoveItem { group, item });
                }
                if !input.modifiers.alt {
                    return;
                }
                let edit = match input.key {
                    Key::Up if item > 0 => BarEdit::MoveItem {
                        group,
                        item,
                        to_group: group,
                        at: item - 1,
                    },
                    Key::Down if item + 1 < draft.current.sections[group].islands.len() => {
                        BarEdit::MoveItem {
                            group,
                            item,
                            to_group: group,
                            at: item + 2,
                        }
                    }
                    Key::Left if group > 0 => BarEdit::MoveItem {
                        group,
                        item,
                        to_group: group - 1,
                        at: draft.current.sections[group - 1].islands.len(),
                    },
                    Key::Right if group + 1 < draft.current.sections.len() => BarEdit::MoveItem {
                        group,
                        item,
                        to_group: group + 1,
                        at: draft.current.sections[group + 1].islands.len(),
                    },
                    _ => return,
                };
                self.apply(edit);
            }
            Target::Group(group) if input.modifiers.alt => {
                let to = match input.key {
                    Key::Left | Key::Up if group > 0 => group - 1,
                    Key::Right | Key::Down if group + 1 < draft.current.sections.len() => group + 1,
                    _ => return,
                };
                self.apply(BarEdit::MoveGroup { from: group, to });
            }
            _ => {}
        }
    }
}
