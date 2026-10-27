use super::*;
use creamui_core::{
    layout::{LengthPercentageAuto, Position},
    Key, KeyInput, Painter, Rect, Widget,
};
use creamui_widgets::{ButtonVariant, Dialog, Popover};
use std::rc::Rc;

pub(super) fn options(state: &State, width: f32) -> BoxedWidget {
    Box::new(OptionsMenu {
        state: state.clone(),
        width,
    })
}

struct OptionsMenu {
    state: State,
    width: f32,
}

impl Widget for OptionsMenu {
    fn is_modal(&self) -> bool {
        true
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn focusable(&self) -> bool {
        true
    }
    fn style(&self) -> creamui_core::Style {
        Style {
            position: Position::Absolute,
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(0.0),
                top: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            ..column(0.0)
        }
        .into()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let state = self.state.clone();
        Some(Rc::new(move || state.menu_open.set(false)))
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let state = self.state.clone();
        Some(Rc::new(move |input| {
            if input.key == Key::Escape {
                state.menu_open.set(false);
            }
        }))
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let theme = use_theme();
        let state = self.state.clone();
        let cancel = self.state.clone();
        let right = content_inset(self.width) + ((self.width - 760.0) / 2.0).max(0.0);
        vec![Box::new(
            Popover::new(pad(
                Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Auto,
                        right: LengthPercentageAuto::Length(right),
                        top: LengthPercentageAuto::Length(142.0),
                        bottom: LengthPercentageAuto::Auto,
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(224.0),
                        height: Dimension::Auto,
                    },
                    ..column(0.0)
                },
                6.0,
                6.0,
            ))
            .child(Box::new(
                RawButton::new(
                    pad(
                        Style {
                            align_items: Some(AlignItems::Center),
                            ..row(10.0)
                        },
                        10.0,
                        10.0,
                    ),
                    move || {
                        state.menu_open.set(false);
                        state.reset_open.set(true);
                    },
                )
                .on_key_press(move |input| {
                    if input.key == Key::Escape {
                        cancel.menu_open.set(false);
                    }
                })
                .corner_radius(8.0)
                .hover_style(StateStyle::new().background(theme.colors.surface_hover))
                .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
                .with_children(vec![
                    Box::new(
                        Icon::new(self.state.icons.get("refresh"), theme.colors.text_secondary)
                            .size(16.0),
                    ),
                    Box::new(Text::new("Restore defaults…").font_size(13.0)),
                ]),
            )),
        )]
    }
}

pub(super) fn reset_dialog(state: &State) -> BoxedWidget {
    let dismiss = state.clone();
    let reset = state.clone();
    Box::new(Dialog::new("Restore default preferences?", "Reset browsing and window layout preferences. Your files and folders stay as they are.", move || dismiss.reset_open.set(false))
        .dismiss_action("Cancel")
        .action("Restore defaults", ButtonVariant::Primary, move || {
            reset.config.set(Config::default());
            reset.persist();
            reset.reset_open.set(false);
        }))
}
