use crate::components::*;
use crate::config::AudioBackground;
use crate::state::{PlayerState, Repeat, Status};
use creamui_core::runtime::ImageFit;
use creamui_core::{Painter, Size, Widget};
use creamui_image::ImageData;
use creamui_widgets::{RawSlider, Text};
use std::rc::Rc;

struct BarCard {
    style: creamui_core::Style,
    children: Vec<BoxedWidget>,
    color: Color,
    backdrop: Option<ImageData>,
}

impl Styled for BarCard {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.style = style;
    }
}

impl Widget for BarCard {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(backdrop) = &self.backdrop {
            painter.push_clip_rounded(rect, 16.0);
            painter.draw_image(rect, backdrop.image(), None);
            painter.pop_clip();
        }
        painter.fill_rect(rect, self.color, 16.0);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(Rc::new(|| {}))
    }
}

fn frac(point: Point, rect: Rect) -> f64 {
    (point.x / rect.width.max(1.0)).clamp(0.0, 1.0) as f64
}

fn no_media(p: &PlayerState) -> bool {
    p.status.get() != Status::Playing || p.media.get().is_none()
}

fn seekbar(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let disabled = no_media(p);
    let duration = p.duration.get();
    let fraction = if duration > 0.0 {
        (p.shown_position() / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let (start, update, end) = (p.clone(), p.clone(), p.clone());
    let rail = view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            align_items: Some(AlignItems::Center),
            ..row(0.0)
        },
        vec![Box::new(
            RawView::new(Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(5.0),
                },
                ..row(0.0)
            })
            .background(t.colors.surface_hover)
            .corner_radius(2.5)
            .child(Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(fraction),
                        height: Dimension::Length(5.0),
                    },
                    ..row(0.0)
                })
                .background(if disabled {
                    t.colors.text_disabled
                } else {
                    t.colors.accent
                })
                .corner_radius(2.5)
                .with_children(if disabled {
                    vec![]
                } else {
                    vec![Box::new(
                        RawView::new(Style {
                            position: creamui_core::layout::Position::Absolute,
                            size: creamui_core::layout::Size {
                                width: Dimension::Length(13.0),
                                height: Dimension::Length(13.0),
                            },
                            inset: creamui_core::layout::Rect {
                                top: LengthPercentageAuto::Length(-4.0),
                                right: LengthPercentageAuto::Length(-7.0),
                                bottom: LengthPercentageAuto::Auto,
                                left: LengthPercentageAuto::Auto,
                            },
                            ..row(0.0)
                        })
                        .background(t.colors.text_primary)
                        .corner_radius(6.5),
                    )]
                }),
            )),
        )],
    );
    Box::new(
        RawButton::new(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(20.0),
                },
                align_items: Some(AlignItems::Center),
                ..row(0.0)
            },
            || {},
        )
        .disabled(disabled)
        .keyboard_focus(false)
        .background(CLEAR)
        .with_drag_start(move |point, rect| start.begin_scrub(frac(point, rect)))
        .with_drag(move |point, rect| update.update_scrub(frac(point, rect)))
        .with_drag_end(move || end.end_scrub())
        .child(rail),
    )
}

fn icon_button(
    p: &PlayerState,
    icon: &str,
    active: bool,
    disabled: bool,
    on_click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    let color = if disabled {
        t.colors.text_disabled
    } else if active {
        t.colors.accent
    } else {
        t.colors.text_secondary
    };
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..fixed(30.0, 30.0)
            },
            on_click,
        )
        .disabled(disabled)
        .keyboard_focus(false)
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .child(Box::new(Icon::new(p.icons.get(icon), color).size(16.0))),
    )
}

fn media_button(
    p: &PlayerState,
    icon: &str,
    text: &str,
    show_label: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    if !show_label {
        return icon_button(p, icon, false, false, click);
    }
    let t = use_theme();
    Box::new(
        RawButton::new(pad(row(5.0), 9.0, 5.0), click)
            .keyboard_focus(false)
            .corner_radius(8.0)
            .background(t.colors.surface_hover)
            .hover_style(StateStyle::new().background(t.colors.selection_background))
            .with_children(vec![
                glyph(p, icon, 14.0, t.colors.text_primary),
                Box::new(Text::new(text).font_size(11.0)),
            ]),
    )
}

fn play_button(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let disabled = no_media(p);
    let playing = p.playing.get();
    let icon = p.icons.get(if playing { "pause" } else { "play" });
    let p = p.clone();
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..fixed(38.0, 38.0)
            },
            move || p.toggle(),
        )
        .disabled(disabled)
        .keyboard_focus(false)
        .background(if disabled {
            t.colors.surface_hover
        } else {
            t.colors.accent
        })
        .corner_radius(19.0)
        .hover_style(StateStyle::new().background(t.colors.accent.mix(t.colors.text_primary, 0.12)))
        .child(Box::new(
            Icon::new(
                icon,
                if disabled {
                    t.colors.text_disabled
                } else {
                    t.colors.selection_text
                },
            )
            .size(19.0),
        )),
    )
}

/// Distance the bar travels when it slides out below the window.
const SLIDE: f32 = 140.0;

fn time_label(text: String, end: bool) -> BoxedWidget {
    view(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(50.0),
                height: Dimension::Auto,
            },
            flex_shrink: 0.0,
            justify_content: Some(if end {
                JustifyContent::End
            } else {
                JustifyContent::Start
            }),
            ..row(0.0)
        },
        vec![label(text, 11.5, true)],
    )
}

