//! The in-process preferences window launched by `cplayer --settings`.

use crate::components::{column, fixed, pad, row, view};
use crate::config::{
    Accent, AudioBackground, BarStyle, Config, ControlsWidth, CoverShape, DiscordTitle,
    PlaybackBackend, Renderer, RepeatMode, TimeDisplay,
};
use crate::engine::AvailableBackends;
use crate::icons::Icons;
use creamui_core::{
    layout::{AlignItems, Dimension, FlexWrap, JustifyContent, LengthPercentage, Style},
    BoxedWidget, LinearGradient, Size, StateStyle, Styled,
};
use creamui_reactive::Signal;
use creamui_render::{AppHandle, WindowHandle};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{
    tab_styles, AppTitlebar, ButtonVariant, Dialog, Icon, RawButton, RawScrollView, RawSlider,
    RawView, ScrollController, SegmentedControl, Switch, Tab, TabColors, TabSizing, Tabs, Text,
};
use std::{cell::RefCell, rc::Rc};

const HEADER_HEIGHT: f32 = 64.0;
const CENTER_MARGIN: f32 = 12.0;
const DOCUMENT_WIDTH: f32 = 720.0;
const PAGES: [(Page, &str); 5] = [
    (Page::Playback, "Playback"),
    (Page::Interface, "Interface"),
    (Page::Appearance, "Appearance"),
    (Page::Shortcuts, "Shortcuts"),
    (Page::About, "About"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Playback,
    Interface,
    Appearance,
    Shortcuts,
    About,
}

#[derive(Clone)]
pub(crate) struct State {
    config: Signal<Config>,
    available_backends: Signal<Option<AvailableBackends>>,
    page: Signal<Page>,
    message: Signal<Option<String>>,
    reset_open: Signal<bool>,
    scroll: ScrollController,
    icons: Icons,
    system_accent: Color,
    pub(crate) window: Rc<RefCell<Option<WindowHandle>>>,
}

impl State {
    pub(crate) fn load() -> Self {
        Self {
            config: Signal::new(Config::load()),
            available_backends: Signal::new(None),
            page: Signal::new(Page::Playback),
            message: Signal::new(None),
            reset_open: Signal::new(false),
            scroll: ScrollController::new(0.0),
            icons: Icons::load(),
            system_accent: crate::app::load_system_theme().colors.accent,
            window: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn detect_backends(&self, app: AppHandle) {
        let state = self.clone();
        app.spawn_background(AvailableBackends::detect, move |available| {
            state.available_backends.set(Some(available));
        });
    }

    fn update(&self, change: impl FnOnce(&mut Config)) {
        let accent = self.config.peek().appearance.accent;
        self.config.update(change);
        match self.config.peek().save() {
            Ok(()) => self.message.set(None),
            Err(error) => self
                .message
                .set(Some(format!("Couldn't save preferences: {error}"))),
        }
        if self.config.peek().appearance.accent != accent {
            if let Some(window) = self.window.borrow().as_ref() {
                window.set_theme(crate::app::load_theme());
            }
        }
    }
}

pub(crate) fn build(state: &State, size: Size) -> BoxedWidget {
    creamui_reactive::with_context_scope(|| {
        let theme = use_theme();
        let page = state.page.get();
        let (title, description, body) = match page {
            Page::Playback => (
                "Playback",
                "What happens when you open something to watch or listen to.",
                playback(state),
            ),
            Page::Interface => (
                "Interface",
                "How the player controls behave while you watch.",
                interface(state),
            ),
            Page::Appearance => (
                "Appearance",
                "Colors, artwork and the look of the controls.",
                appearance(state),
            ),
            Page::Shortcuts => (
                "Keyboard shortcuts",
                "Keys available in the player window.",
                shortcuts(state),
            ),
            Page::About => (
                "About",
                "",
                coconut_ui::about!("Media Player", state.icons.get("app")),
            ),
        };
        let mut document_children = if page == Page::About {
            vec![body]
        } else {
            vec![
                page_heading(state, title, description, page != Page::Shortcuts),
                body,
                Box::new(
                    Text::secondary("Changes are saved automatically and apply to open players.")
                        .font_size(12.0),
                ) as BoxedWidget,
            ]
        };
        if let Some(message) = state.message.get() {
            document_children.push(Box::new(
                Text::new(message)
                    .color(theme.colors.danger)
                    .font_size(12.0),
            ));
        }
        let inset = if size.width < 600.0 { 24.0 } else { 40.0 };
        let mut document = pad(document(22.0), inset, 24.0);
        document.padding.bottom = LengthPercentage::Length(20.0);
        let content = RawScrollView::controlled(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                ..column(0.0)
            },
            state.scroll.clone(),
        )
        .scrollbar_width(5.0)
        .scrollbar_margin(4.0)
        .scrollbar_color(theme.colors.border_strong)
        .scrollbar_hover_color(theme.colors.text_disabled)
        .scrollbar_pressed_color(theme.colors.text_secondary)
        .child(view(document, document_children));
        let mut children = vec![
            titlebar(state, page, size.width),
            Box::new(content) as BoxedWidget,
        ];
        if state.reset_open.get() {
            children.push(reset_dialog(state));
        }
        Box::new(
            RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(size.width),
                    height: Dimension::Length(size.height),
                },
                ..column(0.0)
            })
            .background(LinearGradient::new(
                150.0,
                theme.colors.surface,
                theme.colors.surface.mix(theme.colors.accent, 0.06),
            ))
            .with_children(children),
        )
    })
}

