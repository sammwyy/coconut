use super::*;
use crate::{portal::PortalRequest, views as view};
use creamui_core::{Point, Rect, Renderer, Size, TitlebarMode, TitlebarState};
use creamui_render::{Damage, Rasterizer, SceneRecorder, WindowDecorationMode};
use creamui_theme::{Color, Theme, ThemeProvider};

#[test]
fn navigation_notifies_live_subscribers_only_after_state_is_consistent() {
    let fixture = tempfile::tempdir().unwrap();
    let directory = Location::Directory(fixture.path().to_path_buf());
    let browser = BrowserState::new(WindowRequest::manager(None));
    let snapshots = Rc::new(RefCell::new(Vec::new()));
    // Unlike static renders, a live window subscribes to both the tab list
    // and the text/scroll controllers stored inside each tab.
    let _effect = creamui_reactive::create_effect({
        let browser = browser.clone();
        let snapshots = snapshots.clone();
        move || {
            browser.message.get();
            browser.overwrite.get();
            let tab = browser.tab();
            snapshots.borrow_mut().push((
                tab.location().clone(),
                tab.address.value(),
                tab.search.value(),
                tab.scroll.offset(),
                browser.active.get(),
                browser.tabs.get().len(),
            ));
        }
    });
    let check = |action: &dyn Fn(), expected: &Location, active: usize, count: usize| {
        snapshots.borrow_mut().clear();
        action();
        assert_eq!(
            *snapshots.borrow(),
            vec![(
                expected.clone(),
                expected.address(),
                String::new(),
                0.0,
                active,
                count
            )]
        );
    };
    check(&|| browser.navigate(directory.clone()), &directory, 0, 1);
    browser.tab().search.set_value("query");
    browser.tab().scroll.set(50.0);
    check(&|| browser.travel(false), &Location::Home, 0, 1);
    check(&|| browser.travel(true), &directory, 0, 1);
    check(&|| browser.add_tab(), &Location::Home, 1, 2);
    check(&|| browser.switch(0), &directory, 0, 2);
    check(&|| browser.close_tab(0), &Location::Home, 0, 1);
    browser
        .tab()
        .address
        .set_value(fixture.path().display().to_string());
    check(&|| browser.submit_address(), &directory, 0, 1);
}

#[test]
fn navigation_history_branches_and_tabs_keep_their_state() {
    let fixture = tempfile::tempdir().unwrap();
    let a = fixture.path().join("A");
    let b = fixture.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let browser = BrowserState::new(WindowRequest::manager(None));
    browser.navigate(Location::Directory(a.clone()));
    browser.navigate(Location::Directory(b.clone()));
    browser.travel(false);
    assert_eq!(browser.tab().location(), &Location::Directory(a.clone()));
    browser.travel(true);
    assert_eq!(browser.tab().location(), &Location::Directory(b.clone()));
    browser.travel(false);
    browser.navigate(Location::Home);
    assert_eq!(browser.tab().cursor + 1, browser.tab().history.len());
    browser.navigate(Location::Directory(a.clone()));
    browser.tab().search.set_value("report");
    browser.edit(|t| t.view = DirectoryView::Grid);
    browser.add_tab();
    browser.navigate(Location::Directory(b));
    browser.switch(0);
    assert_eq!(browser.tab().search.value(), "report");
    assert_eq!(browser.tab().view, DirectoryView::Grid);
    browser.close_tab(1);
    assert_eq!(browser.tab().location(), &Location::Directory(a));
    browser.close_tab(0);
    assert_eq!(browser.tabs.peek().len(), 1);
}

