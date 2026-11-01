use super::*;
use coconut_core::{DockPosition, IslandEntry, SectionConfig};
use coconut_plugin_kit::{bar_editor::Target, design, Island, IslandRenderContext, SharedState};
use creamui_core::{BoxedWidget, Point, Renderer, Scene, Size, Style};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Theme, ThemeProvider};
use creamui_widgets::{RawButton, Text};

struct Item(Rc<Cell<usize>>);
impl Island for Item {
    fn id(&self) -> &'static str {
        "clock"
    }
    fn build(&self, _: &IslandRenderContext<'_>) -> BoxedWidget {
        let clicks = self.0.clone();
        Box::new(
            RawButton::new(Style::new().width(38.0).height(32.0), move || {
                clicks.set(clicks.get() + 1)
            })
            .child(Box::new(Text::new("12:45"))),
        )
    }
}

fn fixture(position: DockPosition) -> DockConfig {
    let mut entry = IslandEntry::with_id("clock");
    entry.config.insert("format".into(), "%H:%M 日本語".into());
    DockConfig {
        position,
        sections: vec![
            SectionConfig {
                islands: vec![entry, IslandEntry::with_id("app_launcher")],
                ..Default::default()
            },
            SectionConfig::default(),
            SectionConfig {
                enabled: false,
                islands: vec![IslandEntry::with_id("clock")],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

fn render(widget: BoxedWidget, size: Size, theme: Theme, name: &str) -> Scene {
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        theme.surface,
        theme.colors,
    );
    let scene = Renderer::new().render(widget, size, &mut recorder);
    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
    raster.render(&recorder.finish(), &Damage::Full);
    if let Ok(directory) = std::env::var("COCONUT_QUICK_EDIT_SNAPSHOTS") {
        std::fs::create_dir_all(&directory).unwrap();
        raster
            .pixmap()
            .save_png(format!("{directory}/{name}.png"))
            .unwrap();
    }
    assert!(raster.pixmap().pixels().iter().any(|pixel| pixel.red() > 0));
    scene
}

fn bar_scene(
    state: &State,
    bar: &DockConfig,
    size: Size,
    theme: Theme,
    clicks: Rc<Cell<usize>>,
    name: &str,
) -> Scene {
    let context = super::super::editing::Context {
        editor: BarEditor::default(),
        quick: state.clone(),
        window: Default::default(),
        kind: BarKind::Statusbar,
        bar: bar.clone(),
        available: Rc::new(vec!["clock".into()]),
    };
    creamui_reactive::provide_context(context);
    let item: Rc<dyn Island> = Rc::new(Item(clicks));
    let open_panel: Rc<dyn Fn(&'static str, Point)> = Rc::new(|_, _| {});
    let widget = super::super::build_dock_with_lookup(
        size,
        &state.preview(bar),
        &|_| Some(item.clone()),
        &SharedState::default(),
        &open_panel,
    );
    render(widget, size, theme, name)
}

#[test]
fn on_bar_drag_moves_items_between_sections_and_reorders_in_every_orientation() {
    for base in [Theme::light(), Theme::dark()] {
        for position in [
            DockPosition::Top,
            DockPosition::Bottom,
            DockPosition::Left,
            DockPosition::Right,
        ] {
            creamui_reactive::with_context_scope(|| {
                design::load_fonts();
                let theme = design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let bar = fixture(position);
                let state = State::default();
                state.begin(&bar);
                let clicks = Rc::new(Cell::new(0));
                let size = if position.is_vertical() {
                    Size {
                        width: 96.0,
                        height: 640.0,
                    }
                } else {
                    Size {
                        width: 640.0,
                        height: 76.0,
                    }
                };
                let scene = bar_scene(
                    &state,
                    &bar,
                    size,
                    theme,
                    clicks.clone(),
                    &format!("bar-{position:?}-{}", theme.surface.r),
                );
                let source = state.bounds.borrow()[&Target::Item { group: 0, item: 0 }];
                let point = Point {
                    x: source.x + 8.0,
                    y: source.y + source.height - 8.0,
                };
                scene.hit_test(point).unwrap()();
                assert_eq!(clicks.get(), 0, "edit hitboxes must shield widget actions");
                let index = scene.drag_hit_test(point).unwrap();
                let (rect, start) = scene.drag_start_at(point).unwrap();
                start(
                    Point {
                        x: point.x - rect.x,
                        y: point.y - rect.y,
                    },
                    rect,
                );
                let destination = state.bounds.borrow()[&Target::Group(1)];
                let drop = Point {
                    x: destination.x + destination.width / 2.0,
                    y: destination.y + destination.height / 2.0,
                };
                scene.draggable_at(index).unwrap().1(
                    Point {
                        x: drop.x - rect.x,
                        y: drop.y - rect.y,
                    },
                    rect,
                );
                assert_eq!(state.preview(&bar), bar);
                let preview = bar_scene(
                    &state,
                    &bar,
                    size,
                    theme,
                    clicks.clone(),
                    "bar-drop-preview",
                );
                assert_eq!(preview.draggable_at(index).unwrap().0, rect);
                preview.drag_end_at(index).unwrap()();
                let moved = state.preview(&bar);
                assert_eq!(
                    moved.sections[1].islands[0].config["format"].as_str(),
                    Some("%H:%M 日本語")
                );
                assert_eq!(moved.sections[0].islands.len(), 1);
                let scene = bar_scene(&state, &bar, size, theme, clicks.clone(), "bar-group-ready");
                let source = state.bounds.borrow()[&Target::Group(1)];
                let point = Point {
                    x: source.x + 14.0,
                    y: source.y + 14.0,
                };
                let index = scene
                    .drag_hit_test(point)
                    .expect("section grip is draggable");
                let (rect, start) = scene.drag_start_at(point).unwrap();
                start(
                    Point {
                        x: point.x - rect.x,
                        y: point.y - rect.y,
                    },
                    rect,
                );
                let destination = state.bounds.borrow()[&Target::Group(2)];
                let drop = Point {
                    x: destination.x + destination.width - 8.0,
                    y: destination.y + destination.height - 8.0,
                };
                scene.draggable_at(index).unwrap().1(
                    Point {
                        x: drop.x - rect.x,
                        y: drop.y - rect.y,
                    },
                    rect,
                );
                scene.drag_end_at(index).unwrap()();
                assert_eq!(state.preview(&bar).sections[2], moved.sections[1]);
                state.discard();
                assert_eq!(state.preview(&bar), bar);
                assert!(!state.is_active());
            });
        }
    }
}

#[test]
fn tools_render_compact_layout_style_catalog_empty_and_save_error_states() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for size in [
            Size {
                width: 420.0,
                height: 520.0,
            },
            Size {
                width: 320.0,
                height: 400.0,
            },
        ] {
            creamui_reactive::with_context_scope(|| {
                design::load_fonts();
                let theme = design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let state = State::default();
                let bar = fixture(DockPosition::Bottom);
                state.begin(&bar);
                let available = vec![
                    "clock".into(),
                    "app_launcher".into(),
                    "current_playing".into(),
                ];
                for tab in 0..3 {
                    state.tab.set(tab);
                    render(
                        state.tools_tree(size, BarKind::Dockbar, &available, Rc::new(|| {})),
                        size,
                        theme,
                        &format!("tools-{variant}-{}-{tab}", size.width),
                    );
                }
                state.open_catalog(1);
                render(
                    state.tools_tree(size, BarKind::Dockbar, &available, Rc::new(|| {})),
                    size,
                    theme,
                    &format!("catalog-{variant}-{}", size.width),
                );
                state.handle_key(
                    KeyInput {
                        key: Key::Enter,
                        modifiers: Default::default(),
                    },
                    true,
                    &available,
                );
                assert_eq!(state.preview(&bar).sections[1].islands.len(), 1);
                state
                    .draft
                    .update(|draft| draft.as_mut().unwrap().current.sections.clear());
                render(
                    state.tools_tree(size, BarKind::Dockbar, &available, Rc::new(|| {})),
                    size,
                    theme,
                    &format!("empty-{variant}-{}", size.width),
                );
                state.persistence.report_error("The bar was changed elsewhere. Discard and reopen Quick Edit to load the latest layout.".into());
                render(
                    state.tools_tree(size, BarKind::Dockbar, &available, Rc::new(|| {})),
                    size,
                    theme,
                    &format!("error-{variant}-{}", size.width),
                );
                state.handle_key(
                    KeyInput {
                        key: Key::Escape,
                        modifiers: Default::default(),
                    },
                    true,
                    &available,
                );
                assert!(state.is_active());
                state.handle_key(
                    KeyInput {
                        key: Key::Escape,
                        modifiers: Default::default(),
                    },
                    true,
                    &available,
                );
                assert!(!state.is_active());
            });
        }
    }
}

#[test]
fn item_order_cancellation_removal_keyboard_and_busy_guards_use_the_draft() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::dark());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let bar = fixture(DockPosition::Top);
        let state = State::default();
        state.begin(&bar);
        let clicks = Rc::new(Cell::new(0));
        let size = Size {
            width: 640.0,
            height: 76.0,
        };
        let scene = bar_scene(&state, &bar, size, theme, clicks.clone(), "item-order");
        let source = state.bounds.borrow()[&Target::Item { group: 0, item: 0 }];
        let point = Point {
            x: source.x + 5.0,
            y: source.y + source.height - 5.0,
        };
        let index = scene.drag_hit_test(point).unwrap();
        let (rect, start) = scene.drag_start_at(point).unwrap();
        start(
            Point {
                x: point.x - rect.x,
                y: point.y - rect.y,
            },
            rect,
        );
        let destination = state.bounds.borrow()[&Target::Item { group: 0, item: 1 }];
        let point = Point {
            x: destination.x + destination.width - 4.0,
            y: destination.y + destination.height - 4.0,
        };
        scene.draggable_at(index).unwrap().1(
            Point {
                x: point.x - rect.x,
                y: point.y - rect.y,
            },
            rect,
        );
        scene.drag_end_at(index).unwrap()();
        assert_eq!(
            state.preview(&bar).sections[0].islands[0].id,
            "app_launcher"
        );
        let before = state.preview(&bar);
        state.start_drag(Target::Item { group: 0, item: 1 }, point);
        state.update_drag(Point { x: 700.0, y: 100.0 });
        state.finish_drag();
        assert_eq!(state.preview(&bar), before);
        state.start_drag(Target::Item { group: 0, item: 1 }, point);
        state.handle_key(
            KeyInput {
                key: Key::Escape,
                modifiers: Default::default(),
            },
            true,
            &[],
        );
        assert!(state.is_active());
        assert!(state.drag.peek().is_none());
        state.item_key(
            Target::Item { group: 0, item: 1 },
            KeyInput {
                key: Key::Right,
                modifiers: creamui_core::Modifiers {
                    alt: true,
                    ..Default::default()
                },
            },
        );
        assert_eq!(
            state.preview(&bar).sections[1].islands[0].config["format"].as_str(),
            Some("%H:%M 日本語")
        );
        let scene = bar_scene(&state, &bar, size, theme, clicks, "item-remove");
        let item = state.bounds.borrow()[&Target::Item { group: 1, item: 0 }];
        scene
            .hit_test(Point {
                x: item.x + item.width - 8.0,
                y: item.y + 8.0,
            })
            .unwrap()();
        assert!(state.preview(&bar).sections[1].islands.is_empty());
        let before = state.preview(&bar);
        state.apply(BarEdit::RemoveGroup(0));
        assert_eq!(
            state.preview(&bar),
            before,
            "occupied sections cannot be removed in Quick Edit"
        );
        state.persistence.dismiss_transients();
        state.persistence.busy.set(true);
        state.discard();
        state.apply(BarEdit::AddGroup);
        assert!(state.is_active());
        assert_eq!(state.preview(&bar).sections.len(), 3);
        state.persistence.busy.set(false);
        state.discard();
        assert_eq!(state.preview(&bar), bar);
    });
}