fn titlebar(state: &State, page: Page, width: f32) -> BoxedWidget {
    let labels: Vec<&str> = PAGES.iter().map(|(_, label)| *label).collect();
    let wanted = tab_styles(&labels, TabSizing::Equal, 34.0, 14.0)
        .iter()
        .map(|s| match s.size.width {
            Dimension::Length(w) => w,
            _ => 0.0,
        })
        .sum::<f32>()
        + 8.0 * (labels.len() as f32 - 1.0)
        + 8.0;
    let bar = AppTitlebar::new(width)
        .height(HEADER_HEIGHT)
        .center_margin(CENTER_MARGIN);
    let (start, end) = bar.reserved();
    let centered = width - 2.0 * (start.max(end) + CENTER_MARGIN);
    // Narrow windows give the tabs all the room beside the controls instead
    // of a symmetric slot.
    let available = if centered >= wanted {
        wanted
    } else {
        (width - start - end - 2.0 * CENTER_MARGIN).max(0.0)
    };
    let tabs = RawView::new(fixed(available, 42.0)).child(navigation(state, page, available));
    Box::new(if centered >= wanted {
        bar.center(tabs)
    } else {
        bar.padding(CENTER_MARGIN).mirror(false).start(tabs)
    })
}

fn navigation(state: &State, active: Page, available: f32) -> BoxedWidget {
    let theme = use_theme();
    let labels: Vec<&str> = PAGES.iter().map(|(_, label)| *label).collect();
    let mut styles = tab_styles(&labels, TabSizing::Equal, 34.0, 14.0);
    let share =
        ((available - 8.0 - 8.0 * (labels.len() as f32 - 1.0)) / labels.len() as f32).max(0.0);
    for style in &mut styles {
        style.padding = Style::default().padding;
        if let Dimension::Length(width) = style.size.width {
            style.size.width = Dimension::Length(width.min(share));
        }
    }
    let mut colors = TabColors::dark();
    colors.background = theme.colors.surface_elevated;
    colors.active_background = theme.colors.selection_background;
    colors.active_text = theme.colors.accent;
    colors.inactive_background = None;
    colors.container_radius = 12.0;
    let mut tabs = Tabs::new(
        colors,
        pad(
            Style {
                flex_shrink: 0.0,
                ..row(8.0)
            },
            4.0,
            4.0,
        ),
    );
    for (index, (page, label)) in PAGES.into_iter().enumerate() {
        let state = state.clone();
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            label,
            active == page,
            move || {
                state.scroll.set(0.0);
                state.page.set(page);
            },
        )));
    }
    Box::new(tabs)
}

