use super::*;
use coconut_file_menu::{FileMenuOptions, FileMenuTarget};
use creamui_widgets::ContextMenuItem;

impl DesktopState {
    pub(super) fn open_context_menu(&self, point: Point, target: FileMenuTarget) {
        let Some(window) = self.desktop_window.borrow().clone() else {
            return;
        };
        let open = target.clone();
        let target_entry = match &open {
            FileMenuTarget::Item { path, .. } => self
                .entries
                .peek()
                .iter()
                .find(|entry| &entry.path == path)
                .cloned(),
            _ => None,
        };
        let refresh = self.clone();
        let mut options = FileMenuOptions::new(
            move |path, _| {
                if let Some(entry) = &target_entry {
                    open_target(entry.target.clone());
                } else {
                    open_path(path);
                }
            },
            move || refresh.reload(),
        );
        if matches!(target, FileMenuTarget::Background(_)) {
            let sort = self.clone();
            let arrange = self.clone();
            let refresh = self.clone();
            let settings = self.clone();
            options = options.properties(false).extras(vec![
                ContextMenuItem::submenu(
                    "sort",
                    "Sort icons",
                    vec![
                        ContextMenuItem::new("sort-name", "Name", move || {
                            creamui_reactive::batch(|| {
                                sort.positions.set(HashMap::new());
                                sort.selected.set(None);
                                sort.entries.update(|entries| {
                                    entries.sort_by_key(|entry| entry.label.to_lowercase())
                                });
                            });
                        }),
                        ContextMenuItem::new("arrange", "Align to grid", move || {
                            arrange.positions.set(HashMap::new())
                        }),
                    ],
                ),
                ContextMenuItem::new("refresh", "Refresh", move || refresh.reload()),
                ContextMenuItem::new("desktop-settings", "Desktop settings…", move || {
                    let Some(window) = settings.desktop_window.borrow().clone() else {
                        return;
                    };
                    let controller = settings.file_menu.clone();
                    controller.perform(
                        &window,
                        || {},
                        || {
                            let mut child = coconut_core::spawn_detached(
                                Command::new("coconut-settings").arg("/desktop"),
                            )
                            .map_err(|error| error.to_string())?;
                            std::thread::spawn(move || {
                                if let Err(error) = child.wait() {
                                    eprintln!("desktop: could not reap settings: {error}");
                                }
                            });
                            Ok(())
                        },
                    );
                }),
            ]);
        }
        self.file_menu.open(&window, point, target, options);
    }
    fn reload(&self) {
        let Some(window) = self.desktop_window.borrow().clone() else {
            return;
        };
        let theme = self.icon_theme.borrow().clone();
        self.start_loading(&window.app(), theme);
    }
}
