use crate::components::*;
use crate::config::{AudioBackground, CoverShape};
use crate::state::{PlayerState, Status};
use crate::views::chrome;
use creamui_core::runtime::ImageFit;
use creamui_core::Size;
use creamui_image::Image;
use creamui_widgets::RawSpinner;

fn stage_content(child: BoxedWidget) -> BoxedWidget {
    Box::new(
        RawView::new(Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..Default::default()
        })
        .child(child),
    )
}

fn spacer(height: f32) -> BoxedWidget {
    view(fixed(1.0, height), vec![])
}

fn keycap(key: &str, text: &str) -> BoxedWidget {
    let t = use_theme();
    view(
        row(7.0),
        vec![
            Box::new(
                RawView::new(pad(
                    Style {
                        justify_content: Some(JustifyContent::Center),
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(22.0),
                            height: Dimension::Length(0.0),
                        },
                        ..row(0.0)
                    },
                    6.0,
                    3.0,
                ))
                .background(t.colors.surface_elevated)
                .corner_radius(6.0)
                .border(t.colors.border, 1.0)
                .child(Box::new(Text::new(key).font_size(11.0))),
            ),
            label(text, 11.5, true),
        ],
    )
}

fn pill_button(
    p: &PlayerState,
    icon: &str,
    text: &str,
    primary: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    let (background, hover, foreground) = if primary {
        (
            t.colors.accent,
            t.colors.accent.mix(Color::rgb(255, 255, 255), 0.12),
            t.colors.selection_text,
        )
    } else {
        (
            t.colors.surface_elevated,
            t.colors.surface_hover,
            t.colors.text_primary,
        )
    };
    let mut button = RawButton::new(
        pad(
            Style {
                justify_content: Some(JustifyContent::Center),
                ..row(8.0)
            },
            18.0,
            10.0,
        ),
        click,
    )
    .background(background)
    .corner_radius(12.0)
    .hover_style(StateStyle::new().background(hover))
    .focus_style(StateStyle::new().outline(t.colors.accent, 2.0));
    if !primary {
        button = button.border(t.colors.border, 1.0);
    }
    Box::new(button.with_children(vec![
        glyph(p, icon, 16.0, foreground),
        Box::new(Text::new(text).font_size(13.0).color(foreground)),
    ]))
}

fn welcome(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let open = p.clone();
    let add = p.clone();
    let tile = Box::new(
        RawView::new(Style {
            justify_content: Some(JustifyContent::Center),
            ..fixed(104.0, 104.0)
        })
        .background(creamui_core::LinearGradient::new(
            140.0,
            t.colors.accent.mix(Color::rgb(255, 255, 255), 0.15),
            t.colors.accent.mix(Color::rgb(0, 0, 0), 0.3),
        ))
        .corner_radius(30.0)
        .border(t.colors.border, 1.0)
        .child(glyph(p, "play", 46.0, Color::rgb(255, 255, 255))),
    ) as BoxedWidget;
    view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(16.0),
                right: LengthPercentage::Length(16.0),
                top: LengthPercentage::Length(chrome::HEIGHT),
                bottom: LengthPercentage::Length(105.0),
            },
            ..column(0.0)
        },
        vec![
            tile,
            spacer(28.0),
            Box::new(Text::new("Nothing playing").font_size(26.0).bold(true)),
            spacer(8.0),
            label(
                "Open a video or a song, or drop files anywhere in this window.",
                13.5,
                true,
            ),
            spacer(26.0),
            view(
                row(10.0),
                vec![
                    pill_button(p, "open", "Open", true, move || open.open_dialog()),
                    pill_button(p, "plus", "Add Queue", false, move || add.add_dialog()),
                    pill_button(p, "settings", "Settings", false, crate::app::open_settings),
                ],
            ),
            spacer(34.0),
            view(
                row(18.0),
                vec![
                    keycap("O", "Open"),
                    keycap("T", "Queue"),
                    keycap(",", "Settings"),
                ],
            ),
            spacer(14.0),
            Box::new(
                Text::new("MP4 · MKV · WebM · MOV · MP3 · FLAC · OGG · Opus · WAV and more")
                    .font_size(11.0)
                    .color(t.colors.text_disabled),
            ),
        ],
    )
}

