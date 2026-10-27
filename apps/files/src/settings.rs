//! The in-process preferences window launched by `cfiles --settings`.

use crate::components::CLEAR;
use crate::{
    config::{Config, ItemSize, TabPosition},
    shortcuts::{Action, Binding},
};
use creamui_core::{
    layout::{AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, Style},
    BoxedWidget, LinearGradient, Size, StateStyle, Styled,
};
use creamui_reactive::Signal;
use creamui_render::platform::{Key, KeyEvent, Modifiers};
use creamui_theme::use_theme;
use creamui_widgets::{
    tab_styles, AppTitlebar, Icon, RawButton, RawScrollView, RawView, ScrollController,
    SegmentedControl, Switch, Symbol, Tab, TabColors, TabSizing, Tabs, Text,
};
use std::{cell::RefCell, rc::Rc};

mod menu;
mod preview;

const HEADER_HEIGHT: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Browsing,
    Layout,
    Shortcuts,
    About,
}

#[derive(Clone)]
pub(crate) struct State {
    config: Signal<Config>,
    page: Signal<Page>,
    message: Signal<Option<String>>,
    scroll: ScrollController,
    icons: crate::icons::Icons,
    menu_open: Signal<bool>,
    reset_open: Signal<bool>,
    capturing: Signal<bool>,
    pub(crate) window: Rc<RefCell<Option<creamui_render::WindowHandle>>>,
}

impl State {
    pub(crate) fn load() -> Self {
        Self {
            config: Signal::new(Config::load()),
            page: Signal::new(Page::Browsing),
            message: Signal::new(None),
            scroll: ScrollController::new(0.0),
            icons: crate::icons::Icons::load(),
            menu_open: Signal::new(false),
            reset_open: Signal::new(false),
            capturing: Signal::new(false),
            window: Rc::new(RefCell::new(None)),
        }
    }

    fn persist(&self) {
        match self.config.peek().save() {
            Ok(()) => self.message.set(None),
            Err(error) => self
                .message
                .set(Some(format!("Couldn't save preferences: {error}"))),
        }
    }
    pub(crate) fn key(&self, event: KeyEvent, modifiers: Modifiers) -> bool {
        if !self.capturing.peek() {
            return false;
        }
        if !event.pressed {
            return true;
        }
        match event.key {
            Key::Escape => {
                self.capturing.set(false);
                self.message.set(None);
            }
            Key::Backspace | Key::Delete if modifiers == Modifiers::default() => {
                self.config.update(|config| {
                    config
                        .shortcuts
                        .insert(Action::Search.id().into(), String::new());
                });
                self.capturing.set(false);
                self.persist();
            }
            _ => {
                if let Some(binding) = Binding::from_event(&event.key, modifiers) {
                    self.config.update(|config| {
                        config
                            .shortcuts
                            .insert(Action::Search.id().into(), binding.display());
                    });
                    self.capturing.set(false);
                    self.persist();
                }
            }
        }
        true
    }
}

pub(crate) fn build(state: &State, size: Size) -> BoxedWidget {
    let theme = use_theme();
    let page = state.page.get();
    let (title, description, body) = match page {
        Page::Browsing => (
            "Browsing",
            "Choose how folders open and what they show.",
            browsing(state),
        ),
        Page::Layout => (
            "Window layout",
            "Arrange your tabs and status information.",
            window_layout(state),
        ),
        Page::Shortcuts => (
            "Keyboard shortcuts",
            "Choose the keys used by File Explorer.",
            shortcuts(state),
        ),
        Page::About => (
            "About",
            "File Explorer",
            coconut_ui::about!("File Explorer", Symbol::Folder),
        ),
    };
    let mut document_children = if page == Page::About {
        vec![body]
    } else {
        vec![
            page_heading(state, title, description),
            body,
            Box::new(
                Text::secondary("Changes are saved automatically for new windows.").font_size(12.0),
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
    let mut document_style = pad(document(20.0), content_inset(size.width), 24.0);
    document_style.padding.bottom = LengthPercentage::Length(16.0);
    let content = RawScrollView::controlled(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..column(0.0)
        },
        state.scroll.clone(),
    )
    .scrollbar_width(5.0)
    .scrollbar_margin(4.0)
    .child(Box::new(
        RawView::new(document_style).with_children(document_children),
    ));
    let mut children = vec![
        titlebar(state, page, size.width),
        Box::new(content) as BoxedWidget,
    ];
    if state.menu_open.get() {
        children.push(menu::options(state, size.width));
    }
    if state.reset_open.get() {
        children.push(menu::reset_dialog(state));
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
            theme.colors.surface.mix(theme.colors.accent, 0.055),
        ))
        .with_children(children),
    )
}

