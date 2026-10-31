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
        self.edit(Tab::reload);
    }
}
