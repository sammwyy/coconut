use crate::components::*;
use crate::state::PlayerState;
use creamui_core::Size;
use creamui_widgets::{RawScrollView, Text};

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let Some(media) = p.media.get() else {
        return view(column(0.0), vec![]);
    };
    let height = (media.audio_tracks.len() as f32 * 38.0)
        .min((size.height - 56.0).max(40.0))
        .min(310.0);
    let rows = media
        .audio_tracks
        .into_iter()
        .map(|track| {
            let state = p.clone();
            let active = media.audio_stream == Some(track.index);
            Box::new(
                RawButton::new(pad(row(8.0), 12.0, 9.0), move || {
                    state.select_audio(track.index)
                })
                .keyboard_focus(false)
                .background(if active {
                    t.colors.selection_background
                } else {
                    t.colors.surface_elevated
                })
                .hover_style(StateStyle::new().background(t.colors.surface_hover))
                .child(Box::new(
                    Text::new(ellipsize(&track.label, 34)).font_size(12.0),
                )),
            ) as BoxedWidget
        })
        .collect();
    Box::new(
        RawView::new(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(crate::views::chrome::HEIGHT + 8.0),
                left: LengthPercentageAuto::Length(if size.width >= 380.0 { 112.0 } else { 8.0 }),
                right: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(260.0_f32.min((size.width - 16.0).max(0.0))),
                height: Dimension::Length(height),
            },
            ..column(0.0)
        })
        .background(t.colors.surface_elevated)
        .border(t.colors.border, 1.0)
        .corner_radius(12.0)
        .child(Box::new(
            RawScrollView::controlled(column(2.0), p.audio_scroll.clone())
                .scrollbar_color(t.colors.border_strong)
                .scrollbar_hover_color(t.colors.text_disabled)
                .scrollbar_pressed_color(t.colors.text_secondary)
                .child(view(column(2.0), rows)),
        )),
    )
}
