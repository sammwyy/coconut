use crate::components::{fixed_body, group, row, section, tab_colors, update_config};
use coconut_core::{DockAlign, DockConfig, DockPosition, ShellConfig};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_reactive::Signal;
use creamui_widgets::layout::Flex;
use creamui_widgets::{
    tab_styles, ScrollController, SegmentedControl, Select, Switch, Tab, TabSizing, Tabs, Text,
    TextSize,
};

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

pub use coconut_core::BarKind;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DockTab {
    Display,
    Appearance,
    Content,
}

pub fn build_bar_page(
    size: Size,
    config: &Signal<ShellConfig>,
    scroll: &ScrollController,
    kind: BarKind,
) -> BoxedWidget {
    let router = creamui_router::use_router();
    let tab = match router.location().query("tab").as_deref() {
        Some("appearance") => DockTab::Appearance,
        Some("position") => DockTab::Display,
        _ => DockTab::Content,
    };
    let bar = kind.get(&config.get()).clone();
    let body = match tab {
        DockTab::Display => display_card(config, kind, &bar, size.width < 880.0),
        DockTab::Appearance => appearance_card(config, kind, &bar),
        DockTab::Content => {
            let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
            super::bar_content::build(&context, kind, &bar)
        }
    };
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    fixed_body(
        tabs(&router, tab, size.width < 880.0),
        body,
        scroll.clone(),
        move |scroll| {
            let style = scroll.style();
            super::bar_drag::measured(
                &context.bar_content.drag,
                super::bar_drag::Region::Viewport,
                style,
                scroll,
                "bar-scroll".into(),
            )
        },
    )
}

fn tabs(router: &creamui_router::Router, active: DockTab, compact: bool) -> BoxedWidget {
    let labels = ["Contents", "Layout", "Style"];
    let styles = tab_styles(
        &labels,
        TabSizing::Fill,
        38.0,
        if compact { 4.0 } else { 12.0 },
    );
    let mut tabs = Tabs::new(tab_colors(), Default::default()).gap(8.0);
    for (index, label) in labels.into_iter().enumerate() {
        let target = [DockTab::Content, DockTab::Display, DockTab::Appearance][index];
        let router = router.clone();
        tabs = tabs.child(Box::new(Tab::new(
            tab_colors(),
            styles[index].clone(),
            label,
            active == target,
            move || {
                crate::routes::set_query(
                    &router,
                    "tab",
                    ["contents", "position", "appearance"][index],
                )
            },
        )));
    }
    Box::new(tabs)
}

