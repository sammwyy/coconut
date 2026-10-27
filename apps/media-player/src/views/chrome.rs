use crate::components::*;
use crate::state::{PlayerState, Status};
use creamui_widgets::AppTitlebar;

pub const HEIGHT: f32 = 40.0;

fn action(
    p: &PlayerState,
    dark: bool,
    icon: &str,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    if !dark {
        return icon_button(p, icon, active, click);
    }
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                ..fixed(32.0, 32.0)
            },
            click,
        )
        .keyboard_focus(false)
        .background(if active {
            Color::rgb(55, 55, 55)
        } else {
            CLEAR
        })
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(Color::rgb(45, 45, 45)))
        .child(glyph(p, icon, 17.0, Color::rgb(255, 255, 255))),
    )
}

pub fn build(p: &PlayerState, reveal: f32, width: f32) -> BoxedWidget {
    let media = p.media.get();
    let dark = p.status.get() == Status::Playing;
    let video = media.as_ref().is_some_and(|m| m.has_video) && dark;
    let queue = p.clone();
    let mut actions = vec![
        action(p, dark, "settings", false, crate::app::open_settings),
        action(p, dark, "queue", p.queue_open.get(), move || {
            queue.subtitles_open.set(false);
            queue.audio_open.set(false);
            queue.queue_open.set(!queue.queue_open.get());
        }),
    ];
    if video {
        let subtitles = p.clone();
        actions.push(action(
            p,
            dark,
            "subtitles",
            p.subtitles_open.get(),
            move || {
                subtitles.queue_open.set(false);
                subtitles.audio_open.set(false);
                subtitles
                    .subtitles_open
                    .set(!subtitles.subtitles_open.get());
            },
        ));
    }
    if media.as_ref().is_some_and(|m| m.audio_tracks.len() > 1) {
        let audio = p.clone();
        actions.push(action(
            p,
            dark,
            "audio-track",
            p.audio_open.get(),
            move || {
                audio.queue_open.set(false);
                audio.subtitles_open.set(false);
                audio.audio_open.set(!audio.audio_open.get());
            },
        ));
    }
    let mut bar = AppTitlebar::new(width)
        .height(HEIGHT)
        .start_boxed(view(row(2.0), actions));
    if dark {
        bar = bar.foreground(Color::rgb(255, 255, 255));
    }
    if video {
        let title = media.map(|m| m.title).unwrap_or_default();
        bar = bar.center(
            Text::new(ellipsize(&title, ((width - 260.0) / 7.5).max(8.0) as usize))
                .font_size(13.0)
                .bold(true)
                .color(Color::rgb(255, 255, 255)),
        );
    }
    Box::new(
        bar.layout(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(-(1.0 - reveal) * HEIGHT),
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Auto,
            },
            ..Default::default()
        })
        .background(CLEAR),
    )
}