#[test]
fn overflowing_sections_scroll_without_exposing_clipped_drag_targets() {
    for position in [DockPosition::Top, DockPosition::Left] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(Theme::light());
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let mut bar = fixture(position);
            bar.sections[0].islands = (0..16).map(|_| IslandEntry::with_id("clock")).collect();
            let state = State::default();
            state.begin(&bar);
            let clicks = Rc::new(Cell::new(0));
            let size = if position.is_vertical() {
                Size {
                    width: 96.0,
                    height: 480.0,
                }
            } else {
                Size {
                    width: 640.0,
                    height: 76.0,
                }
            };
            bar_scene(
                &state,
                &bar,
                size,
                theme,
                clicks.clone(),
                "overflow-initial",
            );
            let scene = bar_scene(&state, &bar, size, theme, clicks.clone(), "overflow-ready");
            let clip = state.clips.borrow()[&0];
            let point = Point {
                x: clip.x + 10.0,
                y: clip.y + 10.0,
            };
            let scroll = scene
                .scroll_hit_test(point)
                .expect("long sections expose a wheel handler");
            scene.on_scroll_at(scroll).unwrap()(400.0);
            bar_scene(
                &state,
                &bar,
                size,
                theme,
                clicks,
                &format!("overflow-scrolled-{position:?}"),
            );
            let first = state.bounds.borrow()[&Target::Item { group: 0, item: 0 }];
            assert!(if position.is_vertical() {
                first.y < clip.y
            } else {
                first.x < clip.x
            });
            assert!(scene
                .drag_hit_test(Point {
                    x: size.width + 10.0,
                    y: size.height + 10.0
                })
                .is_none());
        });
    }
}