fn page_heading(state: &State, title: &str, description: &str, resettable: bool) -> BoxedWidget {
    let theme = use_theme();
    let reset = state.clone();
    let mut children = vec![
        view(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                align_items: Some(AlignItems::Start),
                ..column(5.0)
            },
            vec![
                Box::new(Text::new(title).font_size(24.0).bold(true)),
                Box::new(Text::secondary(description).font_size(13.0)),
            ],
        ),
        Box::new(
            RawButton::new(
                pad(
                    Style {
                        justify_content: Some(JustifyContent::Center),
                        flex_shrink: 0.0,
                        ..row(7.0)
                    },
                    12.0,
                    7.0,
                ),
                move || reset.reset_open.set(true),
            )
            .background(theme.colors.surface_elevated)
            .corner_radius(10.0)
            .outline(theme.colors.border, 1.0)
            .hover_style(StateStyle::new().background(theme.colors.surface_hover))
            .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
            .with_children(vec![
                Box::new(
                    Icon::new(state.icons.get("refresh"), theme.colors.text_secondary).size(14.0),
                ),
                Box::new(Text::secondary("Restore defaults").font_size(12.0)),
            ]),
        ) as BoxedWidget,
    ];
    if !resettable {
        children.pop();
    }
    view(row(16.0), children)
}

fn reset_dialog(state: &State) -> BoxedWidget {
    let dismiss = state.clone();
    let reset = state.clone();
    Box::new(
        Dialog::new(
            "Restore default preferences?",
            "Playback, interface and appearance settings go back to their original values.",
            move || dismiss.reset_open.set(false),
        )
        .dismiss_action("Cancel")
        .action("Restore defaults", ButtonVariant::Primary, move || {
            reset.update(|config| *config = Config::default());
            reset.reset_open.set(false);
        }),
    )
}

fn playback(state: &State) -> BoxedWidget {
    let playback = state.config.get().playback;
    let available = state.available_backends.get();
    let backend_options = available.unwrap_or_default().choices();
    let backend = if available.is_none() {
        Box::new(Text::secondary("Detecting playback backends…")) as BoxedWidget
    } else if backend_options.is_empty() {
        Box::new(Text::secondary(
            "No backend available. Install ffprobe and libmpv or ffmpeg.",
        )) as BoxedWidget
    } else {
        let selected = available.unwrap_or_default().effective(playback.backend);
        let labels: Vec<_> = backend_options
            .iter()
            .map(|backend| match backend {
                PlaybackBackend::Mpv => "libmpv",
                PlaybackBackend::Ffmpeg => "FFmpeg",
                PlaybackBackend::Auto => unreachable!(),
            })
            .collect();
        segmented(
            state,
            backend_options
                .iter()
                .position(|backend| Some(*backend) == selected)
                .unwrap_or(0),
            &labels,
            move |config, index| config.playback.backend = backend_options[index],
        )
    };
    let volume = {
        let state = state.clone();
        view(
            row(10.0),
            vec![
                Box::new(
                    RawSlider::new(
                        Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Length(150.0),
                                height: Dimension::Length(28.0),
                            },
                            flex_shrink: 0.0,
                            ..row(0.0)
                        },
                        playback.volume,
                        use_theme().colors.surface_hover,
                        use_theme().colors.accent,
                        use_theme().colors.text_primary,
                        move |v| state.update(|c| c.playback.volume = (v * 20.0).round() / 20.0),
                    )
                    .track(4.0, 2.0)
                    .handle(13.0, 6.5),
                ) as BoxedWidget,
                view(
                    Style {
                        justify_content: Some(JustifyContent::End),
                        ..fixed(38.0, 20.0)
                    },
                    vec![Box::new(
                        Text::secondary(format!("{}%", (playback.volume * 100.0).round()))
                            .font_size(12.0),
                    )],
                ),
            ],
        )
    };
    let speeds = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
    let speed = segmented(
        state,
        speeds
            .iter()
            .position(|s| (s - playback.speed).abs() < 0.01)
            .unwrap_or(2),
        &["0.5×", "0.75×", "1×", "1.25×", "1.5×", "2×"],
        move |c, i| c.playback.speed = speeds[i],
    );
    let repeat = segmented(
        state,
        playback.repeat as usize,
        &["Off", "All", "One"],
        |c, i| {
            c.playback.repeat = [RepeatMode::Off, RepeatMode::All, RepeatMode::One][i];
        },
    );
    let short_steps = [5, 10, 15];
    let long_steps = [30, 60, 120];
    let seek = segmented(
        state,
        short_steps
            .iter()
            .position(|s| *s == playback.seek_step)
            .unwrap_or(0),
        &["5 s", "10 s", "15 s"],
        move |c, i| c.playback.seek_step = short_steps[i],
    );
    let seek_long = segmented(
        state,
        long_steps
            .iter()
            .position(|s| *s == playback.seek_step_long)
            .unwrap_or(0),
        &["30 s", "1 min", "2 min"],
        move |c, i| c.playback.seek_step_long = long_steps[i],
    );
    view(
        column(22.0),
        vec![
            section(
                "Playback engine",
                vec![preference_row(
                    state,
                    "play",
                    "Backend",
                    "Available engines only. Switching preserves the current track and position.",
                    backend,
                )],
            ),
            section(
                "When the player starts",
                vec![
                    preference_row(state, "volume", "Volume", "Initial output level.", volume),
                    preference_row(state, "speed", "Speed", "Initial playback rate.", speed),
                    preference_row(
                        state,
                        "repeat",
                        "Repeat",
                        "Repeat the queue or the current track.",
                        repeat,
                    ),
                    preference_row(
                        state,
                        "shuffle",
                        "Shuffle",
                        "Play the queue in random order.",
                        toggle(state, playback.shuffle, |c| {
                            c.playback.shuffle = !c.playback.shuffle
                        }),
                    ),
                ],
            ),
            section(
                "Files",
                vec![preference_row(
                    state,
                    "play",
                    "Play automatically",
                    "Start playback as soon as a file has loaded.",
                    toggle(state, playback.autoplay, |c| {
                        c.playback.autoplay = !c.playback.autoplay
                    }),
                )],
            ),
            section(
                "Seeking",
                vec![
                    preference_row(state, "forward", "Arrow keys", "Skip with ← and →.", seek),
                    preference_row(
                        state,
                        "forward",
                        "Shift + arrow keys",
                        "Larger skips with Shift held.",
                        seek_long,
                    ),
                ],
            ),
        ],
    )
}

