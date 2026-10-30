use super::*;

impl FileMenuController {
    pub(super) fn items(
        &self,
        window: &WindowHandle,
        target: FileMenuTarget,
        options: FileMenuOptions,
        snapshot: Snapshot,
    ) -> Vec<ContextMenuItem> {
        let mut items = Vec::new();
        let directory = target.directory();
        if let FileMenuTarget::Item { path, directory } = &target {
            let open = options.open.clone();
            let path = path.clone();
            let directory = *directory;
            items.push(ContextMenuItem::new(
                "open",
                options.open_label,
                move || open(path.clone(), directory),
            ));
            let applications = snapshot
                .applications
                .into_iter()
                .enumerate()
                .map(|(index, entry)| {
                    let controller = self.clone();
                    let window = window.clone();
                    let path = target.path().to_path_buf();
                    let label = entry.name(None).unwrap_or("Application").to_owned();
                    ContextMenuItem::new(format!("open-with-{index}"), label, move || {
                        let path = path.clone();
                        let entry = entry.clone();
                        controller.perform(
                            &window,
                            || {},
                            move || operations::launch_application(&entry, &path),
                        );
                    })
                })
                .collect();
            items.push(ContextMenuItem::submenu(
                "open-with",
                "Open with",
                applications,
            ));
            items.push(ContextMenuItem::separator());
            let controller = self.clone();
            let copy_window = window.clone();
            let path = target.path().to_path_buf();
            items.push(ContextMenuItem::new("copy", "Copy", move || {
                let path = path.clone();
                controller.perform(&copy_window, || {}, move || operations::copy_path(&path));
            }));
            if options.editable {
                items.push(self.form_item(
                    "rename",
                    "Rename…",
                    FormKind::Rename,
                    target.path().to_path_buf(),
                    window,
                    options.refresh.clone(),
                ));
            }
        } else if options.editable {
            items.push(ContextMenuItem::submenu(
                "new",
                "New",
                vec![
                    self.form_item(
                        "new-folder",
                        "Folder…",
                        FormKind::NewFolder,
                        directory.clone(),
                        window,
                        options.refresh.clone(),
                    ),
                    self.form_item(
                        "new-file",
                        "Empty file…",
                        FormKind::NewFile,
                        directory.clone(),
                        window,
                        options.refresh.clone(),
                    ),
                ],
            ));
        }
        if options.editable
            && (matches!(target, FileMenuTarget::Background(_))
                || matches!(
                    target,
                    FileMenuTarget::Item {
                        directory: true,
                        ..
                    }
                ))
        {
            let controller = self.clone();
            let window = window.clone();
            let destination = directory.clone();
            let refresh = options.refresh.clone();
            let sources = snapshot.copied;
            items.push(ContextMenuItem::new("paste", "Paste", move || {
                let destination = destination.clone(); let sources = sources.clone(); let refresh = refresh.clone();
                controller.perform(&window, move || refresh(), move || operations::paste(&sources, &destination).map_err(|error| format!("Could not complete copy: {error}. Files already copied remain in the destination.")));
            }).enabled(snapshot.can_paste));
        }
        items.push(ContextMenuItem::separator());
        let controller = self.clone();
        let terminal_window = window.clone();
        let terminal_directory = directory.clone();
        items.push(ContextMenuItem::new(
            "terminal",
            "Open terminal here",
            move || {
                let directory = terminal_directory.clone();
                controller.perform(
                    &terminal_window,
                    || {},
                    move || operations::open_terminal(&directory),
                );
            },
        ));
        let controller = self.clone();
        let explorer_window = window.clone();
        items.push(ContextMenuItem::new(
            "explorer",
            "Open in File Explorer",
            move || {
                let directory = directory.clone();
                controller.perform(
                    &explorer_window,
                    || {},
                    move || open_in_explorer(&directory),
                );
            },
        ));
        if !options.extras.is_empty() {
            items.push(ContextMenuItem::separator());
            items.extend(options.extras);
        }
        if !options.show_properties {
            return items;
        }
        items.push(ContextMenuItem::separator());
        let controller = self.clone();
        let path = target.path().to_path_buf();
        let window = window.clone();
        items.push(ContextMenuItem::new(
            "properties",
            "Properties",
            move || {
                let path = path.clone();
                let controller = controller.clone();
                let done = controller.clone();
                let generation = controller.generation.get();
                window.app().spawn_background(
                    move || properties(&path),
                    move |result| {
                        if done.generation.get() != generation {
                            return;
                        }
                        match result {
                            Ok(body) => done.information("Properties", body),
                            Err(error) => done.information("Could not read properties", error),
                        }
                    },
                );
            },
        ));
        items
    }
    fn form_item(
        &self,
        id: &str,
        label: &str,
        kind: FormKind,
        path: PathBuf,
        window: &WindowHandle,
        refresh: Rc<dyn Fn()>,
    ) -> ContextMenuItem {
        let controller = self.clone();
        let window = window.clone();
        ContextMenuItem::new(id, label, move || {
            let name = if matches!(kind, FormKind::Rename) {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            } else {
                String::new()
            };
            batch(|| {
                controller.name.set_value(name);
                controller.error.set(None);
                controller.panel.set(Some(Panel::Form(Form {
                    kind: kind.clone(),
                    path: path.clone(),
                    window: window.clone(),
                    refresh: refresh.clone(),
                })));
            });
            window.focus_text_input();
        })
    }
}
