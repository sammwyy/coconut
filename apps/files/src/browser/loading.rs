use super::*;

struct DirectoryRequest {
    identity: Rc<()>,
    revision: u64,
    paths: Vec<PathBuf>,
    hidden: bool,
}

type DirectoryResults = Vec<Result<Vec<files::Entry>, String>>;

impl BrowserState {
    pub(crate) fn load_directories(&self) {
        if self.directory_job_running.get() {
            return;
        }
        if self
            .window
            .borrow()
            .as_ref()
            .is_some_and(WindowHandle::is_closed)
        {
            return;
        }
        let tabs = self.tabs.peek();
        let active = self.active.peek();
        let Some(tab) = tabs
            .get(active)
            .filter(|tab| tab.loading)
            .or_else(|| tabs.iter().find(|tab| tab.loading))
        else {
            return;
        };
        let request = DirectoryRequest {
            identity: tab.identity.clone(),
            revision: tab.load_revision,
            paths: if tab.view == DirectoryView::Columns {
                tab.column_paths.clone()
            } else {
                match tab.location() {
                    Location::Directory(path) => vec![path.clone()],
                    Location::Home => return,
                }
            },
            hidden: tab.show_hidden,
        };
        let paths = request.paths.clone();
        let hidden = request.hidden;
        let work = move || {
            paths
                .iter()
                .map(|path| {
                    files::entries(path, hidden).map_err(|error| {
                        eprintln!("cfiles: could not read {}: {error}", path.display());
                        format!("Could not read this folder: {error}")
                    })
                })
                .collect()
        };
        let window = self.window.borrow().clone();
        if let Some(window) = window {
            self.directory_job_running.set(true);
            let state = self.clone();
            window.app().spawn_background(work, move |results| {
                state.directory_job_running.set(false);
                state.finish_directory_load(&request, results);
                state.load_directories();
            });
        } else {
            self.finish_directory_load(&request, work());
            self.load_directories();
        }
    }

    fn finish_directory_load(&self, request: &DirectoryRequest, results: DirectoryResults) {
        self.tabs.update(|tabs| {
            let Some(tab) = tabs.iter_mut().find(|tab| {
                Rc::ptr_eq(&tab.identity, &request.identity)
                    && tab.load_revision == request.revision
                    && tab.show_hidden == request.hidden
            }) else {
                return;
            };
            tab.loading = false;
            let results: Vec<_> = results
                .into_iter()
                .map(|result| result.map(Rc::new))
                .collect();
            let active = results
                .last()
                .expect("directory load has an active path")
                .clone();
            if tab.view == DirectoryView::Columns {
                tab.column_entries = Rc::new(results);
            }
            match active {
                Ok(entries) => {
                    if tab
                        .selected
                        .as_ref()
                        .is_some_and(|path| !entries.iter().any(|entry| &entry.path == path))
                    {
                        tab.selected = None;
                    }
                    tab.entries = entries;
                    tab.error = None;
                }
                Err(error) => tab.error = Some(error),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_coalesces_directory_loads_while_a_worker_is_running() {
        let fixture = tempfile::tempdir().unwrap();
        let first = fixture.path().join("first");
        let last = fixture.path().join("last");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&last).unwrap();
        fs::write(last.join("latest.txt"), "text").unwrap();
        let browser = BrowserState::new(WindowRequest::manager(None));
        browser.directory_job_running.set(true);
        browser.navigate(Location::Directory(first.clone()));
        let first_revision = browser.tab().load_revision;
        browser.navigate(Location::Directory(last.clone()));
        assert!(browser.tab().loading);
        assert!(browser.tab().entries.is_empty());
        assert!(browser.tab().load_revision > first_revision);
        fs::remove_dir(first).unwrap();
        browser.directory_job_running.set(false);
        browser.load_directories();
        assert!(!browser.tab().loading);
        assert_eq!(browser.tab().entries[0].path, last.join("latest.txt"));
        assert!(browser.tab().error.is_none());
    }

    #[test]
    fn directory_completion_tracks_tabs_after_removal_and_ignores_old_revisions() {
        let browser = BrowserState::new(WindowRequest::manager(None));
        browser.add_tab();
        let tab = browser.tab();
        let request = DirectoryRequest {
            identity: tab.identity.clone(),
            revision: tab.load_revision,
            paths: vec![PathBuf::from("/")],
            hidden: tab.show_hidden,
        };
        browser.close_tab(0);
        browser.finish_directory_load(&request, vec![Err("current error".into())]);
        assert_eq!(browser.tab().error.as_deref(), Some("current error"));
        browser.edit(Tab::reload);
        browser.finish_directory_load(&request, vec![Err("obsolete error".into())]);
        assert!(browser.tab().error.is_none());
    }
}