#[test]
fn real_widgets_keep_edit_controls_and_preview_layout_and_appearance() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (position, size) in [
            (
                DockPosition::Bottom,
                Size {
                    width: 1100.0,
                    height: 76.0,
                },
            ),
            (
                DockPosition::Top,
                Size {
                    width: 640.0,
                    height: 76.0,
                },
            ),
            (
                DockPosition::Left,
                Size {
                    width: 96.0,
                    height: 640.0,
                },
            ),
        ] {
            creamui_reactive::with_context_scope(|| {
                design::load_fonts();
                let theme = design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let mut bar = fixture(position);
                bar.sections[0].islands = vec![
                    IslandEntry::with_id("app_launcher"),
                    IslandEntry::with_id("workspaces"),
                ];
                bar.sections[2].islands = vec![
                    IslandEntry::with_id("clock"),
                    IslandEntry::with_id("control_center"),
                ];
                let state = State::default();
                state.begin(&bar);
                let context = super::super::editing::Context {
                    editor: BarEditor::default(),
                    quick: state.clone(),
                    window: Default::default(),
                    kind: BarKind::Statusbar,
                    bar: bar.clone(),
                    available: Rc::new(vec![]),
                };
                creamui_reactive::provide_context(context);
                let shared = SharedState::default();
                shared.insert(coconut_plugin_clock::ClockText(Signal::new("12:45".into())));
                let lookup = |id: &str| -> Option<Rc<dyn Island>> {
                    match id {
                        "clock" => Some(Rc::new(coconut_plugin_clock::ClockIsland {
                            format: "%H:%M".into(),
                        })),
                        "app_launcher" => {
                            Some(Rc::new(coconut_plugin_app_launcher::AppLauncherIsland))
                        }
                        "workspaces" => Some(Rc::new(coconut_plugin_workspaces::WorkspacesIsland(
                            "workspaces",
                        ))),
                        "control_center" => Some(Rc::new(
                            coconut_plugin_tray::ControlCenterIsland::new(Default::default()),
                        )),
                        _ => None,
                    }
                };
                let panel: Rc<dyn Fn(&'static str, Point)> = Rc::new(|_, _| {});
                for unified in [false, true] {
                    let mut draft = state.preview(&bar);
                    draft.unify_island_background = unified;
                    draft.background_opacity = 0.65;
                    draft.island_background_source = coconut_core::BackgroundSource::Custom;
                    draft.island_background_color = coconut_core::DesktopColor {
                        r: 50,
                        g: 120,
                        b: 145,
                    };
                    state.replace(draft);
                    let widget = super::super::build_dock_with_lookup(
                        size,
                        &state.preview(&bar),
                        &lookup,
                        &shared,
                        &panel,
                    );
                    let scene = render(
                        widget,
                        size,
                        theme,
                        &format!("real-{variant}-{position:?}-{unified}"),
                    );
                    let item = state.bounds.borrow()[&Target::Item { group: 0, item: 0 }];
                    assert!(scene
                        .drag_hit_test(Point {
                            x: item.x + 5.0,
                            y: item.y + item.height - 5.0
                        })
                        .is_some());
                }
                if !position.is_vertical() {
                    let mut draft = state.preview(&bar);
                    draft.sections.truncate(1);
                    draft.fill_available_space = false;
                    draft.align = coconut_core::DockAlign::End;
                    state.replace(draft);
                    let widget = super::super::build_dock_with_lookup(
                        size,
                        &state.preview(&bar),
                        &lookup,
                        &shared,
                        &panel,
                    );
                    render(
                        widget,
                        size,
                        theme,
                        &format!("aligned-{variant}-{position:?}"),
                    );
                    assert!(state.bounds.borrow()[&Target::Group(0)].x > size.width / 2.0);
                }
            });
        }
    }
}

