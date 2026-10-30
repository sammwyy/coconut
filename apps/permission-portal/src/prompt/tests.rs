use super::*;
use creamui_core::{Key, KeyInput, Point, Renderer};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Theme, ThemeProvider};
use std::cell::RefCell;

fn view() -> PromptView {
    PromptView {
        subject: "/opt/example/application-with-a-long-name/bin/application".into(),
        title: "Screen capture".into(),
        body: "An application wants to capture the contents of the screen.".into(),
        deny_label: "Deny".into(),
        grant_label: "Allow once".into(),
        offer_always: true,
        choices: Vec::new(),
        busy: Signal::new(false),
        error: Signal::new(String::new()),
        scroll: ScrollController::default(),
    }
}

#[test]
fn prompt_renders_visible_actions_in_both_themes_and_compact_windows() {
    creamui_reactive::with_context_scope(|| {
        let view = view();
        let calls = Rc::new(RefCell::new(Vec::new()));
        for (index, (theme, size)) in [
            (
                Theme::light(),
                Size {
                    width: 560.0,
                    height: 320.0,
                },
            ),
            (
                Theme::dark(),
                Size {
                    width: 560.0,
                    height: 320.0,
                },
            ),
            (
                Theme::light(),
                Size {
                    width: 640.0,
                    height: 480.0,
                },
            ),
            (
                Theme::dark(),
                Size {
                    width: 1100.0,
                    height: 760.0,
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let mut recorder = SceneRecorder::new();
            recorder.begin(
                size.width as u32,
                size.height as u32,
                1.0,
                theme.surface,
                theme.colors,
            );
            let record = calls.clone();
            let scene = Renderer::new().render(
                view.build(
                    size,
                    Rc::new(move |verdict, remember| record.borrow_mut().push((verdict, remember))),
                ),
                size,
                &mut recorder,
            );
            let list = recorder.finish();
            let visible: std::collections::HashSet<_> = (0..size.height as usize)
                .step_by(3)
                .flat_map(|y| {
                    (0..size.width as usize).step_by(3).filter_map({
                        let scene = &scene;
                        move |x| {
                            scene.focus_hit_test(Point {
                                x: x as f32,
                                y: y as f32,
                            })
                        }
                    })
                })
                .collect();
            assert_eq!(visible, [0, 1, 2].into_iter().collect());
            assert_eq!(scene.modal_focus(), Some(0));
            calls.borrow_mut().clear();
            for index in 0..3 {
                scene.on_key_at(index).unwrap()(KeyInput {
                    key: Key::Enter,
                    modifiers: Default::default(),
                });
            }
            assert_eq!(
                *calls.borrow(),
                [("deny", false), ("allow", false), ("allow", true)]
            );
            let mut raster = Rasterizer::new(list.width, list.height);
            raster.render(&list, &Damage::Full);
            assert!(raster
                .pixmap()
                .data()
                .chunks_exact(4)
                .any(|bytes| bytes != &raster.pixmap().data()[..4]));
            if let Ok(directory) = std::env::var("PERMISSION_PORTAL_PREVIEW_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                raster
                    .pixmap()
                    .save_png(format!("{directory}/prompt-{index}.png"))
                    .unwrap();
            }
        }
    });
}

#[test]
fn access_choices_survive_rebuilds_and_save_errors_keep_actions_visible() {
    creamui_reactive::with_context_scope(|| {
        let mut view = view();
        view.title = "Application access".into();
        view.offer_always = false;
        view.choices = vec![Choice {
            id: "level".into(),
            label: "Access level".into(),
            options: vec![("low".into(), "Low".into()), ("high".into(), "High".into())],
            controller: SelectController::new(0),
        }];
        view.choices[0].controller.select(1);
        view.error
            .set("Could not apply the decision: permission store is not writable.".into());
        let size = Size {
            width: 560.0,
            height: 400.0,
        };
        creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
        let mut renderer = Renderer::new();
        for busy in [false, true, false] {
            view.busy.set(busy);
            let mut recorder = SceneRecorder::new();
            let theme = Theme::dark();
            recorder.begin(560, 400, 1.0, theme.surface, theme.colors);
            let scene = renderer.render(view.build(size, Rc::new(|_, _| {})), size, &mut recorder);
            let list = recorder.finish();
            let visible: std::collections::HashSet<_> = (0..400)
                .step_by(3)
                .flat_map(|y| {
                    (0..560).step_by(3).filter_map({
                        let scene = &scene;
                        move |x| {
                            scene.focus_hit_test(Point {
                                x: x as f32,
                                y: y as f32,
                            })
                        }
                    })
                })
                .collect();
            assert_eq!(view.choices[0].controller.selected(), 1);
            if busy {
                assert_eq!(visible, [0].into_iter().collect());
            } else {
                assert_eq!(visible, [0, 1, 2].into_iter().collect());
                if let Ok(directory) = std::env::var("PERMISSION_PORTAL_PREVIEW_DIR") {
                    let mut raster = Rasterizer::new(560, 400);
                    raster.render(&list, &Damage::Full);
                    raster
                        .pixmap()
                        .save_png(format!("{directory}/access-error.png"))
                        .unwrap();
                }
            }
        }
    });
}
