use coconut_integration_clipboard::Entry;
use coconut_plugin_kit::{
    chrome::{shell_border, shell_card},
    open_settings,
};
use creamui_core::{
    layout::{Dimension, Style as LayoutStyle},
    BoxedWidget, Size, Styled,
};
use creamui_widgets::{
    layout::{fixed, Align, Flex},
    Button, ButtonSize, ScrollController, ScrollView, Text, TextSize,
};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub available: bool,
    pub enabled: bool,
    pub revision: u64,
    pub current: String,
    pub entries: Vec<Entry>,
    pub has_next: bool,
    pub message: String,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            available: false,
            enabled: true,
            revision: 0,
            current: String::new(),
            entries: Vec::new(),
            has_next: false,
            message: "Loading clipboard…".into(),
        }
    }
}

pub(super) struct Actions {
    pub refresh: Rc<dyn Fn()>,
    pub pause: Rc<dyn Fn()>,
    pub clear: Rc<dyn Fn()>,
    pub restore: Rc<dyn Fn(u64)>,
    pub remove: Rc<dyn Fn(u64)>,
    pub navigate: Rc<dyn Fn(i32)>,
}

pub(super) fn preview(text: &str, maximum: usize) -> String {
    let mut chars = text.chars();
    let mut preview: String = chars
        .by_ref()
        .take(maximum)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if chars.next().is_some() {
        preview.push('…');
    }
    preview
}