fn error(p: &PlayerState, message: &str) -> BoxedWidget {
    let t = use_theme();
    let open = p.clone();
    view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(12.0)
        },
        vec![
            glyph(p, "app", 40.0, t.colors.text_secondary),
            label("Couldn't play this file", 15.0, false),
            label(message, 11.5, true),
            Box::new(
                RawButton::new(
                    pad(
                        Style {
                            justify_content: Some(JustifyContent::Center),
                            ..row(0.0)
                        },
                        18.0,
                        8.0,
                    ),
                    move || open.open_dialog(),
                )
                .background(t.colors.accent)
                .corner_radius(9.0)
                .hover_style(
                    StateStyle::new().background(t.colors.accent.mix(t.colors.text_primary, 0.12)),
                )
                .child(Box::new(
                    Text::new("Open another file")
                        .font_size(12.5)
                        .color(t.colors.selection_text),
                )),
            ),
        ],
    )
}

fn fill_layer(child: impl Styled + 'static) -> BoxedWidget {
    Box::new(child.layout(Style {
        position: creamui_core::layout::Position::Absolute,
        inset: creamui_core::layout::Rect {
            top: LengthPercentageAuto::Length(0.0),
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Length(0.0),
        },
        ..Default::default()
    }))
}

fn audio_backdrop(p: &PlayerState, background: AudioBackground) -> Vec<BoxedWidget> {
    let t = use_theme();
    let gradient = || {
        fill_layer(
            RawView::new(Style::default()).background(creamui_core::LinearGradient::new(
                160.0,
                t.colors.accent.mix(BLACK, 0.55),
                Color::rgb(6, 6, 8),
            )),
        )
    };
    match (background, p.backdrop.get()) {
        (AudioBackground::Blurred, Some(backdrop)) => {
            vec![fill_layer(Image::new(backdrop).fit(ImageFit::Cover))]
        }
        (AudioBackground::Black, _) => vec![],
        _ => vec![gradient()],
    }
}

