use coconut_integration_clipboard::Entry;
use coconut_plugin_kit::{
    chrome::{shell_border, shell_card, shell_control, shell_panel, shell_text},
    open_settings, pixel_icon,
};
use creamui_core::{BoxedWidget, LengthValue, Size, StateStyle, Styled};
use creamui_reactive::Signal;
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    Button, ButtonSize, ButtonState, ButtonVariant, Dialog, DialogContent, DialogFooter,
    DialogHeader, DialogMessage, ModalActions, RawButton, RawView, ScrollController, ScrollView,
    Text, TextController, TextInput, TextSize,
};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub loading: bool,
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
            loading: true,
            available: false,
            enabled: true,
            revision: 0,
            current: String::new(),
            entries: Vec::new(),
            has_next: false,
            message: String::new(),
        }
    }
}
#[derive(Clone)]
pub(super) struct Presentation {
    pub query: TextController,
    pub confirm_clear: Signal<bool>,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            query: TextController::default(),
            confirm_clear: Signal::new(false),
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
    let mut value = chars
        .by_ref()
        .take(maximum)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    if chars.next().is_some() {
        value.push('…');
    }
    value
}

pub(super) fn build(
    size: Size,
    snapshot: &Snapshot,
    busy: bool,
    page: u32,
    scroll: ScrollController,
    actions: &Actions,
    presentation: &Presentation,
) -> BoxedWidget {
    let confirm = presentation.confirm_clear.clone();
    let mut content = Flex::column()
        .size(size.width, size.height)
        .padding(16.0)
        .gap(12.0)
        .background(shell_card())
        .border(shell_border(), 1.0)
        .child(Box::new(
            Flex::row()
                .full_width()
                .align(Align::Center)
                .gap(10.0)
                .child(pixel_icon("clipboard", 22.0, shell_text()))
                .child(Box::new(
                    Flex::column()
                        .grow(1.0)
                        .gap(2.0)
                        .child(Box::new(
                            Text::new("Clipboard").size(TextSize::Lg).bold(true),
                        ))
                        .child(Box::new(
                            Text::secondary(if snapshot.loading {
                                "Loading…"
                            } else if !snapshot.available {
                                "Unavailable"
                            } else if snapshot.enabled {
                                "History is on"
                            } else {
                                "History is paused"
                            })
                            .size(TextSize::Xs),
                        )),
                ))
                .child(Box::new(Button::secondary(
                    ButtonSize::Xs,
                    "Settings",
                    || open_settings("/desktop/widgets/clipboard"),
                ))),
        ))
        .child(Box::new(
            TextInput::controlled(&presentation.query)
                .placeholder("Search clipboard history…")
                .height(34.0)
                .width(LengthValue::Percent(100.0)),
        ));
    if !snapshot.current.is_empty() {
        content = content.child(Box::new(
            Flex::column()
                .full_width()
                .padding(12.0)
                .gap(6.0)
                .background(shell_panel())
                .border(shell_border(), 1.0)
                .corner_radius(10.0)
                .child(Box::new(
                    Text::secondary("CURRENTLY COPIED")
                        .size(TextSize::Xs)
                        .bold(true),
                ))
                .child(Box::new(
                    RawView::new(
                        creamui_core::Style::new()
                            .height(if size.height < 460.0 { 20.0 } else { 36.0 })
                            .width(LengthValue::Percent(100.0)),
                    )
                    .child(Box::new(
                        Text::new(preview(&snapshot.current, 140)).size(TextSize::Sm),
                    )),
                )),
        ));
    }
    if !snapshot.message.is_empty() {
        let retry = actions.refresh.clone();
        content = content.child(Box::new(
            Flex::column()
                .gap(6.0)
                .child(Box::new(
                    Text::secondary(preview(&snapshot.message, 140)).size(TextSize::Sm),
                ))
                .child(Box::new(
                    Button::secondary(ButtonSize::Xs, "Retry", move || retry()).disabled(busy),
                )),
        ));
    }
    content = content.child(Box::new(
        Text::secondary(if presentation.query.value().is_empty() {
            "RECENT ITEMS"
        } else {
            "SEARCH RESULTS"
        })
        .size(TextSize::Xs)
        .bold(true),
    ));
    let mut list = Flex::column().full_width().gap(8.0);
    if snapshot.entries.is_empty() {
        let (title, detail) = if snapshot.loading {
            (
                "Loading your clipboard",
                "Your recent text will appear here.",
            )
        } else if !presentation.query.value().is_empty() {
            (
                "No matching items",
                "Try a different word or clear the search.",
            )
        } else if !snapshot.enabled {
            (
                "History is paused",
                "Resume to save new copies. Your current clipboard is unchanged.",
            )
        } else if !snapshot.available {
            (
                "Clipboard is unavailable",
                "Check the service and try again.",
            )
        } else {
            (
                "Nothing copied yet",
                "Copy some text and it will appear here.",
            )
        };
        list = list.child(Box::new(
            Flex::column()
                .full_width()
                .gap(6.0)
                .padding(18.0)
                .child(Box::new(Text::new(title).bold(true)))
                .child(Box::new(Text::secondary(detail).size(TextSize::Sm))),
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
        let kind = if text.starts_with("https://") || text.starts_with("http://") {
            "Link"
        } else if text.contains('\n') {
            "Multiline text"
        } else {
            "Text"
        };
        let row = RawButton::new(
            creamui_core::Style::new()
                .width(LengthValue::Percent(100.0))
                .padding(10.0),
            move || restore(id),
        )
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(10.0)
        .hover_style(StateStyle::new().background(shell_control()))
        .focus_style(StateStyle::new().outline(shell_text(), 1.5))
        .disabled(busy || !snapshot.available)
        .child(Box::new(
            Flex::row()
                .full_width()
                .align(Align::Center)
                .gap(10.0)
                .child(Box::new(
                    Flex::column()
                        .grow(1.0)
                        .min_width(0.0)
                        .gap(5.0)
                        .child(Box::new(
                            RawView::new(
                                creamui_core::Style::new()
                                    .height(32.0)
                                    .width(LengthValue::Percent(100.0)),
                            )
                            .child(Box::new(Text::new(preview(text, 180)).size(TextSize::Sm))),
                        ))
                        .child(Box::new(
                            Text::secondary(format!("{kind} · {time}")).size(TextSize::Xs),
                        )),
                ))
                .child(Box::new(
                    Button::secondary(ButtonSize::Xs, "Remove", move || remove(id))
                        .width(66.0)
                        .min_width(66.0)
                        .disabled(busy),
                )),
        ));
        list = list.child(creamui_core::keyed(row, id));
    }
    let mut scroll_style = creamui_core::layout::Style {
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Default::default()
    };
    scroll_style.size.width = creamui_core::layout::Dimension::Percent(1.0);
    scroll_style.size.height = creamui_core::layout::Dimension::Length(0.0);
    scroll_style.min_size.height = creamui_core::layout::Dimension::Length(40.0);
    content = content.child(Box::new(
        ScrollView::controlled(scroll_style, scroll).child(Box::new(list)),
    ));
    if page > 0 || snapshot.has_next {
        let previous = actions.navigate.clone();
        let next = actions.navigate.clone();
        content = content.child(Box::new(
            Flex::row()
                .full_width()
                .justify(Justify::Between)
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
    }
    let pause = actions.pause.clone();
    content = content.child(Box::new(
        Flex::row()
            .full_width()
            .align(Align::Center)
            .justify(Justify::Between)
            .child(Box::new(
                Button::secondary(
                    ButtonSize::Sm,
                    if snapshot.enabled { "Pause" } else { "Resume" },
                    move || pause(),
                )
                .width(88.0)
                .disabled(busy || snapshot.loading),
            ))
            .child(Box::new(
                Button::secondary(ButtonSize::Sm, "Clear history…", move || {
                    confirm.set(true)
                })
                .disabled(busy || snapshot.entries.is_empty()),
            )),
    ));
    let mut children = vec![Box::new(content) as BoxedWidget];
    if presentation.confirm_clear.get() {
        let cancel = presentation.confirm_clear.clone();
        let dismiss = cancel.clone();
        let close = cancel.clone();
        let clear = actions.clear.clone();
        children.push(Box::new(
            Dialog::custom(move || dismiss.set(false))
                .width((size.width - 32.0).min(360.0))
                .header(Box::new(
                    DialogHeader::new("Clear clipboard history?")
                        .on_close(move || close.set(false)),
                ))
                .content(Box::new(DialogContent::new().child(Box::new(
                    DialogMessage::new(
                        "Saved items will be removed. Your current clipboard will stay available.",
                    ),
                ))))
                .footer(Box::new(
                    DialogFooter::new().child(Box::new(
                        ModalActions::new()
                            .child(Button::styled(
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                                "Cancel",
                                ButtonState::Normal,
                                move || cancel.set(false),
                            ))
                            .child(
                                Button::styled(
                                    ButtonVariant::Destructive,
                                    ButtonSize::Sm,
                                    "Clear history",
                                    ButtonState::Normal,
                                    {
                                        let confirm = presentation.confirm_clear.clone();
                                        move || {
                                            confirm.set(false);
                                            clear();
                                        }
                                    },
                                )
                                .disabled(busy),
                            ),
                    )),
                )),
        ));
    }
    Box::new(
        Flex::column()
            .size(size.width, size.height)
            .with_children(children),
    )
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
                for mode in [
                    "ready",
                    "empty",
                    "unavailable",
                    "paused",
                    "loading",
                    "no-results",
                    "confirm-clear",
                ] {
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
                            loading: mode == "loading",
                            available: mode != "unavailable",
                            enabled: mode != "paused",
                            revision: 1,
                            current: preview(
                                &"Coconut · 日本語 · long copied text ".repeat(10),
                                160,
                            ),
                            entries: if matches!(mode, "empty" | "loading" | "no-results") {
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
                            has_next: matches!(mode, "ready" | "paused"),
                            message: if mode == "unavailable" {
                                "Clipboard permission service is unavailable".into()
                            } else {
                                String::new()
                            },
                        };
                        let presentation = Presentation::default();
                        if mode == "no-results" {
                            presentation.query.set_value("no matching text");
                        }
                        if mode == "confirm-clear" {
                            presentation.confirm_clear.set(true);
                        }
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
                                &presentation,
                            ),
                            size,
                            &mut recorder,
                        );
                        let display = recorder.finish();
                        assert!(display.items.len() > 12);
                        if mode == "ready" {
                            let focus = (0..size.height as u32)
                                .step_by(4)
                                .find_map(|y| {
                                    (0..size.width as u32).step_by(4).find_map(|x| {
                                        let point = Point {
                                            x: x as f32,
                                            y: y as f32,
                                        };
                                        scene.focus_hit_test(point).filter(|index| *index == 2)
                                    })
                                })
                                .expect("the first restore row is visible");
                            scene.on_key_at(focus).unwrap()(KeyInput {
                                key: Key::Enter,
                                modifiers: Default::default(),
                            });
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

#[cfg(test)]
mod interaction_tests {
    use super::*;
    use creamui_core::{Key, KeyInput, Renderer};
    use creamui_render::SceneRecorder;
    use creamui_theme::{Theme, ThemeProvider};
    use std::cell::Cell;

    #[test]
    fn remove_does_not_restore_and_clearing_requires_confirmation() {
        creamui_reactive::with_context_scope(|| {
            coconut_plugin_kit::design::load_fonts();
            let theme = Theme::light();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let removed = Rc::new(Cell::new(0));
            let restored = removed.clone();
            let clear_count = Rc::new(Cell::new(0));
            let cleared = clear_count.clone();
            let remove = removed.clone();
            let actions = Actions {
                refresh: Rc::new(|| {}),
                pause: Rc::new(|| {}),
                navigate: Rc::new(|_| {}),
                clear: Rc::new(move || cleared.set(cleared.get() + 1)),
                remove: Rc::new(move |id| remove.set(id)),
                restore: Rc::new(move |_| restored.set(999)),
            };
            let snapshot = Snapshot {
                loading: false,
                available: true,
                enabled: true,
                revision: 1,
                current: "Keep this current text".into(),
                entries: vec![(7, 1000, "A history item".into())],
                has_next: false,
                message: String::new(),
            };
            let presentation = Presentation::default();
            let size = Size {
                width: 400.0,
                height: 540.0,
            };
            let enter = KeyInput {
                key: Key::Enter,
                modifiers: Default::default(),
            };
            let render = |busy| {
                let mut recorder = SceneRecorder::new();
                recorder.begin(400, 540, 1.0, theme.colors.surface, theme.colors);
                Renderer::new().render(
                    build(
                        size,
                        &snapshot,
                        busy,
                        0,
                        ScrollController::default(),
                        &actions,
                        &presentation,
                    ),
                    size,
                    &mut recorder,
                )
            };
            let scene = render(false);
            scene.on_key_at(3).unwrap()(enter);
            assert_eq!(
                removed.get(),
                7,
                "remove activates without restoring the row"
            );
            scene.on_key_at(5).unwrap()(enter);
            assert!(presentation.confirm_clear.get());
            assert_eq!(clear_count.get(), 0);
            render(false).on_key_at(1).unwrap()(enter);
            assert!(!presentation.confirm_clear.get());
            assert_eq!(clear_count.get(), 0);
            render(false).on_key_at(5).unwrap()(enter);
            assert!(
                render(true).on_key_at(2).is_none(),
                "busy confirmation cannot discard its action"
            );
            render(false).on_key_at(2).unwrap()(enter);
            assert_eq!(clear_count.get(), 1);
            assert_eq!(snapshot.current, "Keep this current text");
        });
    }
}