fn titlebar(state: &State, page: Page, width: f32) -> BoxedWidget {
    let titlebar = AppTitlebar::new(width)
        .height(HEADER_HEIGHT)
        .center_margin(16.0);
    let (start, end) = titlebar.reserved();
    let wanted = tab_styles(
        &["Browsing", "Window layout", "Shortcuts", "About"],
        TabSizing::Equal,
        34.0,
        16.0,
    )
    .iter()
    .map(|s| {
        if let Dimension::Length(w) = s.size.width {
            w
        } else {
            0.0
        }
    })
    .sum::<f32>()
        + 24.0;
    let left = start + 8.0;
    let right = (width - end - 8.0).max(left);
    let symmetric = 2.0 * (width / 2.0 - left).min(right - width / 2.0).max(0.0);
    let available = wanted
        .min(if symmetric >= 160.0 {
            symmetric
        } else {
            right - left
        })
        .max(0.0);
    Box::new(titlebar.center_boxed(Box::new(
        RawView::new(fixed(available, 42.0)).child(navigation(state, page, available)),
    )))
}

fn navigation(state: &State, active: Page, available_width: f32) -> BoxedWidget {
    let theme = use_theme();
    let labels = [
        "Browsing",
        if available_width < 440.0 {
            "Layout"
        } else {
            "Window layout"
        },
        "Shortcuts",
        "About",
    ];
    let mut styles = tab_styles(&labels, TabSizing::Equal, 34.0, 16.0);
    for style in &mut styles {
        // Tab also passes this layout to its label. The fixed width already
        // includes the inset, so avoid applying that padding twice.
        style.padding = Style::default().padding;
        if let Dimension::Length(width) = style.size.width {
            style.size.width =
                Dimension::Length(width.min(((available_width - 32.0) / 4.0).max(0.0)));
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
    for (index, page) in [Page::Browsing, Page::Layout, Page::Shortcuts, Page::About]
        .into_iter()
        .enumerate()
    {
        let state = state.clone();
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            labels[index],
            active == page,
            move || {
                state.scroll.set(0.0);
                state.menu_open.set(false);
                state.page.set(page);
            },
        )));
    }
    Box::new(tabs)
}

fn page_heading(state: &State, title: &str, description: &str) -> BoxedWidget {
    let theme = use_theme();
    let open_menu = state.clone();
    Box::new(
        RawView::new(Style {
            align_items: Some(AlignItems::Center),
            ..row(16.0)
        })
        .with_children(vec![
            Box::new(
                RawView::new(Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    ..column(5.0)
                })
                .with_children(vec![
                    Box::new(Text::new(title).font_size(24.0).bold(true)),
                    Box::new(Text::secondary(description).font_size(13.0)),
                ]),
            ),
            Box::new(
                RawButton::new(
                    Style {
                        align_items: Some(AlignItems::Center),
                        justify_content: Some(JustifyContent::Center),
                        ..fixed(32.0, 32.0)
                    },
                    move || open_menu.menu_open.set(true),
                )
                .background(theme.colors.surface_elevated)
                .corner_radius(10.0)
                .outline(theme.colors.border, 1.0)
                .hover_style(StateStyle::new().background(theme.colors.surface_hover))
                .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
                .child(Box::new(Text::secondary("•••").font_size(14.0))),
            ),
        ]),
    )
}