fn audio_stage(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let appearance = p.config.get().appearance;
    let media = p.media.get();
    let narrow = size.width < 700.0;
    let room = (size.height - chrome::HEIGHT - 150.0).max(0.0);
    let side = if narrow {
        (size.width * 0.62).min(room * 0.6)
    } else {
        (size.width * 0.36).min(room)
    }
    .clamp(96.0, 420.0);
    let radius = match appearance.cover_shape {
        CoverShape::Rounded => (side * 0.05).max(8.0),
        CoverShape::Square => 3.0,
        CoverShape::Circle => side / 2.0,
    };
    let cover: BoxedWidget = match p.cover.get() {
        Some(cover) => Box::new(
            RawView::new(fixed(side, side))
                .corner_radius(radius)
                .child(Box::new(
                    Image::new(cover)
                        .fit(ImageFit::Cover)
                        .layout(fixed(side, side))
                        .corner_radius(radius),
                )),
        ),
        None => Box::new(
            RawView::new(Style {
                justify_content: Some(JustifyContent::Center),
                ..fixed(side, side)
            })
            .background(creamui_core::LinearGradient::new(
                140.0,
                t.colors.accent.mix(Color::rgb(255, 255, 255), 0.1),
                t.colors.accent.mix(BLACK, 0.5),
            ))
            .corner_radius(radius)
            .child(glyph(p, "music", side * 0.4, Color::rgb(255, 255, 255))),
        ),
    };
    let (title, subtitle) = match &media {
        Some(media) => {
            let parts: Vec<String> = media
                .album
                .iter()
                .chain(media.artist.iter())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.replace(';', ", "))
                .collect();
            let subtitle = if parts.is_empty() {
                crate::cli::kind_label(&media.path)
            } else {
                parts.join(" - ")
            };
            (media.title.clone(), subtitle)
        }
        None => (String::new(), String::new()),
    };
    let align = if narrow {
        creamui_core::TextAlign::Center
    } else {
        creamui_core::TextAlign::Start
    };
    let text_width = if narrow {
        (size.width - 64.0).max(120.0)
    } else {
        (size.width - side - 48.0 - 96.0).clamp(120.0, 420.0)
    };
    let text_style = Style {
        max_size: creamui_core::layout::Size {
            width: Dimension::Length(text_width),
            height: Dimension::Auto,
        },
        ..Default::default()
    };
    let info = view(
        Style {
            max_size: creamui_core::layout::Size {
                width: Dimension::Length(text_width),
                height: Dimension::Auto,
            },
            align_items: Some(if narrow {
                AlignItems::Center
            } else {
                AlignItems::Start
            }),
            ..column(10.0)
        },
        vec![
            Box::new(
                Text::new(title)
                    .font_size(if narrow { 24.0 } else { 32.0 })
                    .bold(true)
                    .color(Color::rgb(255, 255, 255))
                    .text_align(align)
                    .layout(text_style.clone()),
            ),
            Box::new(
                Text::new(subtitle)
                    .font_size(14.5)
                    .color(Color::rgb(185, 185, 185))
                    .text_align(align)
                    .layout(text_style),
            ),
        ],
    );
    // Positioned so it paints above the absolutely positioned backdrop layers.
    let content = view(
        Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(0.0),
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Length(0.0),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(32.0),
                right: LengthPercentage::Length(32.0),
                top: LengthPercentage::Length(chrome::HEIGHT),
                bottom: LengthPercentage::Length(if narrow { 120.0 } else { 110.0 }),
            },
            ..if narrow { column(26.0) } else { row(48.0) }
        },
        vec![cover, info],
    );
    let mut layers = audio_backdrop(p, appearance.audio_background);
    layers.push(content);
    view(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(size.width),
                height: Dimension::Length(size.height),
            },
            flex_shrink: 0.0,
            ..column(0.0)
        },
        layers,
    )
}

fn centered(children: Vec<BoxedWidget>) -> BoxedWidget {
    view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(12.0)
        },
        children,
    )
}

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let media = p.media.get();
    let mut dark = true;
    let content: BoxedWidget = match p.status.get() {
        Status::Empty => {
            dark = false;
            welcome(p)
        }
        Status::Loading => {
            dark = false;
            centered(vec![
                Box::new(RawSpinner::new(t.colors.accent).size(30.0)),
                label("Loading…", 12.0, true),
            ])
        }
        Status::Error(message) => {
            dark = false;
            error(p, &message)
        }
        Status::Playing => match &media {
            Some(media) if media.has_video => {
                let image: BoxedWidget = match p.frame.get() {
                    Some(frame) => {
                        Box::new(Image::new(frame).fit(ImageFit::Contain).layout(Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Percent(1.0),
                                height: Dimension::Percent(1.0),
                            },
                            ..Default::default()
                        }))
                    }
                    None => view(row(0.0), vec![]),
                };
                stage_content(image)
            }
            Some(_) => stage_content(audio_stage(p, size)),
            None => {
                dark = false;
                welcome(p)
            }
        },
    };
    let background: creamui_core::Background = if dark {
        BLACK.into()
    } else {
        creamui_core::LinearGradient::new(
            150.0,
            t.colors.surface,
            t.colors.surface.mix(t.colors.accent, 0.07),
        )
        .into()
    };
    Box::new(
        RawView::new(Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..column(0.0)
        })
        .background(background)
        .with_children(vec![content]),
    )
}