fn interface(state: &State) -> BoxedWidget {
    let interface = state.config.get().interface;
    let delays = [2, 3, 5, 10];
    let delay = segmented(
        state,
        delays
            .iter()
            .position(|d| *d == interface.autohide_delay)
            .unwrap_or(1),
        &["2 s", "3 s", "5 s", "10 s"],
        move |c, i| c.interface.autohide_delay = delays[i],
    );
    let width = segmented(
        state,
        interface.controls_width as usize,
        &["Compact", "Default", "Wide"],
        |c, i| {
            c.interface.controls_width = [
                ControlsWidth::Compact,
                ControlsWidth::Default,
                ControlsWidth::Wide,
            ][i];
        },
    );
    let time = segmented(
        state,
        interface.time_display as usize,
        &["Duration", "Remaining"],
        |c, i| c.interface.time_display = [TimeDisplay::Total, TimeDisplay::Remaining][i],
    );
    let renderer = segmented(
        state,
        interface.renderer as usize,
        &["GPU", "CPU"],
        |c, i| c.interface.renderer = [Renderer::Gpu, Renderer::Cpu][i],
    );
    view(
        column(22.0),
        vec![
            section(
                "Controls",
                vec![
                    preference_row(
                        state,
                        "timer",
                        "Hide controls after",
                        "Idle time before video controls slide away.",
                        delay,
                    ),
                    preference_row(
                        state,
                        "sparkle",
                        "Animations",
                        "Slide the controls in and out smoothly.",
                        toggle(state, interface.animations, |c| {
                            c.interface.animations = !c.interface.animations
                        }),
                    ),
                    preference_row(
                        state,
                        "panel",
                        "Controls width",
                        "Maximum width of the playback bar.",
                        width,
                    ),
                ],
            ),
            section(
                "Information",
                vec![preference_row(
                    state,
                    "clock",
                    "Time on the right",
                    "Show the total length or the time left.",
                    time,
                )],
            ),
            section(
                "Rendering",
                vec![preference_row(
                    state,
                    "sliders",
                    "Renderer",
                    "How windows are drawn. Applies after restarting.",
                    renderer,
                )],
            ),
            section(
                "Window",
                vec![preference_row(
                    state,
                    "panel",
                    "Keep playing in background",
                    "Closing the player keeps playback running in the tray. Applies after restarting the player.",
                    toggle(state, interface.background_playback, |c| {
                        c.interface.background_playback = !c.interface.background_playback
                    }),
                )],
            ),
            section(
                "Integrations",
                vec![
                    preference_row(
                        state,
                        "chat",
                        "Discord Rich Presence",
                        "Show the current track as your Discord status while Discord is running.",
                        toggle(state, interface.discord_presence, |c| {
                            c.interface.discord_presence = !c.interface.discord_presence
                        }),
                    ),
                    preference_row(
                        state,
                        "text",
                        "Only for tagged files",
                        "Hide the status for files without artist metadata instead of showing the filename.",
                        toggle(state, interface.discord_metadata_only, |c| {
                            c.interface.discord_metadata_only =
                                !c.interface.discord_metadata_only
                        }),
                    ),
                    preference_row(
                        state,
                        "text",
                        "Status text",
                        "What leads the Discord presence; the second line shows the remaining field.",
                        segmented(
                            state,
                            interface.discord_title as usize,
                            &["Artist", "Title", "Artist - Title", "Title - Artist"],
                            |c, i| {
                                c.interface.discord_title = [
                                    DiscordTitle::Artist,
                                    DiscordTitle::Title,
                                    DiscordTitle::ArtistTitle,
                                    DiscordTitle::TitleArtist,
                                ][i];
                            },
                        ),
                    ),
                    preference_row(
                        state,
                        "image",
                        "Cover art",
                        "Upload embedded artwork to catbox.moe so Discord can display it.",
                        toggle(state, interface.discord_covers, |c| {
                            c.interface.discord_covers = !c.interface.discord_covers
                        }),
                    ),
                ],
            ),
        ],
    )
}