fn browsing(state: &State) -> BoxedWidget {
    let config = state.config.get();
    let views = Box::new(RawView::new(row(12.0)).with_children(vec![
        view_choice(state, false, "List", "Details at a glance"),
        view_choice(state, true, "Grid", "Room for your icons"),
    ])) as BoxedWidget;
    let hidden = Switch::new(config.explorer.show_hidden, {
        let state = state.clone();
        move || {
            state
                .config
                .update(|config| config.explorer.show_hidden = !config.explorer.show_hidden);
            state.persist();
        }
    });
    let item_size = SegmentedControl::new(
        match config.explorer.item_size {
            ItemSize::Compact => 0,
            ItemSize::Default => 1,
            ItemSize::Large => 2,
        },
        {
            let state = state.clone();
            move |index| {
                state.config.update(|config| {
                    config.explorer.item_size = match index {
                        0 => ItemSize::Compact,
                        2 => ItemSize::Large,
                        _ => ItemSize::Default,
                    }
                });
                state.persist();
            }
        },
    )
    .option("Compact")
    .option("Default")
    .option("Large");
    let columns = section(
        "Details columns",
        vec![
            column_switch(state, "Name", "File or folder name.", |c| &mut c.name),
            column_switch(state, "Size", "File size.", |c| &mut c.size),
            column_switch(state, "Type", "Folder or file type.", |c| &mut c.kind),
            column_switch(state, "Modification date", "Most recent change.", |c| {
                &mut c.modified
            }),
            column_switch(state, "Creation date", "When the item was created.", |c| {
                &mut c.created
            }),
            column_switch(state, "Permissions", "Unix access mode.", |c| {
                &mut c.permissions
            }),
        ],
    );
    Box::new(RawView::new(column(22.0)).with_children(vec![
        labeled("Default view", views),
        labeled("Item size", Box::new(item_size)),
        section(
            "Files & folders",
            vec![preference_row(
                Symbol::Folder,
                "Show hidden files",
                "Include dot-prefixed files and folders.",
                Box::new(hidden),
            )],
        ),
        columns,
    ]))
}

fn column_switch(
    state: &State,
    title: &str,
    description: &str,
    field: fn(&mut crate::config::DetailColumns) -> &mut bool,
) -> BoxedWidget {
    let enabled = *field(&mut state.config.get().explorer.columns.clone());
    let switch = Switch::new(enabled, {
        let state = state.clone();
        move || {
            state.config.update(|config| {
                let value = field(&mut config.explorer.columns);
                *value = !*value;
            });
            state.persist();
        }
    });
    preference_row(Symbol::Display, title, description, Box::new(switch))
}

fn view_choice(state: &State, grid: bool, title: &str, hint: &str) -> BoxedWidget {
    let theme = use_theme();
    let selected = state.config.get().explorer.default_grid_view == grid;
    let choose = state.clone();
    let mut sample = state.config.get().explorer;
    sample.default_grid_view = grid;
    let check: BoxedWidget = if selected {
        Box::new(Icon::new(Symbol::Check, theme.colors.accent).size(16.0))
    } else {
        Box::new(RawView::new(fixed(16.0, 16.0)))
    };
    Box::new(
        RawButton::new(
            pad(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..column(12.0)
                },
                12.0,
                12.0,
            ),
            move || {
                choose
                    .config
                    .update(|config| config.explorer.default_grid_view = grid);
                choose.persist();
            },
        )
        .background(theme.colors.surface_elevated)
        .outline(
            if selected {
                theme.colors.accent
            } else {
                theme.colors.border
            },
            if selected { 1.5 } else { 1.0 },
        )
        .corner_radius(16.0)
        .hover_style(
            StateStyle::new()
                .background(theme.colors.surface_elevated.mix(theme.colors.accent, 0.06)),
        )
        .focus_style(StateStyle::new().outline(theme.colors.accent, 2.0))
        .with_children(vec![
            Box::new(preview::ExplorerPreview::view(&state.icons, sample)),
            Box::new(
                RawView::new(Style {
                    align_items: Some(AlignItems::Center),
                    ..row(8.0)
                })
                .with_children(vec![
                    Box::new(
                        RawView::new(Style {
                            flex_grow: 1.0,
                            flex_basis: Dimension::Length(0.0),
                            ..column(3.0)
                        })
                        .with_children(vec![
                            Box::new(Text::new(title).font_size(13.0).bold(true)),
                            Box::new(Text::secondary(hint).font_size(11.0)),
                        ]),
                    ),
                    check,
                ]),
            ),
        ]),
    )
}

