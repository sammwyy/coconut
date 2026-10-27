use crate::{config::ExplorerConfig, filesystem as files};
use creamui_widgets::{ScrollController, TextController};
use std::{path::PathBuf, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Location {
    Home,
    Directory(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryView {
    List,
    Grid,
    Columns,
}
impl Location {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Home => "Home".into(),
            Self::Directory(path) => files::name(path),
        }
    }
    pub(crate) fn address(&self) -> String {
        match self {
            Self::Home => "Home".into(),
            Self::Directory(path) => path.display().to_string(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Tab {
    pub(crate) history: Vec<Location>,
    pub(crate) cursor: usize,
    pub(crate) entries: Rc<Vec<files::Entry>>,
    pub(crate) error: Option<String>,
    pub(crate) selected: Option<PathBuf>,
    pub(crate) view: DirectoryView,
    /// Directories displayed from left to right in Columns Mode.
    pub(crate) column_paths: Vec<PathBuf>,
    pub(crate) column_scrolls: Vec<ScrollController>,
    pub(crate) sort: SortColumn,
    pub(crate) sort_ascending: bool,
    pub(crate) show_hidden: bool,
    pub(crate) address: TextController,
    pub(crate) search: TextController,
    pub(crate) scroll: ScrollController,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortColumn {
    Name,
    Size,
    Kind,
    Modified,
    Created,
    Permissions,
}
impl Tab {
    pub(crate) fn new(location: Location, explorer: &ExplorerConfig) -> Self {
        let column_paths = column_paths(&location);
        let mut tab = Self {
            address: TextController::new(location.address()),
            history: vec![location],
            cursor: 0,
            entries: Rc::new(Vec::new()),
            error: None,
            selected: None,
            view: if explorer.default_grid_view {
                DirectoryView::Grid
            } else {
                DirectoryView::List
            },
            column_scrolls: column_paths
                .iter()
                .map(|_| ScrollController::new(0.0))
                .collect(),
            column_paths,
            sort: SortColumn::Name,
            sort_ascending: true,
            show_hidden: explorer.show_hidden,
            search: TextController::new(""),
            scroll: ScrollController::new(0.0),
        };
        tab.reload();
        tab
    }
    pub(crate) fn location(&self) -> &Location {
        &self.history[self.cursor]
    }
    pub(crate) fn reload(&mut self) {
        self.error = None;
        self.entries = Rc::new(match self.location() {
            Location::Home => Vec::new(),
            Location::Directory(path) => match files::entries(path, self.show_hidden) {
                Ok(entries) => entries,
                Err(error) => {
                    self.error = Some(format!("Could not read this folder: {error}"));
                    Vec::new()
                }
            },
        });
        if self
            .selected
            .as_ref()
            .is_some_and(|p| !self.entries.iter().any(|e| &e.path == p))
        {
            self.selected = None;
        }
    }
    pub(crate) fn moved(&mut self) {
        self.selected = None;
        self.search.set_value("");
        self.address.set_value(self.location().address());
        self.scroll.set(0.0);
        self.sync_columns();
        self.reload();
    }
    pub(crate) fn navigate(&mut self, location: Location) {
        if self.location() != &location {
            self.history.truncate(self.cursor + 1);
            self.history.push(location);
            self.cursor += 1;
        }
        self.moved();
    }
    pub(crate) fn travel(&mut self, forward: bool) {
        if forward && self.cursor + 1 < self.history.len() {
            self.cursor += 1;
        } else if !forward && self.cursor > 0 {
            self.cursor -= 1;
        } else {
            return;
        }
        self.moved();
    }

    /// Opens a directory from a Columns Mode column. Unlike regular
    /// navigation, retain the branch to its parent so only the columns to the
    /// right are replaced.
    pub(crate) fn navigate_column(&mut self, path: PathBuf) {
        let location = Location::Directory(path.clone());
        if self.location() != &location {
            self.history.truncate(self.cursor + 1);
            self.history.push(location);
            self.cursor += 1;
        }
        self.selected = None;
        self.search.set_value("");
        self.address.set_value(self.location().address());
        self.scroll.set(0.0);
        if let Some(parent_index) = path
            .parent()
            .and_then(|parent| self.column_paths.iter().position(|column| column == parent))
        {
            self.column_paths.truncate(parent_index + 1);
            self.column_paths.push(path);
            self.column_scrolls.truncate(parent_index + 1);
            self.column_scrolls.push(ScrollController::new(0.0));
        } else {
            self.sync_columns();
        }
        self.reload();
    }

    fn sync_columns(&mut self) {
        self.column_paths = column_paths(self.location());
        self.column_scrolls = self
            .column_paths
            .iter()
            .map(|_| ScrollController::new(0.0))
            .collect();
    }
}

fn column_paths(location: &Location) -> Vec<PathBuf> {
    let Location::Directory(path) = location else {
        return Vec::new();
    };
    let home = files::home();
    let root = if path.starts_with(&home) {
        home
    } else {
        path.clone()
    };
    let mut paths = vec![root.clone()];
    let Ok(relative) = path.strip_prefix(&root) else {
        return paths;
    };
    let mut current = root;
    for component in relative.components() {
        current.push(component.as_os_str());
        paths.push(current.clone());
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_branch_tracks_the_active_directory_and_replaces_its_tail() {
        let fixture = tempfile::tempdir().unwrap();
        let parent = fixture.path().join("parent");
        let first = parent.join("first");
        let second = parent.join("second");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();

        let mut tab = Tab::new(
            Location::Directory(parent.clone()),
            &ExplorerConfig::default(),
        );
        tab.navigate_column(first.clone());
        assert_eq!(tab.column_paths, vec![parent.clone(), first]);
        assert_eq!(tab.column_scrolls.len(), 2);

        tab.navigate_column(second.clone());
        assert_eq!(tab.column_paths, vec![parent, second]);
        assert_eq!(tab.column_scrolls.len(), 2);
    }
}
