use super::*;
use coconut_file_menu::{FileMenuOptions, FileMenuTarget};
use creamui_core::Point;
use creamui_widgets::ContextMenuItem;

impl BrowserState {
    pub(crate) fn open_context_menu(&self, point: Point, target: FileMenuTarget) {
        batch(|| {
            *self.last_click.borrow_mut() = None;
            self.edit(|tab| {
                tab.selected = match &target {
                    FileMenuTarget::Item { path, .. } => Some(path.clone()),
                    FileMenuTarget::Background(_) => None,
                }
            });
        });
        let window = self.window.borrow().clone();
        let Some(window) = window else {
            return;
        };
        let options = self.context_menu_options(&target);
        self.file_menu.open(&window, point, target, options);
    }
    pub(super) fn context_menu_options(&self, target: &FileMenuTarget) -> FileMenuOptions {
        let open = self.clone();
        let refresh = self.clone();
        let mut options = FileMenuOptions::new(
            move |path, directory| {
                if open.context.is_save() && !directory {
                    batch(|| {
                        open.name.set_value(files::name(&path));
                        open.edit(|tab| tab.selected = Some(path));
                        open.overwrite.set(None);
                        open.message.set(None);
                    });
                } else {
                    open.activate(path, directory);
                }
            },
            move || refresh.refresh_directory(),
        )
        .editable(self.context.is_manager());
        if self.context.is_save()
            && matches!(
                target,
                FileMenuTarget::Item {
                    directory: false,
                    ..
                }
            )
        {
            options = options.open_label("Use file name");
        }
        if matches!(target, FileMenuTarget::Background(_))
            && self.tab().location() == &Location::Home
        {
            let home = self.clone();
            let settings = self.clone();
            return options.editable(false).extras(vec![
                ContextMenuItem::new("open-home", "Open home folder", move || {
                    home.navigate(Location::Directory(files::home()))
                }),
                ContextMenuItem::new("settings", "File Explorer settings…", move || {
                    settings.open_settings()
                }),
            ]);
        }
        if matches!(target, FileMenuTarget::Background(_)) {
            let sort_items = [
                ("name", "Name", crate::navigation::SortColumn::Name),
                ("size", "Size", crate::navigation::SortColumn::Size),
                ("kind", "Type", crate::navigation::SortColumn::Kind),
                (
                    "modified",
                    "Modified",
                    crate::navigation::SortColumn::Modified,
                ),
            ]
            .into_iter()
            .map(|(id, label, sort)| {
                let state = self.clone();
                ContextMenuItem::new(id, label, move || {
                    state.edit(|tab| {
                        tab.sort = sort;
                        tab.sort_ascending = true;
                    })
                })
            })
            .collect();
            let hidden = self.clone();
            let refresh = self.clone();
            let settings = self.clone();
            options = options.extras(vec![
                ContextMenuItem::submenu("sort", "Sort by", sort_items),
                ContextMenuItem::new(
                    "hidden",
                    if self.tab().show_hidden {
                        "Hide hidden files"
                    } else {
                        "Show hidden files"
                    },
                    move || {
                        hidden.edit(|tab| tab.show_hidden = !tab.show_hidden);
                        hidden.refresh_directory();
                    },
                ),
                ContextMenuItem::new("refresh", "Refresh", move || refresh.refresh_directory()),
                ContextMenuItem::new("settings", "File Explorer settings…", move || {
                    settings.open_settings()
                }),
            ]);
        }
        options
    }
    pub(crate) fn refresh_directory(&self) {
        let Some(window) = self.window.borrow().clone() else {
            return;
        };
        let tab = self.tab();
        let Location::Directory(path) = tab.location().clone() else {
            return;
        };
        let index = self.active.peek();
        let identity = tab.entries.clone();
        let hidden = tab.show_hidden;
        let inspect_path = path.clone();
        let state = self.clone();
        window.app().spawn_background(
            move || files::entries(&inspect_path, hidden),
            move |result| {
                batch(|| {
                    state.tabs.update(|tabs| {
                        let Some(tab) = tabs.get_mut(index).filter(|tab| {
                            tab.location() == &Location::Directory(path)
                                && tab.show_hidden == hidden
                                && Rc::ptr_eq(&tab.entries, &identity)
                        }) else {
                            return;
                        };
                        match result {
                            Ok(entries) => {
                                if tab.selected.as_ref().is_some_and(|path| {
                                    !entries.iter().any(|entry| &entry.path == path)
                                }) {
                                    tab.selected = None;
                                }
                                tab.entries = Rc::new(entries);
                                tab.error = None;
                            }
                            Err(error) => {
                                tab.error = Some(format!("Could not read this folder: {error}"))
                            }
                        }
                    })
                });
            },
        );
    }
}