fn appearance(state: &State) -> BoxedWidget {
    let appearance = state.config.get().appearance;
    let background = segmented(
        state,
        appearance.audio_background as usize,
        &["Blurred art", "Gradient", "Black"],
        |c, i| {
            c.appearance.audio_background = [
                AudioBackground::Blurred,
                AudioBackground::Gradient,
                AudioBackground::Black,
            ][i];
        },
    );
    let shape = segmented(
        state,
        appearance.cover_shape as usize,
        &["Rounded", "Square", "Circle"],
        |c, i| {
            c.appearance.cover_shape =
                [CoverShape::Rounded, CoverShape::Square, CoverShape::Circle][i];
        },
    );
    let bar = segmented(
        state,
        appearance.bar_style as usize,
        &["Floating", "Solid"],
        |c, i| c.appearance.bar_style = [BarStyle::Floating, BarStyle::Solid][i],
    );
    let opacity = {
        let state = state.clone();
        view(
            row(10.0),
            vec![
                Box::new(
                    RawSlider::new(
                        Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Length(150.0),
                                height: Dimension::Length(28.0),
                            },
                            flex_shrink: 0.0,
                            ..row(0.0)
                        },
                        appearance.bar_opacity.min(100) as f32 / 100.0,
                        use_theme().colors.surface_hover,
                        use_theme().colors.accent,
                        use_theme().colors.text_primary,
                        move |value| {
                            state.update(|c| {
                                c.appearance.bar_opacity =
                                    (value.clamp(0.0, 1.0) * 100.0).round() as u8
                            })
                        },
                    )
                    .track(4.0, 2.0)
                    .handle(13.0, 6.5),
                ) as BoxedWidget,
                Box::new(
                    Text::secondary(format!("{}%", appearance.bar_opacity.min(100)))
                        .font_size(12.0),
                ),
            ],
        )
    };
    view(
        column(22.0),
        vec![
            preview(state),
            labeled("Accent color", accents(state, appearance.accent)),
            section(
                "Music",
                vec![
                    preference_row(
                        state,
                        "image",
                        "Background",
                        "Behind the cover art while music plays.",
                        background,
                    ),
                    preference_row(
                        state,
                        "shape",
                        "Cover shape",
                        "Corners of the album artwork.",
                        shape,
                    ),
                ],
            ),
            section(
                "Controls",
                vec![
                    preference_row(
                        state,
                        "panel",
                        "Playback bar",
                        "Raised or flat background.",
                        bar,
                    ),
                    preference_row(
                        state,
                        "sliders",
                        "Background opacity",
                        "Only the card background changes; controls remain fully visible.",
                        opacity,
                    ),
                    preference_row(
                        state,
                        "image",
                        "Blur behind controls",
                        "Soften video or artwork beneath the playback bar.",
                        toggle(state, appearance.bar_blur, |c| {
                            c.appearance.bar_blur = !c.appearance.bar_blur
                        }),
                    ),
                ],
            ),
        ],
    )
}