fn window_layout(state: &State) -> BoxedWidget {
    let config = state.config.get();
    let position = SegmentedControl::new(
        usize::from(config.explorer.tab_position == TabPosition::Bottom),
        {
            let state = state.clone();
            move |index| {
                state.config.update(|config| {
                    config.explorer.tab_position = if index == 0 {
                        TabPosition::Top
                    } else {
                        TabPosition::Bottom
                    };
                });
                state.persist();
            }
        },
    )
    .option("Top")
    .option("Bottom");
    let statusbar = Switch::new(config.explorer.show_statusbar, {
        let state = state.clone();
        move || {
            state
                .config
                .update(|config| config.explorer.show_statusbar = !config.explorer.show_statusbar);
            state.persist();
        }
    });
    let controls = section(
        "Window elements",
        vec![
            preference_row(
                Symbol::Controls,
                "Tab position",
                "Place tabs above or below your files.",
                Box::new(position),
            ),
            preference_row(
                Symbol::Display,
                "Show status bar",
                "Selection details and the item count.",
                Box::new(statusbar),
            ),
        ],
    );
    Box::new(RawView::new(column(22.0)).with_children(vec![
        Box::new(RawView::new(pad(column(0.0), 12.0, 12.0))
            .background(use_theme().colors.surface_elevated)
            .outline(use_theme().colors.border, 1.0)
            .corner_radius(16.0)
            .child(Box::new(preview::ExplorerPreview::new(&state.icons, config.explorer, 152.0)))),
        controls,
    ]))
}

fn shortcuts(state: &State) -> BoxedWidget {
    let theme = use_theme();
    let action = Action::Search;
    let capturing = state.capturing.get();
    let binding = state
        .config
        .get()
        .shortcuts
        .get(action.id())
        .cloned()
        .unwrap_or_else(|| action.default_binding().into());
    let text = if capturing {
        "Press shortcut…".into()
    } else if binding.is_empty() {
        "Disabled".into()
    } else {
        binding.replace('+', " + ")
    };
    let capture = state.clone();
    let control = RawButton::new(
        Style {
            justify_content: Some(JustifyContent::Center),
            ..fixed(156.0, 34.0)
        },
        move || {
            capture.capturing.set(true);
            capture.message.set(None);
        },
    )
    .background(if capturing {
        theme.colors.selection_background
    } else {
        CLEAR
    })
    .corner_radius(8.0)
    .hover_style(StateStyle::new().background(theme.colors.surface_hover))
    .focus_style(StateStyle::new().outline(theme.colors.accent, 1.0))
    .child(Box::new(Text::secondary(text).font_size(12.0)));
    let details = RawView::new(Style {
        flex_grow: 1.0,
        flex_basis: Dimension::Length(0.0),
        ..column(3.0)
    })
    .with_children(vec![
        Box::new(Text::new(action.label()).font_size(13.0)),
        Box::new(Text::secondary("Show or hide search in the current folder.").font_size(11.0)),
    ]);
    let shortcut_row = RawView::new(pad(
        Style {
            align_items: Some(AlignItems::Center),
            ..row(12.0)
        },
        16.0,
        12.0,
    ))
    .with_children(vec![
        icon_badge(Symbol::Keyboard),
        Box::new(details),
        Box::new(control),
    ]);
    let group = RawView::new(column(0.0))
        .background(theme.colors.surface_elevated)
        .outline(theme.colors.border, 1.0)
        .corner_radius(16.0)
        .child(Box::new(shortcut_row));
    Box::new(RawView::new(column(22.0)).with_children(vec![
        labeled("File Explorer", Box::new(group)),
        Box::new(Text::secondary("Click the shortcut, then press a key combination. Escape cancels; Backspace clears.").font_size(12.0)),
    ]))
}