#[test]
fn listing_is_complete_sorted_and_follows_directory_links() {
    let fixture = tempfile::tempdir().unwrap();
    for index in 0..40 {
        fs::write(fixture.path().join(format!("file-{index:02}.txt")), "text").unwrap();
    }
    fs::create_dir(fixture.path().join("Folder")).unwrap();
    std::os::unix::fs::symlink(
        fixture.path().join("Folder"),
        fixture.path().join("Linked folder"),
    )
    .unwrap();
    fs::write(fixture.path().join(".hidden"), "hidden").unwrap();
    let items = files::entries(fixture.path(), false).unwrap();
    assert_eq!(items.len(), 42);
    assert!(items[0].directory);
    assert!(items[1].directory);
    assert_eq!(items[1].name(), "Linked folder");
    assert_eq!(items[2].name(), "file-00.txt");
    let pins = vec![
        fixture.path().join("spaces and # symbols"),
        fixture.path().join("line\nbreak"),
    ];
    let state = fixture.path().join("state/pinned");
    files::write_paths(&state, &pins).unwrap();
    assert_eq!(files::read_paths(&state), Some(pins));
    files::write_paths(&state, &[]).unwrap();
    assert_eq!(files::read_paths(&state), Some(vec![]));
}

#[test]
fn save_validates_names_and_requires_replace_confirmation() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("exists.txt"), "original").unwrap();
    let mut request = WindowRequest::save("Save document".into(), "exists.txt".into());
    request.start_directory = Some(fixture.path().to_path_buf());
    let browser = BrowserState::new(request);
    browser.accept();
    assert!(browser.result.lock().unwrap().is_none());
    assert!(browser.overwrite.peek().is_some());
    assert_eq!(
        fs::read_to_string(fixture.path().join("exists.txt")).unwrap(),
        "original"
    );
    for name in ["", "..", ".", "../escape", "/absolute", "bad\0name"] {
        browser.name.set_value(name);
        browser.accept();
        assert!(browser.result.lock().unwrap().is_none());
    }
    browser.name.set_value("new.txt");
    browser.accept();
    assert_eq!(
        browser
            .result
            .lock()
            .unwrap()
            .as_ref()
            .map(|selection| selection.path.clone()),
        Some(fixture.path().join("new.txt"))
    );
    assert!(
        !fixture.path().join("new.txt").exists(),
        "portal only returns a destination; it does not write the document"
    );
}

#[test]
fn filtered_selections_cannot_be_accepted_and_folders_can_be_chosen() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("report.txt"), "report").unwrap();
    fs::create_dir(fixture.path().join("Documents")).unwrap();
    let mut request = WindowRequest::open("Open".into(), false);
    request.start_directory = Some(fixture.path().to_path_buf());
    let mut browser = BrowserState::new(request);
    browser.edit(|t| t.selected = Some(fixture.path().join("report.txt")));
    browser.tab().search.set_value("no match");
    browser.accept();
    assert!(browser.result.lock().unwrap().is_none());
    browser.tab().search.set_value("");
    browser.context.request.mode = WindowMode::Portal(PortalRequest::new(PortalType::SelectFolder));
    browser.edit(|t| t.selected = Some(fixture.path().join("Documents")));
    browser.accept();
    assert_eq!(
        browser
            .result
            .lock()
            .unwrap()
            .as_ref()
            .map(|selection| selection.path.clone()),
        Some(fixture.path().join("Documents"))
    );
}