fn accents(state: &State, selected: Accent) -> BoxedWidget {
    let theme = use_theme();
    let swatches = Accent::ALL
        .into_iter()
        .map(|accent| {
            let color: creamui_core::Background = match accent {
                Accent::Auto => {
                    LinearGradient::new(135.0, Color::rgb(236, 72, 153), Color::rgb(59, 130, 246))
                        .into()
                }
                _ => accent.color().unwrap_or(state.system_accent).into(),
            };
            let active = accent == selected;
            let choose = state.clone();
            let glyph = if active {
                Some("check")
            } else if accent == Accent::Auto {
                Some("sparkle")
            } else {
                None
            };
            let check: BoxedWidget = match glyph {
                Some(name) => {
                    Box::new(Icon::new(state.icons.get(name), Color::rgb(255, 255, 255)).size(14.0))
                }
                None => view(row(0.0), vec![]),
            };
            view(
                Style {
                    align_items: Some(AlignItems::Center),
                    ..column(6.0)
                },
                vec![
                    Box::new(
                        RawButton::new(
                            Style {
                                justify_content: Some(JustifyContent::Center),
                                ..fixed(34.0, 34.0)
                            },
                            move || choose.update(|c| c.appearance.accent = accent),
                        )
                        .background(color)
                        .corner_radius(17.0)
                        .outline(
                            if active {
                                theme.colors.text_primary
                            } else {
                                Color::rgba(0, 0, 0, 0)
                            },
                            2.0,
                        )
                        .hover_style(StateStyle::new().outline(theme.colors.border, 2.0))
                        .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
                        .child(check),
                    ) as BoxedWidget,
                    Box::new(
                        Text::secondary(accent.label())
                            .font_size(11.0)
                            .color(if active {
                                theme.colors.text_primary
                            } else {
                                theme.colors.text_secondary
                            }),
                    ),
                ],
            )
        })
        .collect();
    Box::new(
        RawView::new(pad(
            Style {
                flex_wrap: FlexWrap::Wrap,
                justify_content: Some(JustifyContent::SpaceBetween),
                ..row(14.0)
            },
            18.0,
            14.0,
        ))
        .background(theme.colors.surface_elevated)
        .outline(theme.colors.border, 1.0)
        .corner_radius(16.0)
        .with_children(swatches),
    )
}