fn preference_row(
    icon: Symbol,
    title: &str,
    description: &str,
    control: BoxedWidget,
) -> BoxedWidget {
    Box::new(
        RawView::new(pad(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(62.0),
                },
                flex_shrink: 0.0,
                align_items: Some(AlignItems::Center),
                ..row(12.0)
            },
            16.0,
            12.0,
        ))
        .with_children(vec![
            icon_badge(icon),
            Box::new(
                RawView::new(Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..column(3.0)
                })
                .with_children(vec![
                    Box::new(Text::new(title).font_size(13.0)),
                    Box::new(Text::secondary(description).font_size(11.0)),
                ]),
            ),
            Box::new(
                RawView::new(Style {
                    flex_shrink: 0.0,
                    align_items: Some(AlignItems::Center),
                    ..row(0.0)
                })
                .child(control),
            ),
        ]),
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
    Box::new(RawView::new(column(8.0)).with_children(vec![
            Box::new(
                Text::secondary(title.to_uppercase())
                    .font_size(11.0)
                    .bold(true)
                    .padding_left(4.0),
            ),
            content,
        ]))
}

fn icon_badge(icon: Symbol) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(Style {
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            ..fixed(28.0, 28.0)
        })
        .background(theme.colors.surface.mix(theme.colors.accent, 0.1))
        .corner_radius(8.0)
        .child(Box::new(Icon::new(icon, theme.colors.accent).size(16.0))),
    )
}

fn content_inset(width: f32) -> f32 {
    if width < 600.0 {
        24.0
    } else {
        40.0
    }
}

/// Keep the document readable on wide windows.
fn document(gap: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        max_size: creamui_core::layout::Size {
            width: Dimension::Length(760.0),
            height: Dimension::Auto,
        },
        align_self: Some(creamui_core::layout::AlignSelf::Center),
        flex_shrink: 0.0,
        ..column(gap)
    }
}

fn row(gap: f32) -> Style {
    Style {
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        ..Default::default()
    }
}

fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        ..row(gap)
    }
}

fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}