#[test]
fn native_layout_and_portal_previews() {
    creamui_reactive::with_context_scope(|| {
        let theme = Theme::light();
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        creamui_reactive::provide_context(creamui_core::WindowDragHandle::new(|| {}));
        let window = crate::test_window::TestWindow::install(TitlebarState {
            mode: TitlebarMode::Overlay,
            overlay: Rect {
                x: 992.0,
                y: 14.0,
                width: 112.0,
                height: 28.0,
            },
            ..Default::default()
        });
        let fixture = tempfile::tempdir().unwrap();
        for name in [
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
        ] {
            fs::create_dir(fixture.path().join(name)).unwrap();
        }
        for name in [
            "Design notes.md",
            "Morning light.jpg",
            "Project proposal.pdf",
            "Playlist.ogg",
            "Readme.txt",
        ] {
            fs::write(fixture.path().join(name), "preview").unwrap();
        }
        let mut browser = BrowserState::new(WindowRequest::manager(None));
        browser.pins.set(
            ["Desktop", "Documents", "Pictures"]
                .iter()
                .map(|n| fixture.path().join(n))
                .collect(),
        );
        browser.places = Rc::new(
            [
                ("Home folder", "home", ""),
                ("Desktop", "desktop", "Desktop"),
                ("Downloads", "download", "Downloads"),
                ("Documents", "file", "Documents"),
                ("Pictures", "image", "Pictures"),
                ("Videos", "video", "Videos"),
                ("Music", "music", "Music"),
            ]
            .iter()
            .map(|(label, icon, path)| files::Place {
                label: (*label).into(),
                icon,
                path: fixture.path().join(path),
            })
            .collect(),
        );
        browser.drives.set(vec![
            files::Drive {
                label: "File system".into(),
                path: PathBuf::from("/"),
                space: Some((330 << 30, 500 << 30)),
            },
            files::Drive {
                label: "Work drive".into(),
                path: PathBuf::from("/media/Work"),
                space: Some((64 << 30, 128 << 30)),
            },
        ]);
        browser.recent.set(
            [
                "Design notes.md",
                "Morning light.jpg",
                "Project proposal.pdf",
                "Playlist.ogg",
                "Readme.txt",
            ]
            .iter()
            .map(|n| fixture.path().join(n))
            .collect(),
        );
        let size = Size {
            width: 1120.0,
            height: 760.0,
        };
        let render = |browser: &BrowserState, size: Size, name: &str| {
            let mut recorder = SceneRecorder::new();
            recorder.begin(
                size.width as u32,
                size.height as u32,
                1.0,
                Color::rgba(0, 0, 0, 0),
                creamui_theme::use_theme().colors,
            );
            let scene = Renderer::new().render(view::build(browser, size), size, &mut recorder);
            let list = recorder.finish();
            if let Ok(directory) = std::env::var("CFILES_SNAPSHOT_DIR") {
                fs::create_dir_all(&directory).unwrap();
                let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                raster.render(&list, &Damage::Full);
                raster
                    .pixmap()
                    .save_png(Path::new(&directory).join(format!("{name}.png")))
                    .unwrap();
            }
            scene
        };
        let scene = render(&browser, size, "home");
        assert!(scene.window_drag_at(Point { x: 365.0, y: 26.0 }).is_some());
        assert!(
            scene.window_drag_at(Point { x: 12.0, y: 700.0 }).is_some(),
            "the empty sidebar drags the window"
        );
        assert!(scene.window_drag_at(Point { x: 600.0, y: 26.0 }).is_none());
        assert!(
            scene.hit_test(Point { x: 1090.0, y: 28.0 }).is_none(),
            "compositor owns hybrid controls"
        );
        scene
            .hit_test(Point { x: 950.0, y: 26.0 })
            .expect("new tab button")();
        assert_eq!(browser.tabs.peek().len(), 2);
        browser.explorer.tab_position = crate::config::TabPosition::Bottom;
        let bottom_tabs = render(&browser, size, "tabs-bottom");
        assert!(
            bottom_tabs.hit_test(Point { x: 350.0, y: 733.0 }).is_some(),
            "bottom tab strip is interactive"
        );
        browser.explorer.tab_position = crate::config::TabPosition::Top;
        browser.navigate(Location::Directory(fixture.path().to_path_buf()));
        let scene = render(&browser, size, "list");
        let address_point = Point { x: 400.0, y: 81.0 };
        scene.hit_test(address_point).expect("breadcrumb address")();
        assert!(browser.address_editing.peek());
        let address_input = render(&browser, size, "address-input");
        assert!(address_input.focus_hit_test(address_point).is_some());
        browser.cancel_address_edit();

        browser.toggle_search();
        let search = render(&browser, size, "search-empty");
        let search_focus = search.focus_hit_test(address_point).expect("search input");
        search.on_focus_change_at(search_focus).unwrap()(false);
        assert!(!browser.search_active.peek());

        browser.toggle_search();
        browser.tab().search.set_value("desk");
        let search = render(&browser, size, "search-with-query");
        let search_focus = search.focus_hit_test(address_point).expect("search input");
        search.on_focus_change_at(search_focus).unwrap()(false);
        assert!(browser.search_active.peek());
        search.on_key_at(search_focus).unwrap()(creamui_core::KeyInput {
            key: creamui_core::Key::Escape,
            modifiers: Default::default(),
        });
        assert!(!browser.search_active.peek());

        scene
            .hit_test(Point { x: 1056.0, y: 81.0 })
            .expect("grid toggle")();
        assert_eq!(browser.tab().view, DirectoryView::Grid);
        let grid = render(&browser, size, "grid");
        grid.hit_test(Point { x: 1092.0, y: 81.0 })
            .expect("columns toggle")();
        assert_eq!(browser.tab().view, DirectoryView::Columns);
        browser.activate(fixture.path().join("Desktop"), true);
        assert_eq!(browser.tab().column_paths.len(), 2);
        render(&browser, size, "columns");
        browser.travel(false);
        browser.context.request.mode = WindowMode::Portal(PortalRequest::save("Untitled.txt"));
        browser.name.set_value("Untitled.txt");
        render(&browser, size, "portal-save");
        browser.context.request.mode = WindowMode::Portal(PortalRequest::new(PortalType::OpenFile));
        browser.context.request.title = "Open a document".into();
        browser.edit(|t| t.view = DirectoryView::List);
        render(&browser, size, "portal-open");
        window.set(TitlebarMode::Client, Rect::default());
        let scene = render(&browser, size, "client-decorations");
        assert!(
            scene.hit_test(Point { x: 1090.0, y: 28.0 }).is_some(),
            "fallback window controls"
        );
        let compact = render(
            &browser,
            Size {
                width: 800.0,
                height: 600.0,
            },
            "compact",
        );
        let scroll = compact
            .scroll_hit_test(Point { x: 600.0, y: 300.0 })
            .expect("directory scroll area");
        compact.on_scroll_at(scroll).unwrap()(250.0);
        assert!(browser.tab().scroll.offset() > 0.0);
        browser.tab().scroll.set(0.0);
        let focus = compact
            .focus_hit_test(Point { x: 300.0, y: 212.0 })
            .expect("first folder");
        compact.on_key_at(focus).unwrap()(creamui_core::KeyInput {
            key: creamui_core::Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(
            browser.tab().location(),
            &Location::Directory(fixture.path().join("Desktop"))
        );
        browser.travel(false);
        for _ in 0..8 {
            browser.add_tab();
        }
        let many = render(
            &browser,
            Size {
                width: 800.0,
                height: 600.0,
            },
            "many-tabs",
        );
        assert!(
            many.hit_test(Point { x: 776.0, y: 28.0 }).is_some(),
            "tabs must not cover close-window control"
        );
        browser.switch(1);
        window.set(
            TitlebarMode::Overlay,
            Rect {
                x: 16.0,
                y: 14.0,
                width: 112.0,
                height: 28.0,
            },
        );
        let left = render(&browser, size, "left-controls");
        assert!(left.hit_test(Point { x: 60.0, y: 28.0 }).is_none());
        assert!(left.window_drag_at(Point { x: 60.0, y: 28.0 }).is_none());
        creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
        render(&browser, size, "dark");
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let mut request = PortalRequest::new(PortalType::OpenFile);
        request.filters = vec![
            crate::portal::FileFilter {
                name: "Images".into(),
                rules: vec![crate::portal::FilterRule::MimeType("image/*".into())],
            },
            crate::portal::FileFilter {
                name: "Text".into(),
                rules: vec![crate::portal::FilterRule::Glob("*.txt".into())],
            },
        ];
        request.accept_label = Some("Attach".into());
        let filtered = BrowserState::new(WindowRequest {
            title: "Choose an attachment".into(),
            start_directory: Some(fixture.path().to_path_buf()),
            mode: WindowMode::Portal(request),
        });
        let entries = view::directory::visible_entries(&filtered, &filtered.tab());
        assert_eq!(
            entries
                .iter()
                .filter(|entry| !entry.directory)
                .map(|entry| entry.name())
                .collect::<Vec<_>>(),
            vec!["Morning light.jpg"]
        );
        render(&filtered, size, "portal-filters");
        filtered.context.filter.set_open(true);
        let popup = render(&filtered, size, "portal-filter-menu");
        assert!(
            popup
                .hit_test(Point {
                    x: 1000.0,
                    y: 200.0
                })
                .is_some(),
            "filter options remain inside the window"
        );
        filtered.context.filter.select(1);
        let entries = view::directory::visible_entries(&filtered, &filtered.tab());
        assert_eq!(
            entries
                .iter()
                .filter(|entry| !entry.directory)
                .map(|entry| entry.name())
                .collect::<Vec<_>>(),
            vec!["Readme.txt"]
        );
    });
}

#[test]
fn browser_draws_one_set_of_client_controls_for_single_and_multiple_tabs() {
    creamui_reactive::with_context_scope(|| {
        let theme = Theme::light();
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let window = crate::test_window::TestWindow::install(TitlebarState::default());
        let mut browser = BrowserState::new(WindowRequest::manager(None));
        browser.explorer = ExplorerConfig::default();
        let size = Size {
            width: 1120.0,
            height: 760.0,
        };
        for tab_count in [1, 2] {
            if tab_count == 2 {
                browser.add_tab();
            }
            for position in [
                crate::config::TabPosition::Top,
                crate::config::TabPosition::Bottom,
            ] {
                browser.explorer.tab_position = position;
                let mut client_focus_count = 0;
                for mode in [
                    WindowDecorationMode::Client,
                    WindowDecorationMode::Hybrid,
                    WindowDecorationMode::Server,
                    WindowDecorationMode::Pending,
                    WindowDecorationMode::None,
                ] {
                    window.set(
                        match mode {
                            WindowDecorationMode::Client => TitlebarMode::Client,
                            WindowDecorationMode::Hybrid => TitlebarMode::Overlay,
                            WindowDecorationMode::Server => TitlebarMode::Server,
                            WindowDecorationMode::Pending => TitlebarMode::Pending,
                            WindowDecorationMode::None => TitlebarMode::None,
                        },
                        Rect {
                            x: 992.0,
                            y: 14.0,
                            width: 112.0,
                            height: 28.0,
                        },
                    );
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
                    let scene =
                        Renderer::new().render(view::build(&browser, size), size, &mut recorder);
                    let focus_count = (0..)
                        .take_while(|index| scene.focus_id_at(*index).is_some())
                        .count();
                    if mode == WindowDecorationMode::Client {
                        client_focus_count = focus_count;
                        for offset in [28.0, 64.0, 100.0] {
                            assert!(
                                scene
                                    .hit_test(Point {
                                        x: size.width - offset,
                                        y: 27.0
                                    })
                                    .is_some(),
                                "client control missing: {tab_count}/{position:?}/{offset}"
                            );
                        }
                    } else {
                        assert_eq!(focus_count + 3, client_focus_count, "{mode:?}/{tab_count}/{position:?}: controls must appear exactly once in Client mode");
                    }
                }
            }
        }
    });
}

#[derive(Default)]
struct Recorder(Vec<(String, Rect)>);
impl creamui_core::Painter for Recorder {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, rect: Rect, text: &str, _: Color, _: f32, _: creamui_core::TextAlign) {
        self.0.push((text.into(), rect));
    }
}

#[test]
fn secondary_click_targets_items_and_background_in_every_directory_view() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        creamui_reactive::provide_context(creamui_core::WindowDragHandle::new(|| {}));
        crate::test_window::TestWindow::install(TitlebarState::default());
        let fixture = tempfile::tempdir().unwrap();
        let folder = fixture.path().join("Folder");
        let file = fixture.path().join("notes.txt");
        fs::create_dir(&folder).unwrap();
        fs::write(&file, "notes").unwrap();
        let mut browser =
            BrowserState::new(WindowRequest::manager(Some(fixture.path().to_path_buf())));
        browser.explorer = ExplorerConfig::default();
        let location = browser.tab().location().clone();
        for mode in [
            DirectoryView::Grid,
            DirectoryView::List,
            DirectoryView::Columns,
        ] {
            browser.edit(|tab| tab.view = mode);
            let mut recorder = Recorder::default();
            let scene = Renderer::new().render(
                view::build(
                    &browser,
                    Size {
                        width: 1000.0,
                        height: 700.0,
                    },
                ),
                Size {
                    width: 1000.0,
                    height: 700.0,
                },
                &mut recorder,
            );
            for path in [&file, &folder] {
                let label = path.file_name().unwrap().to_str().unwrap();
                let rect = recorder
                    .0
                    .iter()
                    .find(|(text, _)| text == label)
                    .unwrap_or_else(|| panic!("Missing {label} in {mode:?}: {:?}", recorder.0))
                    .1;
                let point = Point {
                    x: rect.x + 2.0,
                    y: rect.y + rect.height / 2.0,
                };
                scene.context_menu_at(point).expect("item context target")(point);
                assert_eq!(browser.tab().selected.as_ref(), Some(path));
                assert_eq!(browser.tab().location(), &location);
                assert!(browser.last_click.borrow().is_none());
            }
            let point = Point { x: 800.0, y: 600.0 };
            scene
                .context_menu_at(point)
                .expect("directory background context target")(point);
            assert!(browser.tab().selected.is_none());
        }
    });
}

