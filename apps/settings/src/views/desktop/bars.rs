use crate::components::{fixed_body, group, row, section, tab_colors, update_config};
use coconut_core::{DockConfig, DockPosition, SectionConfig, ShellConfig};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{
    tab_styles, Button, ButtonSize, ButtonState, ButtonVariant, ScrollController, SegmentedControl,
    Switch, Tab, TabSizing, Tabs,
};

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BarKind {
    Statusbar,
    Dockbar,
}

impl BarKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Statusbar => "Statusbar",
            Self::Dockbar => "Dockbar",
        }
    }
    fn get(self, config: &ShellConfig) -> &DockConfig {
        match self {
            Self::Statusbar => &config.statusbar,
            Self::Dockbar => &config.dockbar,
        }
    }
    fn get_mut<'a>(self, config: &'a mut ShellConfig) -> &'a mut DockConfig {
        match self {
            Self::Statusbar => &mut config.statusbar,
            Self::Dockbar => &mut config.dockbar,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DockTab {
    Display,
    Appearance,
    Content,
}

pub fn build_bar_page(
    _: Size,
    config: &Signal<ShellConfig>,
    scroll: &ScrollController,
    tab: &Signal<DockTab>,
    kind: BarKind,
) -> BoxedWidget {
    let bar = kind.get(&config.get()).clone();
    let body = match tab.get() {
        DockTab::Display => display_card(config, kind, &bar),
        DockTab::Appearance => appearance_card(config, kind, &bar),
        DockTab::Content => content_card(config, kind, &bar),
    };
    fixed_body(tabs(tab, tab.get()), body, scroll.clone())
}

fn tabs(tab: &Signal<DockTab>, active: DockTab) -> BoxedWidget {
    let labels = ["Position", "Appearance", "Contents"];
    let styles = tab_styles(&labels, TabSizing::Fill, 38.0, 12.0);
    let mut tabs = Tabs::new(tab_colors(), Default::default()).gap(8.0);
    for (index, label) in labels.into_iter().enumerate() {
        let target = [DockTab::Display, DockTab::Appearance, DockTab::Content][index];
        let state = tab.clone();
        tabs = tabs.child(Box::new(Tab::new(
            tab_colors(),
            styles[index].clone(),
            label,
            active == target,
            move || state.set(target),
        )));
    }
    Box::new(tabs)
}

fn display_card(config: &Signal<ShellConfig>, kind: BarKind, bar: &DockConfig) -> BoxedWidget {
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
    let placement: BoxedWidget = Box::new(
        SegmentedControl::new(selected, {
            let config = config.clone();
            move |index| update_config(&config, |c| kind.get_mut(c).position = POSITIONS[index])
        })
        .option("Top")
        .option("Bottom")
        .option("Left")
        .option("Right"),
    );
    let thickness: BoxedWidget = Box::new(
        SegmentedControl::new(
            if bar.thickness >= 52.0 {
                2
            } else if bar.thickness >= 44.0 {
                1
            } else {
                0
            },
            {
                let config = config.clone();
                move |index| {
                    update_config(&config, |c| {
                        kind.get_mut(c).thickness = [36.0, 44.0, 56.0][index]
                    })
                }
            },
        )
        .option("Compact")
        .option("Normal")
        .option("Large"),
    );
    section(kind.title(), "This is one of Coconut's two fixed bars. You can move or hide it, but cannot create additional panels.", vec![group(vec![row("Enabled", enabled), row("Screen edge", placement), row("Thickness", thickness)])])
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

fn content_card(config: &Signal<ShellConfig>, kind: BarKind, bar: &DockConfig) -> BoxedWidget {
    let mut groups = Vec::new();
    for (section_index, section_config) in bar.sections.iter().enumerate() {
        let mut rows = Vec::new();
        for (island_index, island) in section_config.islands.iter().enumerate() {
            let label = island.id.clone();
            let mut controls = Flex::row().gap(6.0).align(Align::Center);
            if section_index > 0 {
                let apply = config.clone();
                controls = controls.child(Box::new(Button::styled(
                    ButtonVariant::Secondary,
                    ButtonSize::Sm,
                    "Previous group",
                    ButtonState::Normal,
                    move || {
                        update_config(&apply, |c| {
                            let sections = &mut kind.get_mut(c).sections;
                            if section_index > 0
                                && section_index < sections.len()
                                && island_index < sections[section_index].islands.len()
                            {
                                let island = sections[section_index].islands.remove(island_index);
                                sections[section_index - 1].islands.push(island);
                            }
                        })
                    },
                )));
            }
            if section_index + 1 < bar.sections.len() {
                let apply = config.clone();
                controls = controls.child(Box::new(Button::styled(
                    ButtonVariant::Secondary,
                    ButtonSize::Sm,
                    "Next group",
                    ButtonState::Normal,
                    move || {
                        update_config(&apply, |c| {
                            let sections = &mut kind.get_mut(c).sections;
                            if section_index + 1 < sections.len()
                                && island_index < sections[section_index].islands.len()
                            {
                                let island = sections[section_index].islands.remove(island_index);
                                sections[section_index + 1].islands.insert(0, island);
                            }
                        })
                    },
                )));
            }
            let apply = config.clone();
            controls = controls.child(Box::new(Button::styled(
                ButtonVariant::Destructive,
                ButtonSize::Sm,
                "Remove",
                ButtonState::Normal,
                move || {
                    update_config(&apply, |c| {
                        let sections = &mut kind.get_mut(c).sections;
                        if let Some(section) = sections.get_mut(section_index) {
                            if island_index < section.islands.len() {
                                section.islands.remove(island_index);
                            }
                        }
                    })
                },
            )));
            rows.push(row(&label, Box::new(controls)));
        }
        let apply = config.clone();
        rows.push(row(
            "Group",
            Box::new(Button::styled(
                ButtonVariant::Destructive,
                ButtonSize::Sm,
                "Remove group",
                ButtonState::Normal,
                move || {
                    update_config(&apply, |c| {
                        let sections = &mut kind.get_mut(c).sections;
                        if section_index < sections.len() {
                            sections.remove(section_index);
                        }
                    })
                },
            )),
        ));
        groups.push(group(rows));
    }
    let apply = config.clone();
    groups.push(group(vec![row(
        "Island group",
        Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "Add group",
            ButtonState::Normal,
            move || {
                update_config(&apply, |c| {
                    kind.get_mut(c).sections.push(SectionConfig::default())
                })
            },
        )),
    )]));
    section("Island groups", "Groups are the visual islands that separate widgets. Add widgets from the Islands page, then place them in either bar.", groups)
}
