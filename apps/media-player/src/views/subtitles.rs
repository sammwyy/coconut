use crate::components::*;
use crate::state::PlayerState;
use creamui_core::Size;
use creamui_widgets::{RawScrollView, Text};

fn option(label: &str, click: impl Fn() + 'static) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawButton::new(pad(row(0.0), 8.0, 5.0), click)
            .keyboard_focus(false)
            .background(t.colors.surface_hover)
            .corner_radius(7.0)
            .hover_style(StateStyle::new().background(t.colors.selection_background))
            .child(Box::new(Text::new(label).font_size(11.0))),
    )
}

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let selected = p.subtitle.get();
    let mut tracks = vec![(None, "Off".to_string())];
    if let Some(media) = p.media.get() {
        tracks.extend(
            media
                .subtitles
                .into_iter()
                .map(|track| (Some(track.source), track.label)),
        );
    }
    let controls_height = if selected.is_some() { 100.0 } else { 0.0 };
    let menu_height = (tracks.len() as f32 * 38.0 + controls_height)
        .min((size.height - 56.0).max(40.0))
        .min(390.0);
    let rows = tracks
        .into_iter()
        .map(|(source, name)| {
            let state = p.clone();
            let active = selected == source;
            Box::new(
                RawButton::new(pad(row(8.0), 12.0, 9.0), move || {
                    state.select_subtitle(source.clone())
                })
                .keyboard_focus(false)
                .background(if active {
                    t.colors.selection_background
                } else {
                    t.colors.surface_elevated
                })
                .hover_style(StateStyle::new().background(t.colors.surface_hover))
                .child(Box::new(
                    Text::new(ellipsize(&name, 34))
                        .font_size(12.0)
                        .color(t.colors.text_primary),
                )),
            ) as BoxedWidget
        })
        .collect();
    let mut contents = Vec::new();
    if selected.is_some() {
        let style = p.subtitle_style.get();
        contents.push(view(
            pad(row(5.0), 8.0, 5.0),
            vec![
                option("Top", {
                    let p = p.clone();
                    move || p.set_subtitle_position(0.5, 0.12)
                }),
                option("Bottom", {
                    let p = p.clone();
                    move || p.set_subtitle_position(0.5, 0.88)
                }),
                option("Reset", {
                    let p = p.clone();
                    move || p.reset_subtitle_style()
                }),
            ],
        ));
        contents.push(view(
            pad(row(5.0), 8.0, 5.0),
            vec![
                label("Outline", 11.0, true),
                option("−", {
                    let p = p.clone();
                    move || p.adjust_subtitle_outline(-1.0)
                }),
                label(format!("{:.0}", style.outline), 11.0, false),
                option("+", {
                    let p = p.clone();
                    move || p.adjust_subtitle_outline(1.0)
                }),
            ],
        ));
        contents.push(label("Drag the handle on the video to move", 10.0, true));
    }
    contents.push(Box::new(
        RawScrollView::controlled(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                ..column(2.0)
            },
            p.subtitles_scroll.clone(),
        )
        .scrollbar_color(t.colors.border_strong)
        .scrollbar_hover_color(t.colors.text_disabled)
        .scrollbar_pressed_color(t.colors.text_secondary)
        .child(view(column(2.0), rows)),
    ));
    Box::new(
        RawView::new(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(crate::views::chrome::HEIGHT + 8.0),
                left: LengthPercentageAuto::Length(if size.width >= 350.0 { 76.0 } else { 8.0 }),
                right: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(260.0_f32.min((size.width - 16.0).max(0.0))),
                height: Dimension::Length(menu_height),
            },
            ..column(0.0)
        })
        .background(t.colors.surface_elevated)
        .border(t.colors.border, 1.0)
        .corner_radius(12.0)
        .with_children(contents),
    )
}

pub fn drag_handle(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let style = p.subtitle_style.get();
    let start = p.clone();
    let moving = p.clone();
    let finish = p.clone();
    Box::new(
        RawButton::new(
            Style {
                position: creamui_core::layout::Position::Absolute,
                inset: creamui_core::layout::Rect {
                    left: LengthPercentageAuto::Length(
                        (style.x * size.width - 48.0).clamp(0.0, (size.width - 100.0).max(0.0)),
                    ),
                    top: LengthPercentageAuto::Length(
                        (style.y * size.height - 15.0).clamp(48.0, (size.height - 40.0).max(48.0)),
                    ),
                    right: LengthPercentageAuto::Auto,
                    bottom: LengthPercentageAuto::Auto,
                },
                ..fixed(100.0, 30.0)
            },
            || {},
        )
        .keyboard_focus(false)
        .background(t.colors.accent)
        .corner_radius(8.0)
        .with_drag_start(move |_, _| start.start_subtitle_drag())
        .with_drag(move |point, rect| {
            moving.move_subtitle_pointer((rect.x + point.x, rect.y + point.y))
        })
        .with_drag_end(move || finish.finish_subtitle_drag())
        .child(Box::new(
            Text::new("Move subtitles")
                .font_size(10.0)
                .color(t.colors.selection_text),
        )),
    )
}