#[test]
fn tool_controls_preview_style_preserve_placements_and_keep_save_reachable() {
    for size in [
        Size {
            width: 420.0,
            height: 520.0,
        },
        Size {
            width: 320.0,
            height: 400.0,
        },
    ] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(Theme::dark());
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let bar = fixture(DockPosition::Bottom);
            let state = State::default();
            state.begin(&bar);
            let saves = Rc::new(Cell::new(0));
            for tab in 0..3 {
                state.tab.set(tab);
                let count = saves.clone();
                let scene = render(
                    state.tools_tree(
                        size,
                        BarKind::Dockbar,
                        &["clock".into()],
                        Rc::new(move || count.set(count.get() + 1)),
                    ),
                    size,
                    theme,
                    "tools-actions",
                );
                scene
                    .hit_test(Point {
                        x: size.width - 34.0,
                        y: size.height - 26.0,
                    })
                    .expect("Save stays reachable in every tab and window size")();
            }
            assert_eq!(saves.get(), 3);
            state.tab.set(1);
            state
                .scroll
                .set(if size.width < 400.0 { 180.0 } else { 0.0 });
            let scene = render(
                state.tools_tree(size, BarKind::Dockbar, &[], Rc::new(|| {})),
                size,
                theme,
                "style-controls",
            );
            let (rect, drag) = (0..32)
                .filter_map(|index| scene.draggable_at(index))
                .find(|(rect, _)| rect.width >= 100.0 && rect.height <= 30.0)
                .expect("bar opacity stays reachable after scrolling the compact editor");
            state.apply(BarEdit::GroupGap {
                group: 0,
                gap: "17px".into(),
            });
            let placements = state.preview(&bar).sections;
            drag(
                Point {
                    x: rect.width * 0.25,
                    y: rect.height / 2.0,
                },
                rect,
            );
            let preview = state.preview(&bar);
            assert!(
                (preview.background_opacity - 0.25).abs() < 0.05,
                "opacity={} rect={rect:?} size={size:?}",
                preview.background_opacity
            );
            assert_eq!(preview.sections, placements);
            state.open_catalog(1);
            for ch in "no such widget".chars() {
                state.handle_key(
                    KeyInput {
                        key: Key::Char(ch),
                        modifiers: Default::default(),
                    },
                    true,
                    &["clock".into()],
                );
            }
            render(
                state.tools_tree(size, BarKind::Dockbar, &["clock".into()], Rc::new(|| {})),
                size,
                theme,
                &format!("catalog-no-results-{}", size.width),
            );
            state.handle_key(
                KeyInput {
                    key: Key::Enter,
                    modifiers: Default::default(),
                },
                true,
                &["clock".into()],
            );
            assert!(state.preview(&bar).sections[1].islands.is_empty());
            assert!(state.catalog.peek().is_some());
            state.handle_key(
                KeyInput {
                    key: Key::Escape,
                    modifiers: Default::default(),
                },
                true,
                &[],
            );
            assert!(state.is_active());
            assert!(state.catalog.peek().is_none());
        });
    }
}