/// A miniature of the music view reflecting the current appearance choices.
fn preview(state: &State) -> BoxedWidget {
    let theme = use_theme();
    let config = state.config.get();
    let appearance = &config.appearance;
    let accent = appearance.accent.color().unwrap_or(state.system_accent);
    let background: creamui_core::Background = match appearance.audio_background {
        AudioBackground::Blurred => LinearGradient::new(
            135.0,
            accent.mix(Color::rgb(20, 10, 40), 0.35),
            Color::rgb(30, 60, 90).mix(Color::rgb(0, 0, 0), 0.4),
        )
        .into(),
        AudioBackground::Gradient => LinearGradient::new(
            160.0,
            accent.mix(Color::rgb(0, 0, 0), 0.55),
            Color::rgb(6, 6, 8),
        )
        .into(),
        AudioBackground::Black => Color::rgb(0, 0, 0).into(),
    };
    let side = 74.0;
    let radius = match appearance.cover_shape {
        CoverShape::Rounded => 10.0,
        CoverShape::Square => 2.0,
        CoverShape::Circle => side / 2.0,
    };
    let cover = Box::new(
        RawView::new(Style {
            justify_content: Some(JustifyContent::Center),
            ..fixed(side, side)
        })
        .background(LinearGradient::new(
            135.0,
            accent,
            accent.mix(Color::rgb(0, 0, 0), 0.45),
        ))
        .corner_radius(radius)
        .child(Box::new(
            Icon::new(state.icons.get("music"), Color::rgb(255, 255, 255)).size(30.0),
        )),
    ) as BoxedWidget;
    let info = view(
        Style {
            align_items: Some(AlignItems::Start),
            ..column(4.0)
        },
        vec![
            Box::new(
                Text::new("Song title")
                    .font_size(15.0)
                    .bold(true)
                    .color(Color::rgb(255, 255, 255)),
            ),
            Box::new(
                Text::new("Album - Artist")
                    .font_size(11.0)
                    .color(Color::rgb(185, 185, 185)),
            ),
        ],
    );
    let bar_width = match config.interface.controls_width {
        ControlsWidth::Compact => 0.56,
        ControlsWidth::Default => 0.72,
        ControlsWidth::Wide => 0.92,
    };
    let bar_bg = appearance.bar_background(theme.colors.surface_elevated, theme.colors.surface);
    let bar = Box::new(
        RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(bar_width),
                height: Dimension::Length(22.0),
            },
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(10.0),
                right: LengthPercentage::Length(10.0),
                top: LengthPercentage::Length(0.0),
                bottom: LengthPercentage::Length(0.0),
            },
            ..row(8.0)
        })
        .background(bar_bg)
        .corner_radius(8.0)
        .with_children(vec![
            Box::new(
                RawView::new(fixed(10.0, 10.0))
                    .background(accent)
                    .corner_radius(5.0),
            ) as BoxedWidget,
            Box::new(
                RawView::new(Style {
                    flex_grow: 1.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(3.0),
                    },
                    ..row(0.0)
                })
                .background(theme.colors.surface_hover)
                .corner_radius(1.5)
                .child(Box::new(
                    RawView::new(Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(0.4),
                            height: Dimension::Length(3.0),
                        },
                        ..row(0.0)
                    })
                    .background(accent)
                    .corner_radius(1.5),
                )),
            ),
        ]),
    ) as BoxedWidget;
    Box::new(
        RawView::new(pad(
            Style {
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::SpaceBetween),
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(176.0),
                },
                ..column(0.0)
            },
            16.0,
            16.0,
        ))
        .background(background)
        .corner_radius(16.0)
        .outline(theme.colors.border, 1.0)
        .with_children(vec![
            view(fixed(1.0, 6.0), vec![]),
            view(row(18.0), vec![cover, info]),
            bar,
        ]),
    )
}

fn shortcuts(state: &State) -> BoxedWidget {
    let playback = state.config.get().playback;
    let short = format!("Seek {} seconds", playback.seek_step);
    let long = format!("Seek {} seconds", playback.seek_step_long);
    let group = |title: &str, items: &[(&[&str], &str)]| {
        section(
            title,
            items
                .iter()
                .map(|(keys, action)| shortcut_row(keys, action))
                .collect(),
        )
    };
    view(
        column(22.0),
        vec![
            group(
                "Playback",
                &[
                    (&["Space"], "Play or pause"),
                    (&["←", "→"], &short),
                    (&["Shift", "← →"], &long),
                    (&["↑", "↓"], "Volume up or down"),
                    (&["M"], "Mute"),
                    (&["[", "]"], "Slower or faster"),
                ],
            ),
            group(
                "Queue",
                &[
                    (&["N"], "Next item"),
                    (&["P"], "Previous item"),
                    (&["L"], "Cycle repeat mode"),
                    (&["S"], "Shuffle"),
                    (&["T"], "Show or hide the queue"),
                ],
            ),
            group(
                "Window",
                &[
                    (&["O"], "Open files"),
                    (&["F"], "Maximize"),
                    (&[","], "Open settings"),
                    (&["Q"], "Close player window"),
                ],
            ),
        ],
    )
}