#[test]
fn save_chooser_context_action_uses_the_name_without_accepting_the_request() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("notes.txt");
    fs::write(&path, "notes").unwrap();
    let mut request = WindowRequest::save("Save".into(), "draft.txt".into());
    request.start_directory = Some(fixture.path().to_path_buf());
    let browser = BrowserState::new(request);
    let location = browser.tab().location().clone();
    let options = browser.context_menu_options(&coconut_file_menu::FileMenuTarget::Item {
        path: path.clone(),
        directory: false,
    });
    assert_eq!(options.open_label, "Use file name");
    assert!(!options.editable);
    (options.open)(path.clone(), false);
    assert_eq!(browser.name.peek(), "notes.txt");
    assert_eq!(browser.tab().selected, Some(path));
    assert_eq!(browser.tab().location(), &location);
    assert!(browser.result.lock().unwrap().is_none());
}

#[test]
fn home_context_targets_pinned_folders_drives_recent_files_and_background() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        creamui_reactive::provide_context(creamui_core::WindowDragHandle::new(|| {}));
        crate::test_window::TestWindow::install(TitlebarState::default());
        let fixture = tempfile::tempdir().unwrap();
        let pinned = fixture.path().join("Pinned folder");
        let drive = fixture.path().join("Volume");
        let file = fixture.path().join("recent.txt");
        fs::create_dir(&pinned).unwrap();
        fs::create_dir(&drive).unwrap();
        fs::write(&file, "recent").unwrap();
        let mut browser = BrowserState::new(WindowRequest::manager(None));
        browser.explorer = ExplorerConfig::default();
        browser.pins.set(vec![pinned.clone()]);
        browser.recent.set(vec![file.clone()]);
        browser.drives.set(vec![files::Drive {
            label: "Test volume".into(),
            path: drive.clone(),
            space: None,
        }]);
        let mut recorder = Recorder::default();
        let size = Size {
            width: 1000.0,
            height: 760.0,
        };
        let scene = Renderer::new().render(view::build(&browser, size), size, &mut recorder);
        for (label, path) in [
            ("Pinned folder", pinned),
            ("Test volume", drive),
            ("recent.txt", file),
        ] {
            let rect = recorder
                .0
                .iter()
                .find(|(text, rect)| text == label && rect.x > 236.0)
                .unwrap()
                .1;
            let point = Point {
                x: rect.x + 2.0,
                y: rect.y + rect.height / 2.0,
            };
            scene.context_menu_at(point).unwrap()(point);
            assert_eq!(browser.tab().selected, Some(path));
            assert_eq!(browser.tab().location(), &Location::Home);
        }
        let point = Point { x: 900.0, y: 680.0 };
        scene
            .context_menu_at(point)
            .expect("Home background context target")(point);
        assert!(browser.tab().selected.is_none());
        let options = browser
            .context_menu_options(&coconut_file_menu::FileMenuTarget::Background(files::home()));
        assert!(!options.editable);
        assert!(options.extras.iter().any(|item| item.id == "open-home"));
    });
}