pub fn build(p: &PlayerState, reveal: f32, size: Size) -> BoxedWidget {
    let t = use_theme();
    let config = p.config.get();
    let disabled = no_media(p);
    let max_width = config.interface.controls_width.max_width();
    let window_width = size.width;
    let inner = (window_width - 32.0).min(max_width) - 26.0;
    // Optional controls make way for the transport on narrow windows.
    let compact = inner < 460.0;
    let duration = p.duration.get();
    let position = p.shown_position();
    let end_time = if disabled {
        "00:00".into()
    } else {
        match config.interface.time_display {
            crate::config::TimeDisplay::Total => format_time(duration),
            crate::config::TimeDisplay::Remaining => {
                format!("-{}", format_time((duration - position).max(0.0)))
            }
        }
    };
    let seek_row = view(
        row(10.0),
        vec![
            time_label(format_time(position), false),
            seekbar(p),
            time_label(end_time, true),
        ],
    );

    let shuffle = {
        let p2 = p.clone();
        icon_button(p, "shuffle", p.shuffle.get(), disabled, move || {
            p2.toggle_shuffle()
        })
    };
    let prev = {
        let p2 = p.clone();
        icon_button(p, "prev", false, disabled, move || p2.prev())
    };
    let next = {
        let p2 = p.clone();
        icon_button(p, "next", false, disabled, move || {
            p2.advance(true);
        })
    };
    let repeat = {
        let p2 = p.clone();
        let mode = p.repeat.get();
        let icon = if mode == Repeat::One {
            "repeat-one"
        } else {
            "repeat"
        };
        icon_button(p, icon, mode != Repeat::Off, disabled, move || {
            p2.cycle_repeat()
        })
    };
    let speed = {
        let p2 = p.clone();
        let speed = p.speed.get();
        Box::new(
            RawButton::new(
                Style {
                    justify_content: Some(JustifyContent::Center),
                    ..fixed(44.0, 30.0)
                },
                move || p2.cycle_speed(true),
            )
            .disabled(disabled)
            .keyboard_focus(false)
            .corner_radius(8.0)
            .hover_style(StateStyle::new().background(t.colors.surface_hover))
            .child(Box::new(
                Text::new(format!("{}×", speed)).font_size(11.5).color(
                    if (speed - 1.0).abs() > 0.001 {
                        t.colors.accent
                    } else {
                        t.colors.text_secondary
                    },
                ),
            )),
        ) as BoxedWidget
    };
    let mute = {
        let p2 = p.clone();
        let muted = p.muted.get() || !p.engine.has_audio_output();
        icon_button(
            p,
            if muted { "mute" } else { "volume" },
            muted,
            disabled,
            move || p2.toggle_mute(),
        )
    };
    let volume = {
        let p2 = p.clone();
        Box::new(
            RawSlider::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(80.0),
                        height: Dimension::Length(30.0),
                    },
                    flex_shrink: 0.0,
                    ..row(0.0)
                },
                p.volume.get(),
                t.colors.surface_hover,
                t.colors.accent,
                t.colors.text_primary,
                move |v| p2.set_volume(v),
            )
            .disabled(disabled)
            .track(4.0, 2.0)
            .handle(11.0, 5.5),
        ) as BoxedWidget
    };

    let side = |justify: JustifyContent, children: Vec<BoxedWidget>| {
        view(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                justify_content: Some(justify),
                ..row(4.0)
            },
            children,
        )
    };
    let leading = vec![
        media_button(p, "open", "Open", inner >= 520.0, {
            let p = p.clone();
            move || p.open_dialog()
        }),
        media_button(p, "plus", "Add Queue", inner >= 520.0, {
            let p = p.clone();
            move || p.add_dialog()
        }),
    ];
    let trailing = if compact {
        vec![mute]
    } else {
        vec![speed, mute, volume]
    };
    let center = if inner < 340.0 {
        vec![prev, play_button(p), next]
    } else {
        vec![shuffle, prev, play_button(p), next, repeat]
    };
    let transport = view(
        row(4.0),
        vec![
            side(JustifyContent::Start, leading),
            view(
                Style {
                    flex_shrink: 0.0,
                    ..row(6.0)
                },
                center,
            ),
            side(JustifyContent::End, trailing),
        ],
    );

    let background = config
        .appearance
        .bar_background(t.colors.surface_elevated, t.colors.surface);
    let card_width = (window_width - 32.0).min(max_width).max(1.0);
    let source = if config.appearance.bar_blur {
        match p.media.get() {
            Some(media) if media.has_video => p.frame.get().map(|image| (image, ImageFit::Contain)),
            Some(_) if config.appearance.audio_background == AudioBackground::Blurred => {
                p.backdrop.get().map(|image| (image, ImageFit::Cover))
            }
            _ => None,
        }
    } else {
        None
    };
    let backdrop = source.and_then(|(image, fit)| {
        super::bar_backdrop::blurred(&image, (size.width, size.height), card_width, 84.0, fit)
    });
    let bar = BarCard {
        style: Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            max_size: creamui_core::layout::Size {
                width: Dimension::Length(max_width),
                height: Dimension::Auto,
            },
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(14.0),
                right: LengthPercentage::Length(12.0),
                top: LengthPercentage::Length(10.0),
                bottom: LengthPercentage::Length(10.0),
            },
            ..column(6.0)
        }
        .into(),
        color: background,
        backdrop,
        children: vec![seek_row, transport],
    }
    .corner_radius(16.0);

    view(
        Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(16.0),
                right: LengthPercentageAuto::Length(16.0),
                bottom: LengthPercentageAuto::Length(16.0 - (1.0 - reveal) * SLIDE),
                top: LengthPercentageAuto::Auto,
            },
            justify_content: Some(JustifyContent::Center),
            ..row(0.0)
        },
        vec![Box::new(bar)],
    )
}
