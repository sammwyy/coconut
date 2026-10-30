use super::*;

impl State {
    pub fn open(&self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        let existing = {
            let workspace = self.workspace.borrow();
            workspace
                .tabs
                .iter()
                .position(|(_, tab)| tab.borrow().document.path.as_ref() == Some(&path))
        };
        if let Some(index) = existing {
            self.activate(index);
            return;
        }
        self.busy.update(|busy| *busy += 1);
        self.message.set(format!("Opening {}…", path.display()));
        let state = self.clone();
        self.app.spawn_background(
            move || Document::open(path),
            move |result| {
                state.busy.update(|busy| *busy -= 1);
                match result {
                    Ok(document) => {
                        state.add(document, true);
                        state.message.set("Ready".into());
                    }
                    Err(error) => state.error(error),
                }
            },
        );
    }
    pub(super) fn choose_open(&self) {
        let state = self.clone();
        self.app.spawn_background(
            || {
                rfd::FileDialog::new()
                    .set_title("Open files — CEdit")
                    .pick_files()
            },
            move |paths| {
                if let Some(paths) = paths {
                    for path in paths {
                        state.open(path);
                    }
                }
            },
        );
    }
    pub(super) fn save(&self, id: u64, save_as: bool, close: bool) {
        let Some(tab) = self.tab(id) else {
            return;
        };
        let path = tab.borrow().document.path.clone();
        if save_as || path.is_none() {
            let name = path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled.txt".into());
            let state = self.clone();
            self.app.spawn_background(
                move || {
                    rfd::FileDialog::new()
                        .set_title("Save as — CEdit")
                        .set_file_name(name)
                        .save_file()
                },
                move |path| {
                    if let Some(path) = path {
                        state.write(id, path, close);
                    } else {
                        state.quitting.set(false);
                    }
                },
            );
        } else if let Some(path) = path {
            self.write(id, path, close);
        }
    }
    pub(super) fn write(&self, id: u64, path: PathBuf, close: bool) {
        let Some(tab) = self.tab(id) else {
            return;
        };
        let (snapshot, stamp) = {
            let editor = tab.borrow();
            (
                editor.document.snapshot(),
                if editor.document.path.as_ref() == Some(&path) {
                    editor.document.stamp.clone()
                } else {
                    None
                },
            )
        };
        let version = snapshot.revision;
        let target = path.clone();
        let state = self.clone();
        self.busy.update(|n| *n += 1);
        self.message.set("Saving…".into());
        self.app.spawn_background(
            move || document::save(&snapshot, &target, stamp.as_ref()),
            move |result| {
                state.busy.update(|n| *n -= 1);
                match result {
                    Ok(stamp) => {
                        if let Some(tab) = state.tab(id) {
                            let mut editor = tab.borrow_mut();
                            editor.document.path = Some(path);
                            editor.document.stamp = Some(stamp);
                            editor.document.saved_revision = version;
                        }
                        state.message.set("Saved".into());
                        state.refresh();
                        if close {
                            state.close_tab(id);
                        }
                    }
                    Err(error) => {
                        state.quitting.set(false);
                        state.error(error);
                    }
                }
            },
        );
    }
    pub(super) fn remove(&self, id: u64) {
        {
            let mut workspace = self.workspace.borrow_mut();
            let Some(index) = workspace.tabs.iter().position(|(tab, _)| *tab == id) else {
                return;
            };
            workspace.tabs.remove(index);
            if workspace.tabs.is_empty() {
                let next = workspace.next_id;
                workspace.next_id += 1;
                workspace.tabs.push((
                    next,
                    Rc::new(RefCell::new(Editor::new(Document::default()))),
                ));
                workspace.active = 0;
            } else if index < workspace.active {
                workspace.active -= 1;
            } else {
                workspace.active = workspace.active.min(workspace.tabs.len() - 1);
            }
        }
        self.search(self.query.peek());
        self.refresh();
        if self.quitting.get() {
            self.continue_quit();
        }
    }
    pub fn close_tab(&self, id: u64) {
        if self.busy.peek() > 0 || self.confirming.peek().is_some() {
            return;
        }
        let Some(tab) = self.tab(id) else {
            return;
        };
        if !tab.borrow().document.dirty() {
            self.remove(id);
            return;
        }
        let index = self
            .workspace
            .borrow()
            .tabs
            .iter()
            .position(|(tab, _)| *tab == id)
            .unwrap();
        self.activate(index);
        self.confirming.set(Some(id));
        self.editor_focused.set(false);
        self.refresh();
    }
    pub(super) fn resolve_close(&self, choice: &str) {
        let Some(id) = self.confirming.peek() else {
            return;
        };
        self.confirming.set(None);
        match choice {
            "save" => self.save(id, false, true),
            "discard" => self.remove(id),
            _ => self.quitting.set(false),
        }
        self.editor_focused.set(true);
        self.refresh();
    }

    pub fn request_quit(&self) -> bool {
        if self.busy.peek() > 0 || self.confirming.peek().is_some() {
            self.message
                .set("Finish the current operation before closing.".into());
            return false;
        }
        if self
            .workspace
            .borrow()
            .tabs
            .iter()
            .all(|(_, tab)| !tab.borrow().document.dirty())
        {
            return true;
        }
        self.quitting.set(true);
        self.continue_quit();
        false
    }
    pub(super) fn continue_quit(&self) {
        let dirty = {
            self.workspace
                .borrow()
                .tabs
                .iter()
                .find(|(_, tab)| tab.borrow().document.dirty())
                .map(|(id, _)| *id)
        };
        if let Some(id) = dirty {
            self.close_tab(id);
        } else if let Some(window) = self.window.borrow().as_ref() {
            window.close();
        }
    }
}
