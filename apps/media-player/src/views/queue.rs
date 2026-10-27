use crate::components::*;
use crate::state::PlayerState;
use creamui_widgets::{AppTitlebar, RawScrollView, Text};

fn panel_inset(
    controls_lead: bool,
    bottom: f32,
) -> creamui_core::layout::Rect<LengthPercentageAuto> {
    creamui_core::layout::Rect {
        top: LengthPercentageAuto::Length(crate::views::chrome::HEIGHT + 8.0),
        right: if controls_lead {
            LengthPercentageAuto::Length(12.0)
        } else {
            LengthPercentageAuto::Auto
        },
        bottom: LengthPercentageAuto::Length(bottom),
        left: if controls_lead {
            LengthPercentageAuto::Auto
        } else {
            LengthPercentageAuto::Length(12.0)
        },
    }
}

fn row_item(p: &PlayerState, index: usize) -> Option<RawButton> {
    let t = use_theme();
    let queue = p.queue.get();
    let item = queue.get(index)?.clone();
    let active = p.current.get() == Some(index);
    let is_video = crate::cli::is_video(&item.path);
    let (play, remove) = (p.clone(), p.clone());
    let name = item.name.clone();
    Some(
        RawButton::new(
            Style {
                padding: creamui_core::layout::Rect {
                    left: LengthPercentage::Length(10.0),
                    right: LengthPercentage::Length(6.0),
                    top: LengthPercentage::Length(7.0),
                    bottom: LengthPercentage::Length(7.0),
                },
                justify_content: Some(JustifyContent::SpaceBetween),
                ..row(8.0)
            },
            move || play.play_index(index),
        )
        .keyboard_focus(false)
        .corner_radius(8.0)
        .background(if active {
            t.colors.selection_background
        } else {
            CLEAR
        })
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .with_children(vec![
            view(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    ..row(9.0)
                },
                vec![
                    glyph(
                        p,
                        if is_video { "film" } else { "music" },
                        15.0,
                        if active {
                            t.colors.accent
                        } else {
                            t.colors.text_secondary
                        },
                    ),
                    view(
                        Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            align_items: Some(AlignItems::Start),
                            ..column(1.0)
                        },
                        vec![
                            Box::new(Text::new(name).font_size(12.5).color(if active {
                                t.colors.accent
                            } else {
                                t.colors.text_primary
                            })) as BoxedWidget,
                            label(crate::cli::kind_label(&item.path), 10.0, true),
                        ],
                    ),
                ],
            ),
            icon_button_sized(p, "close", 24.0, 13.0, false, move || {
                remove.remove_at(index)
            }),
        ]),
    )
}

pub fn build(p: &PlayerState, width: f32) -> BoxedWidget {
    let t = use_theme();
    let queue = p.queue.get();
    // Leave room for the bottom transport card.
    let bottom = 100.0;
    let mut rows: Vec<BoxedWidget> = Vec::with_capacity(queue.len() + 1);
    for index in 0..queue.len() {
        if let Some(item) = row_item(p, index) {
            rows.push(creamui_core::keyed(item, index as u64));
        }
    }
    if queue.is_empty() {
        rows.push(view(
            pad(
                Style {
                    align_items: Some(AlignItems::Center),
                    ..column(8.0)
                },
                0.0,
                26.0,
            ),
            vec![
                glyph(p, "queue", 30.0, t.colors.text_disabled),
                label("Queue is empty", 12.0, true),
                label("Add files to build a playlist", 10.5, true),
            ],
        ));
    }
    let add = p.clone();
    Box::new(
        RawButton::new(
            Style {
                position: creamui_core::layout::Position::Absolute,
                inset: panel_inset(AppTitlebar::new(width).controls_lead(), bottom),
                size: creamui_core::layout::Size {
                    width: Dimension::Length(300.0),
                    height: Dimension::Auto,
                },
                ..column(0.0)
            },
            || {},
        )
        .keyboard_focus(false)
        .background(t.colors.surface.mix(t.colors.surface_elevated, 0.5))
        .corner_radius(14.0)
        .border(t.colors.border, 1.0)
        .with_children(vec![
            view(
                pad(
                    Style {
                        justify_content: Some(JustifyContent::SpaceBetween),
                        flex_shrink: 0.0,
                        ..row(8.0)
                    },
                    14.0,
                    10.0,
                ),
                vec![
                    label(
                        if queue.is_empty() {
                            "Queue".to_string()
                        } else {
                            format!("Queue · {}", queue.len())
                        },
                        13.0,
                        false,
                    ),
                    icon_button_sized(p, "plus", 26.0, 15.0, false, move || add.add_dialog()),
                ],
            ),
            Box::new(
                RawScrollView::controlled(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        padding: creamui_core::layout::Rect {
                            left: LengthPercentage::Length(6.0),
                            right: LengthPercentage::Length(6.0),
                            top: LengthPercentage::Length(0.0),
                            bottom: LengthPercentage::Length(8.0),
                        },
                        ..column(4.0)
                    },
                    p.queue_scroll.clone(),
                )
                .scrollbar_color(t.colors.border_strong)
                .scrollbar_hover_color(t.colors.text_disabled)
                .scrollbar_pressed_color(t.colors.text_secondary)
                .child(view(column(2.0), rows)),
            ),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_follows_the_titlebar_action_side() {
        let left = panel_inset(false, 100.0);
        assert!(matches!(left.left, LengthPercentageAuto::Length(12.0)));
        assert!(matches!(left.right, LengthPercentageAuto::Auto));

        let right = panel_inset(true, 100.0);
        assert!(matches!(right.left, LengthPercentageAuto::Auto));
        assert!(matches!(right.right, LengthPercentageAuto::Length(12.0)));
    }
}