pub(super) fn build(
    size: Size,
    snapshot: &Snapshot,
    busy: bool,
    page: u32,
    scroll: ScrollController,
    actions: &Actions,
) -> BoxedWidget {
    let width = (size.width - 30.0).max(200.0);
    let pause = actions.pause.clone();
    let clear = actions.clear.clone();
    let mut content = Flex::column()
        .size(size.width, size.height)
        .padding(14.0)
        .gap(10.0)
        .background(shell_card())
        .border(shell_border(), 1.0)
        .child(Box::new(
            Flex::row()
                .width(width)
                .align(Align::Center)
                .gap(8.0)
                .child(Box::new(Text::new("Clipboard history").size(TextSize::Lg)))
                .child(Box::new(Button::secondary(
                    ButtonSize::Xs,
                    "Settings",
                    || open_settings("/privacy/clipboard"),
                ))),
        ))
        .child(Box::new(
            Text::secondary("Currently copied").size(TextSize::Sm),
        ))
        .child(Box::new(
            Text::new(if snapshot.current.is_empty() {
                "No supported text selection"
            } else {
                &snapshot.current
            })
            .size(TextSize::Sm)
            .layout(LayoutStyle {
                size: fixed(width, 54.0),
                ..Default::default()
            }),
        ))
        .child(Box::new(
            Flex::row()
                .gap(8.0)
                .child(Box::new(
                    Button::secondary(
                        ButtonSize::Sm,
                        if snapshot.enabled {
                            "Pause history"
                        } else {
                            "Resume history"
                        },
                        move || pause(),
                    )
                    .disabled(busy),
                ))
                .child(Box::new(
                    Button::secondary(ButtonSize::Sm, "Clear history", move || clear())
                        .disabled(busy || snapshot.entries.is_empty()),
                )),
        ));
    if !snapshot.message.is_empty() {
        let refresh = actions.refresh.clone();
        content = content.child(Box::new(
            Flex::column()
                .gap(5.0)
                .child(Box::new(
                    Text::secondary(preview(&snapshot.message, 120)).size(TextSize::Sm),
                ))
                .child(Box::new(
                    Button::secondary(ButtonSize::Xs, "Retry", move || refresh()).disabled(busy),
                )),
        ));
    } else if !snapshot.enabled {
        content = content.child(Box::new(
            Text::secondary("History is paused").size(TextSize::Sm),
        ));
    }
    let mut list = Flex::column().width(width).gap(8.0);
    if snapshot.entries.is_empty() {
        list = list.child(Box::new(
            Text::secondary("No saved clipboard entries").size(TextSize::Sm),
        ));
    }
    for (id, copied_at, text) in &snapshot.entries {
        let restore = actions.restore.clone();
        let remove = actions.remove.clone();
        let id = *id;
        let time = chrono::DateTime::from_timestamp(*copied_at as i64, 0)
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        list = list.child(Box::new(
            Flex::column()
                .width(width)
                .gap(3.0)
                .child(Box::new(
                    Flex::row()
                        .width(width)
                        .gap(6.0)
                        .child(Box::new(
                            Button::secondary(ButtonSize::Sm, preview(text, 72), move || {
                                restore(id)
                            })
                            .layout(LayoutStyle {
                                size: fixed(width - 76.0, 34.0),
                                ..Default::default()
                            })
                            .disabled(busy || !snapshot.available),
                        ))
                        .child(Box::new(
                            Button::secondary(ButtonSize::Xs, "Remove", move || remove(id))
                                .disabled(busy),
                        )),
                ))
                .child(Box::new(Text::secondary(time).size(TextSize::Xs))),
        ));
    }
    content = content.child(Box::new(
        ScrollView::controlled(
            LayoutStyle {
                size: fixed(width, 0.0),
                flex_grow: 1.0,
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(width),
                    height: Dimension::Length(80.0),
                },
                ..Default::default()
            },
            scroll,
        )
        .child(Box::new(list)),
    ));
    let previous = actions.navigate.clone();
    let next = actions.navigate.clone();
    content = content.child(Box::new(
        Flex::row()
            .gap(8.0)
            .align(Align::Center)
            .child(Box::new(
                Button::secondary(ButtonSize::Xs, "Previous", move || previous(-1))
                    .disabled(busy || page == 0),
            ))
            .child(Box::new(
                Text::secondary(format!("Page {}", page + 1)).size(TextSize::Xs),
            ))
            .child(Box::new(
                Button::secondary(ButtonSize::Xs, "Next", move || next(1))
                    .disabled(busy || !snapshot.has_next),
            )),
    ));
    Box::new(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_plugin_kit::design;
    use creamui_core::{Key, KeyInput, Point, Renderer};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Color, Theme, ThemeProvider};
    use std::cell::Cell;

    #[test]
    fn popup_renders_and_restores_with_the_keyboard_in_compact_and_normal_sizes() {
        for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            for size in [
                Size {
                    width: 400.0,
                    height: 540.0,
                },
                Size {
                    width: 340.0,
                    height: 420.0,
                },
            ] {
                for mode in ["ready", "empty", "unavailable", "paused"] {
                    creamui_reactive::with_context_scope(|| {
                        design::load_fonts();
                        let theme = design::coconut_theme(theme);
                        creamui_reactive::provide_context(ThemeProvider::new(theme));
                        let restored = Rc::new(Cell::new(0));
                        let result = restored.clone();
                        let actions = Actions {
                            refresh: Rc::new(|| {}),
                            pause: Rc::new(|| {}),
                            clear: Rc::new(|| {}),
                            restore: Rc::new(move |id| result.set(id)),
                            remove: Rc::new(|_| {}),
                            navigate: Rc::new(|_| {}),
                        };
                        let snapshot = Snapshot {
                            available: mode != "unavailable",
                            enabled: mode != "paused",
                            revision: 1,
                            current: preview(
                                &"Coconut · 日本語 · long copied text ".repeat(10),
                                160,
                            ),
                            entries: if mode == "empty" {
                                Vec::new()
                            } else {
                                (1..=20)
                                    .map(|id| {
                                        (
                                            id,
                                            1000,
                                            "Previous text with a long Unicode name · 日本語"
                                                .repeat(3),
                                        )
                                    })
                                    .collect()
                            },
                            has_next: true,
                            message: if mode == "unavailable" {
                                "Clipboard permission service is unavailable".into()
                            } else {
                                String::new()
                            },
                        };
                        let mut recorder = SceneRecorder::new();
                        recorder.begin(
                            size.width as u32,
                            size.height as u32,
                            1.0,
                            Color::rgba(0, 0, 0, 0),
                            theme.colors,
                        );
                        let scene = Renderer::new().render(
                            build(
                                size,
                                &snapshot,
                                false,
                                0,
                                ScrollController::default(),
                                &actions,
                            ),
                            size,
                            &mut recorder,
                        );
                        let display = recorder.finish();
                        assert!(display.items.len() > 12);
                        if mode == "ready" {
                            assert!(
                                (0..size.height as u32)
                                    .step_by(4)
                                    .any(|y| (0..size.width as u32).step_by(4).any(|x| scene
                                        .focus_hit_test(Point {
                                            x: x as f32,
                                            y: y as f32
                                        })
                                        == Some(3))),
                                "the first restore button is visible"
                            );
                            scene
                                .on_key_at(3)
                                .expect("first restore button is focusable")(
                                KeyInput {
                                    key: Key::Enter,
                                    modifiers: Default::default(),
                                },
                            );
                            assert_eq!(restored.get(), 1);
                        }
                        let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                        raster.render(&display, &Damage::Full);
                        assert!(raster.pixmap().pixel(0, 0).unwrap().alpha() > 0);
                        if let Ok(prefix) = std::env::var("COCONUT_CLIPBOARD_SNAPSHOT") {
                            raster
                                .pixmap()
                                .save_png(format!("{prefix}-{variant}-{}-{mode}.png", size.width))
                                .unwrap();
                        }
                    });
                }
            }
        }
    }
}