fn pad(mut style: Style, horizontal: f32, vertical: f32) -> Style {
    style.padding = creamui_core::layout::Rect {
        left: LengthPercentage::Length(horizontal),
        right: LengthPercentage::Length(horizontal),
        top: LengthPercentage::Length(vertical),
        bottom: LengthPercentage::Length(vertical),
    };
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Point, Rect, Renderer, TitlebarMode, TitlebarState, WindowDragHandle};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Theme, ThemeProvider};
    use std::{cell::Cell, rc::Rc};

    fn tab_point(scene: &creamui_core::Scene, width: u32, index: usize) -> Point {
        let xs = (0..width)
            .filter(|x| {
                scene.focus_hit_test(Point {
                    x: *x as f32,
                    y: 32.0,
                }) == Some(index)
            })
            .collect::<Vec<_>>();
        Point {
            x: (xs.first().expect("tab has a visible hit region") + xs.last().unwrap()) as f32
                / 2.0,
            y: 32.0,
        }
    }

    #[test]
    fn preferences_tabs_render_and_navigate_without_changing_config() {
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                crate::test_window::TestWindow::install(TitlebarState {
                    mode: TitlebarMode::Pending,
                    ..Default::default()
                });
                let moves = Rc::new(Cell::new(0));
                creamui_reactive::provide_context(WindowDragHandle::new({
                    let moves = moves.clone();
                    move || moves.set(moves.get() + 1)
                }));
                let state = State {
                    config: Signal::new(Config::default()),
                    page: Signal::new(Page::Browsing),
                    message: Signal::new(None),
                    scroll: ScrollController::new(0.0),
                    icons: crate::icons::Icons::load(),
                    menu_open: Signal::new(false),
                    reset_open: Signal::new(false),
                    capturing: Signal::new(false),
                    window: Rc::new(RefCell::new(None)),
                };
                for (width, height) in [(760, 560), (480, 420), (960, 640)] {
                    let size = Size {
                        width: width as f32,
                        height: height as f32,
                    };
                    state.page.set(Page::Browsing);
                    for (name, expected, next) in [
                        ("browsing", Page::Browsing, Page::Layout),
                        ("layout", Page::Layout, Page::Browsing),
                    ] {
                        assert!(state.page.peek() == expected);
                        let mut recorder = SceneRecorder::new();
                        recorder.begin(width, height, 1.0, CLEAR, theme.colors);
                        let scene =
                            Renderer::new().render(build(&state, size), size, &mut recorder);
                        let list = recorder.finish();
                        // Four tabs, two choices, one switch and the options menu
                        // remain available to keyboard navigation on either page.
                        if height >= 560 {
                            assert!(
                                scene.focus_id_at(7).is_some(),
                                "all controls visible in {name}/{width}"
                            );
                            assert!(scene.focus_id_at(8).is_none());
                        } else {
                            assert!(
                                scene.focus_id_at(4).is_some(),
                                "top controls visible in {name}/{width}"
                            );
                        }
                        // Padding and unused header space move the window;
                        // tabs keep their own click and keyboard behavior.
                        for point in [Point { x: 8.0, y: 32.0 }, Point { x: 80.0, y: 8.0 }] {
                            let before = moves.get();
                            let drag = scene
                                .window_drag_at(point)
                                .expect("header background drags the window");
                            drag();
                            assert_eq!(moves.get(), before + 1);
                            assert!(scene.hit_test(point).is_none());
                        }
                        assert!(scene.window_drag_at(tab_point(&scene, width, 0)).is_none());
                        assert!(scene.window_drag_at(Point { x: 32.0, y: 100.0 }).is_none());
                        for x in (width - 128..width).step_by(8) {
                            assert!(
                                scene
                                    .hit_test(Point {
                                        x: x as f32,
                                        y: 32.0
                                    })
                                    .is_none(),
                                "right edge stays free for compositor controls: {appearance}/{name}/{width}/{x}"
                            );
                        }
                        if let Some(directory) = std::env::var_os("CFILES_SETTINGS_SNAPSHOT_DIR") {
                            let path = std::path::PathBuf::from(directory)
                                .join(format!("{appearance}-{name}-{width}.png"));
                            let mut raster = Rasterizer::new(width, height);
                            raster.render(&list, &Damage::Full);
                            raster
                                .pixmap()
                                .save_png(path)
                                .expect("save settings preview");
                        }
                        state.scroll.set(50.0);
                        scene
                            .hit_test(tab_point(&scene, width, usize::from(next == Page::Layout)))
                            .expect("top tab is clickable")();
                        assert!(state.page.peek() == next);
                        assert_eq!(state.scroll.peek(), 0.0);
                        assert_eq!(state.config.peek(), Config::default());
                        assert!(state.message.peek().is_none());
                    }
                }
            });
        }
    }

    #[test]
    fn about_tab_shows_package_information_and_project_links() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let state = State {
                config: Signal::new(Config::default()),
                page: Signal::new(Page::Browsing),
                message: Signal::new(None),
                scroll: ScrollController::new(0.0),
                icons: crate::icons::Icons::load(),
                menu_open: Signal::new(false),
                reset_open: Signal::new(false),
                capturing: Signal::new(false),
                window: Rc::new(RefCell::new(None)),
            };
            let size = Size {
                width: 760.0,
                height: 640.0,
            };
            let mut recorder = SceneRecorder::new();
            recorder.begin(760, 640, 1.0, CLEAR, theme.colors);
            let scene = Renderer::new().render(build(&state, size), size, &mut recorder);
            scene
                .on_key_at(3)
                .expect("About tab is keyboard accessible")(creamui_core::KeyInput {
                key: creamui_core::Key::Enter,
                modifiers: Default::default(),
            });
            assert!(state.page.peek() == Page::About);
            let mut recorder = SceneRecorder::new();
            recorder.begin(760, 640, 1.0, CLEAR, theme.colors);
            let scene = Renderer::new().render(build(&state, size), size, &mut recorder);
            assert!(
                scene.focus_id_at(7).is_some(),
                "four tabs and four project links"
            );
            assert!(scene.focus_id_at(8).is_none());
            assert_eq!(state.config.peek(), Config::default());
            if let Some(dir) = std::env::var_os("CFILES_SETTINGS_SNAPSHOT_DIR") {
                let mut raster = Rasterizer::new(760, 640);
                raster.render(&recorder.finish(), &Damage::Full);
                raster
                    .pixmap()
                    .save_png(std::path::PathBuf::from(dir).join("cfiles-about.png"))
                    .unwrap();
            }
        });
    }

    #[test]
    fn header_keeps_injected_controls_clear_on_either_side() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let window = crate::test_window::TestWindow::install(TitlebarState::default());
            let state = State {
                config: Signal::new(Config::default()),
                page: Signal::new(Page::Browsing),
                message: Signal::new(None),
                scroll: ScrollController::new(0.0),
                icons: crate::icons::Icons::load(),
                menu_open: Signal::new(false),
                reset_open: Signal::new(false),
                capturing: Signal::new(false),
                window: Rc::new(RefCell::new(None)),
            };
            for (x, width) in [(240, 224), (8, 112)] {
                window.set(
                    TitlebarMode::Overlay,
                    Rect {
                        x: x as f32,
                        y: 14.0,
                        width: width as f32,
                        height: 32.0,
                    },
                );
                let size = Size {
                    width: 480.0,
                    height: HEADER_HEIGHT,
                };
                let mut recorder = SceneRecorder::new();
                recorder.begin(480, HEADER_HEIGHT as u32, 1.0, CLEAR, theme.colors);
                let scene = Renderer::new().render(
                    titlebar(&state, Page::Browsing, size.width),
                    size,
                    &mut recorder,
                );
                for control_x in (x..x + width).step_by(8) {
                    assert!(scene
                        .hit_test(Point {
                            x: control_x as f32,
                            y: 32.0
                        })
                        .is_none());
                }
                scene
                    .hit_test(tab_point(&scene, 480, 1))
                    .expect("layout tab stays accessible")();
                assert!(state.page.peek() == Page::Layout);
                state.page.set(Page::Browsing);
            }
        });
    }

    #[test]
    fn reset_is_only_available_from_options_and_can_be_cancelled() {
        use creamui_core::{Key, KeyInput};
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let mut config = Config::default();
            config.explorer.show_hidden = true;
            let state = State {
                config: Signal::new(config.clone()),
                page: Signal::new(Page::Browsing),
                message: Signal::new(None),
                scroll: ScrollController::new(0.0),
                icons: crate::icons::Icons::load(),
                menu_open: Signal::new(false),
                reset_open: Signal::new(false),
                capturing: Signal::new(false),
                window: Rc::new(RefCell::new(None)),
            };
            let size = Size {
                width: 760.0,
                height: 560.0,
            };
            let render = |name: &str| {
                let mut recorder = SceneRecorder::new();
                recorder.begin(760, 560, 1.0, CLEAR, theme.colors);
                let scene = Renderer::new().render(build(&state, size), size, &mut recorder);
                if let Some(directory) = std::env::var_os("CFILES_SETTINGS_SNAPSHOT_DIR") {
                    let mut raster = Rasterizer::new(760, 560);
                    raster.render(&recorder.finish(), &Damage::Full);
                    raster
                        .pixmap()
                        .save_png(std::path::PathBuf::from(directory).join(format!("{name}.png")))
                        .unwrap();
                }
                scene
            };
            let scene = render("options-closed");
            assert!(
                scene.hit_test(Point { x: 650.0, y: 535.0 }).is_none(),
                "no permanent reset footer"
            );
            scene
                .hit_test(Point { x: 704.0, y: 112.0 })
                .expect("options button")();
            assert!(state.menu_open.peek());
            let scene = render("options-open");
            scene.on_key_at(scene.modal_focus().unwrap()).unwrap()(KeyInput {
                key: Key::Escape,
                modifiers: Default::default(),
            });
            assert!(!state.menu_open.peek());
            state.menu_open.set(true);
            let scene = render("options-dismiss");
            scene
                .hit_test(Point { x: 50.0, y: 300.0 })
                .expect("outside click")();
            assert!(!state.menu_open.peek());
            state.menu_open.set(true);
            let scene = render("options-restore");
            scene
                .hit_test(Point { x: 600.0, y: 165.0 })
                .expect("restore menu item")();
            assert!(!state.menu_open.peek());
            assert!(state.reset_open.peek());
            assert_eq!(
                state.config.peek(),
                config,
                "opening reset never changes preferences"
            );
            let scene = render("reset-confirmation");
            scene.on_key_at(scene.modal_focus().unwrap()).unwrap()(KeyInput {
                key: Key::Enter,
                modifiers: Default::default(),
            });
            assert!(!state.reset_open.peek());
            assert_eq!(state.config.peek(), config);
            // Also preview the tighter combination of bottom tabs, hidden
            // folders and grid view without writing any user preferences.
            state.page.set(Page::Layout);
            state.config.update(|config| {
                config.explorer.default_grid_view = true;
                config.explorer.tab_position = TabPosition::Bottom;
                config.explorer.show_statusbar = false;
            });
            render("layout-grid-bottom");
        });
    }

    #[test]
    fn settings_draw_client_controls_only_for_client_decorations() {
        use creamui_render::WindowDecorationMode;
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            creamui_reactive::provide_context(WindowDragHandle::new(|| {}));
            let window = crate::test_window::TestWindow::install(TitlebarState::default());
            let state = State {
                config: Signal::new(Config::default()),
                page: Signal::new(Page::Browsing),
                message: Signal::new(None),
                scroll: ScrollController::new(0.0),
                icons: crate::icons::Icons::load(),
                menu_open: Signal::new(false),
                reset_open: Signal::new(false),
                capturing: Signal::new(false),
                window: Rc::new(RefCell::new(None)),
            };
            for width in [480, 760, 960] {
                for mode in [
                    WindowDecorationMode::Pending,
                    WindowDecorationMode::Client,
                    WindowDecorationMode::Hybrid,
                    WindowDecorationMode::Server,
                    WindowDecorationMode::None,
                    WindowDecorationMode::Client,
                ] {
                    window.set(
                        match mode {
                            WindowDecorationMode::Pending => TitlebarMode::Pending,
                            WindowDecorationMode::Client => TitlebarMode::Client,
                            WindowDecorationMode::Hybrid => TitlebarMode::Overlay,
                            WindowDecorationMode::Server => TitlebarMode::Server,
                            WindowDecorationMode::None => TitlebarMode::None,
                        },
                        Rect {
                            x: (width - 128) as f32,
                            y: 14.0,
                            width: 112.0,
                            height: 28.0,
                        },
                    );
                    let size = Size {
                        width: width as f32,
                        height: HEADER_HEIGHT,
                    };
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(width as u32, HEADER_HEIGHT as u32, 1.0, CLEAR, theme.colors);
                    let scene = Renderer::new().render(
                        titlebar(&state, Page::Browsing, size.width),
                        size,
                        &mut recorder,
                    );
                    let is_client = mode == WindowDecorationMode::Client;
                    assert!(scene.focus_id_at(if is_client { 6 } else { 3 }).is_some());
                    assert!(scene.focus_id_at(if is_client { 7 } else { 4 }).is_none());
                    for offset in [32.0, 68.0, 104.0] {
                        let point = Point {
                            x: size.width - offset,
                            y: 32.0,
                        };
                        if matches!(
                            mode,
                            WindowDecorationMode::Pending
                                | WindowDecorationMode::Hybrid
                                | WindowDecorationMode::Client
                        ) {
                            assert_eq!(
                                scene.hit_test(point).is_some(),
                                is_client,
                                "{mode:?}/{width}/{offset}"
                            );
                        }
                        if is_client {
                            assert!(scene.window_drag_at(point).is_none());
                        }
                    }
                    assert!(scene.window_drag_at(Point { x: 8.0, y: 32.0 }).is_some());
                    if width == 760 {
                        if let Some(directory) = std::env::var_os("CFILES_SETTINGS_SNAPSHOT_DIR") {
                            let mut recorder = SceneRecorder::new();
                            recorder.begin(760, 560, 1.0, CLEAR, theme.colors);
                            Renderer::new().render(
                                build(
                                    &state,
                                    Size {
                                        width: 760.0,
                                        height: 560.0,
                                    },
                                ),
                                Size {
                                    width: 760.0,
                                    height: 560.0,
                                },
                                &mut recorder,
                            );
                            let mut raster = Rasterizer::new(760, 560);
                            raster.render(&recorder.finish(), &Damage::Full);
                            raster
                                .pixmap()
                                .save_png(
                                    std::path::PathBuf::from(directory)
                                        .join(format!("decorations-{mode:?}.png")),
                                )
                                .unwrap();
                        }
                    }
                }
            }
        });
    }
}