fn display_card(
    config: &Signal<ShellConfig>,
    kind: BarKind,
    bar: &DockConfig,
    compact: bool,
) -> BoxedWidget {
    let enabled: BoxedWidget = Box::new(Switch::new(bar.enabled, {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                let bar = kind.get_mut(c);
                bar.enabled = !bar.enabled;
            })
        }
    }));
    let selected = POSITIONS
        .iter()
        .position(|position| *position == bar.position)
        .unwrap_or(0);
    let apply = config.clone();
    let change = move |index| {
        update_config(&apply, |config| {
            kind.get_mut(config).position = POSITIONS[index]
        })
    };
    let placement: BoxedWidget = if compact {
        let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
        Box::new(
            Select::controlled(
                &["Top", "Bottom", "Left", "Right"],
                context
                    .native
                    .select(&format!("{}-edge", kind.title()), selected),
            )
            .width(160.0)
            .on_select(change),
        )
    } else {
        Box::new(
            SegmentedControl::new(selected, change)
                .option("Top")
                .option("Bottom")
                .option("Left")
                .option("Right"),
        )
    };
    let thickness = number_choice(
        config,
        kind,
        "Thickness",
        bar.thickness,
        &[28.0, 36.0, 44.0, 56.0, 64.0, 76.0, 88.0, 100.0],
        |bar, value| bar.thickness = value,
    );
    let auto_hide: BoxedWidget = Box::new(Switch::new(bar.auto_hide, {
        let config = config.clone();
        move || {
            update_config(&config, |config| {
                let bar = kind.get_mut(config);
                bar.auto_hide = !bar.auto_hide;
            })
        }
    }));
    let fill: BoxedWidget = Box::new(Switch::new(bar.fill_available_space, {
        let config = config.clone();
        move || {
            update_config(&config, |config| {
                let bar = kind.get_mut(config);
                bar.fill_available_space = !bar.fill_available_space;
            })
        }
    }));
    let alignment: BoxedWidget = Box::new(
        SegmentedControl::new(
            match bar.align {
                DockAlign::Start => 0,
                DockAlign::Center => 1,
                DockAlign::End => 2,
            },
            {
                let config = config.clone();
                move |index| {
                    update_config(&config, |config| {
                        kind.get_mut(config).align =
                            [DockAlign::Start, DockAlign::Center, DockAlign::End][index]
                    })
                }
            },
        )
        .option("Start")
        .option("Center")
        .option("End"),
    );
    let margin = number_choice(
        config,
        kind,
        "Screen inset",
        bar.margin,
        &[0.0, 4.0, 8.0, 12.0, 20.0, 32.0, 48.0],
        |bar, value| bar.margin = value,
    );
    let edge_gap = number_choice(
        config,
        kind,
        "End padding",
        bar.edge_gap,
        &[0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 32.0],
        |bar, value| bar.edge_gap = value,
    );
    let section_gap = number_choice(
        config,
        kind,
        "Group spacing",
        bar.section_gap,
        &[0.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0],
        |bar, value| bar.section_gap = value,
    );
    let setting = |label: &str, control| {
        if compact {
            Box::new(
                Flex::column()
                    .gap(8.0)
                    .padding(16.0)
                    .child(Box::new(Text::new(label).size(TextSize::Sm)))
                    .child(control),
            ) as BoxedWidget
        } else {
            row(label, control)
        }
    };
    section(
        "",
        "Place the bar where it feels natural, then adjust how much space it uses.",
        vec![
            group(vec![
                row("Show bar", enabled),
                row("Auto-hide", auto_hide),
                setting("Screen edge", placement),
            ]),
            group(vec![
                setting("Thickness", thickness),
                row("Fill available space", fill),
                setting("Alignment", alignment),
                setting("Screen inset", margin),
                setting("End padding", edge_gap),
                setting("Group spacing", section_gap),
            ]),
        ],
    )
}

fn number_choice(
    config: &Signal<ShellConfig>,
    kind: BarKind,
    label: &str,
    value: f32,
    choices: &[f32],
    set: fn(&mut DockConfig, f32),
) -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let mut choices = choices.to_vec();
    if !choices.contains(&value) {
        choices.push(value);
        choices.sort_by(f32::total_cmp);
    }
    let selected = choices
        .iter()
        .position(|choice| *choice == value)
        .unwrap_or(0);
    let labels = choices
        .iter()
        .map(|value| format!("{value} px"))
        .collect::<Vec<_>>();
    let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let config = config.clone();
    Box::new(
        Select::controlled(
            &labels,
            context
                .native
                .select(&format!("bar-{}-{label}", kind.title()), selected),
        )
        .width(138.0)
        .on_select(move |index| {
            update_config(&config, |config| set(kind.get_mut(config), choices[index]));
        }),
    )
}

fn appearance_card(config: &Signal<ShellConfig>, kind: BarKind, bar: &DockConfig) -> BoxedWidget {
    let toggle = |label, value, change: fn(&mut DockConfig)| -> BoxedWidget {
        let apply = config.clone();
        row(
            label,
            Box::new(Switch::new(value, move || {
                update_config(&apply, |c| change(kind.get_mut(c)))
            })),
        )
    };
    section(
        "Appearance",
        "Choose a continuous bar background or isolated island groups.",
        vec![group(vec![
            toggle("Bar background", bar.show_background, |bar| {
                bar.show_background = !bar.show_background
            }),
            toggle("Bar border", bar.show_border, |bar| {
                bar.show_border = !bar.show_border
            }),
            toggle("Island backgrounds", bar.show_island_background, |bar| {
                bar.show_island_background = !bar.show_island_background
            }),
            toggle("Island borders", bar.show_island_border, |bar| {
                bar.show_island_border = !bar.show_island_border
            }),
            toggle(
                "Join each island group",
                bar.unify_island_background,
                |bar| bar.unify_island_background = !bar.unify_island_background,
            ),
        ])],
    )
}

pub(crate) fn statusbar_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let config = &context.config;
    let dock_scroll = &context.dock_scroll;
    build_bar_page(size, config, dock_scroll, BarKind::Statusbar)
}

pub(crate) fn dock_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let config = &context.config;
    let dock_scroll = &context.dock_scroll;
    build_bar_page(size, config, dock_scroll, BarKind::Dockbar)
}