fn shortcut_row(keys: &[&str], action: &str) -> BoxedWidget {
    let theme = use_theme();
    let caps = keys
        .iter()
        .map(|key| {
            Box::new(
                RawView::new(pad(
                    Style {
                        justify_content: Some(JustifyContent::Center),
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(26.0),
                            height: Dimension::Length(0.0),
                        },
                        ..row(0.0)
                    },
                    8.0,
                    4.0,
                ))
                .background(theme.colors.surface)
                .outline(theme.colors.border, 1.0)
                .corner_radius(6.0)
                .child(Box::new(Text::new(*key).font_size(11.5))),
            ) as BoxedWidget
        })
        .collect();
    view(
        pad(
            Style {
                justify_content: Some(JustifyContent::SpaceBetween),
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(46.0),
                },
                ..row(12.0)
            },
            16.0,
            8.0,
        ),
        vec![
            Box::new(Text::new(action).font_size(13.0)),
            view(row(6.0), caps),
        ],
    )
}

fn segmented(
    state: &State,
    selected: usize,
    options: &[&str],
    apply: impl Fn(&mut Config, usize) + 'static,
) -> BoxedWidget {
    let state = state.clone();
    let mut control = SegmentedControl::new(selected, move |index| {
        state.update(|config| apply(config, index))
    });
    for option in options {
        control = control.option(*option);
    }
    Box::new(control)
}

fn toggle(state: &State, value: bool, flip: fn(&mut Config)) -> BoxedWidget {
    let state = state.clone();
    Box::new(Switch::new(value, move || state.update(flip)))
}

fn preference_row(
    state: &State,
    icon: &str,
    title: &str,
    description: &str,
    control: BoxedWidget,
) -> BoxedWidget {
    view(
        pad(
            Style {
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(62.0),
                },
                flex_shrink: 0.0,
                flex_wrap: FlexWrap::Wrap,
                ..row(12.0)
            },
            16.0,
            12.0,
        ),
        vec![
            icon_badge(state, icon),
            view(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(160.0),
                    align_items: Some(AlignItems::Start),
                    ..column(3.0)
                },
                vec![
                    Box::new(Text::new(title).font_size(13.0)),
                    Box::new(Text::secondary(description).font_size(11.0)),
                ],
            ),
            view(
                Style {
                    flex_shrink: 0.0,
                    ..row(0.0)
                },
                vec![control],
            ),
        ],
    )
}

fn section(title: &str, rows: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    let mut children = Vec::new();
    for (index, item) in rows.into_iter().enumerate() {
        if index > 0 {
            children.push(Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(1.0),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .background(theme.colors.border),
            ) as BoxedWidget);
        }
        children.push(item);
    }
    labeled(
        title,
        Box::new(
            RawView::new(column(0.0))
                .background(theme.colors.surface_elevated)
                .outline(theme.colors.border, 1.0)
                .corner_radius(16.0)
                .with_children(children),
        ),
    )
}

fn labeled(title: &str, content: BoxedWidget) -> BoxedWidget {
    view(
        column(8.0),
        vec![
            Box::new(
                Text::secondary(title.to_uppercase())
                    .font_size(11.0)
                    .bold(true)
                    .padding_left(4.0),
            ),
            content,
        ],
    )
}

fn icon_badge(state: &State, icon: &str) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(Style {
            justify_content: Some(JustifyContent::Center),
            ..fixed(30.0, 30.0)
        })
        .background(theme.colors.surface.mix(theme.colors.accent, 0.12))
        .corner_radius(9.0)
        .child(Box::new(
            Icon::new(state.icons.get(icon), theme.colors.accent).size(16.0),
        )),
    )
}

/// Keeps the document readable on wide windows.
fn document(gap: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        max_size: creamui_core::layout::Size {
            width: Dimension::Length(DOCUMENT_WIDTH),
            height: Dimension::Auto,
        },
        align_self: Some(creamui_core::layout::AlignSelf::Center),
        flex_shrink: 0.0,
        ..column(gap)
    }
}