#[test]
fn keyboard_focus_follows_the_same_placement_across_repeated_reorders() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let mut bar = fixture(DockPosition::Top);
        bar.sections[0].islands.push(IslandEntry::with_id("clock"));
        let state = State::default();
        state.begin(&bar);
        let size = Size {
            width: 1100.0,
            height: 76.0,
        };
        let context = super::super::editing::Context {
            editor: BarEditor::default(),
            quick: state.clone(),
            window: Default::default(),
            kind: BarKind::Statusbar,
            bar: bar.clone(),
            available: Rc::new(vec![]),
        };
        let item: Rc<dyn Island> = Rc::new(Item(Rc::new(Cell::new(0))));
        let shared = SharedState::default();
        let panel: Rc<dyn Fn(&'static str, Point)> = Rc::new(|_, _| {});
        let mut recorder = SceneRecorder::new();
        let mut renderer = Renderer::new();
        let mut scene = {
            recorder.begin(1100, 76, 1.0, theme.surface, theme.colors);
            renderer.render(
                build(
                    size,
                    &state.preview(&bar),
                    &|_| Some(item.clone()),
                    &shared,
                    &panel,
                    &context,
                ),
                size,
                &mut recorder,
            )
        };
        let rect = state.bounds.borrow()[&Target::Item { group: 0, item: 0 }];
        let focus = scene
            .focus_hit_test(Point {
                x: rect.x + 5.0,
                y: rect.y + rect.height - 5.0,
            })
            .unwrap();
        let id = scene.focus_id_at(focus).unwrap();
        for index in 1..=2 {
            let focus = scene
                .focus_index(id)
                .expect("the moved placement retains its focus identity");
            scene.on_key_at(focus).unwrap()(KeyInput {
                key: Key::Down,
                modifiers: creamui_core::Modifiers {
                    alt: true,
                    ..Default::default()
                },
            });
            assert_eq!(
                state.preview(&bar).sections[0].islands[index].config["format"].as_str(),
                Some("%H:%M 日本語")
            );
            recorder.begin(1100, 76, 1.0, theme.surface, theme.colors);
            scene = renderer.render(
                build(
                    size,
                    &state.preview(&bar),
                    &|_| Some(item.clone()),
                    &shared,
                    &panel,
                    &context,
                ),
                size,
                &mut recorder,
            );
        }
    });
}
