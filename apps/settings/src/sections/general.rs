use crate::common::{fixed_body, group, row, section, tab_colors, update_config};
use coconut_core::{DockAlign, DockConfig, DockPosition, IslandEntry, SectionConfig, ShellConfig};
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentage, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_image::{ImageData, SvgSize};
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::{
    tab_styles, Button, ButtonSize, ButtonState, ButtonVariant, Icon, IconImage, IconSource,
    RawButton, RawText, RawView, ScrollController, SegmentedControl, Select, SelectController,
    Surface, SurfaceRole, Switch, Symbol, Tab, TabSizing, Tabs, Text, TextSize,
};
use std::rc::Rc;

const POSITIONS: [DockPosition; 4] = [
    DockPosition::Top,
    DockPosition::Bottom,
    DockPosition::Left,
    DockPosition::Right,
];

const ALIGNS: [DockAlign; 3] = [DockAlign::Start, DockAlign::Center, DockAlign::End];
const EDGE_GAPS: [f32; 3] = [0.0, 16.0, 32.0];
const SECTION_GAPS: [f32; 3] = [0.0, 8.0, 16.0];
const MAX_LENGTHS: [Option<f32>; 3] = [None, Some(600.0), Some(900.0)];
const MARGINS: [f32; 3] = [0.0, 8.0, 16.0];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DockTab {
    Display,
    Appearance,
    Content,
}

/// Ids a click-to-add picker offers per section. Tray's per-device ids
/// ("tray.wifi", ...) are intentionally left out here — those only exist
/// under `TrayMode::Individual` and are managed by `modules/tray.toml`, not
/// by picking a fixed id like the rest of these.
const KNOWN_WIDGETS: &[(&str, &str)] = &[
    ("logo", "Logo"),
    ("weather", "Weather"),
    ("current_playing", "Now playing"),
    ("app_launcher", "App launcher"),
    ("control_center", "Control center"),
    ("clock", "Clock"),
];

fn widget_label(id: &str) -> &str {
    KNOWN_WIDGETS
        .iter()
        .find_map(|(known_id, label)| (*known_id == id).then_some(*label))
        .unwrap_or(id)
}

/// The sidebar label for a dock: its own name if it has one, otherwise a
/// positional fallback. Used both by `apps/settings/src/lib.rs`'s sidebar
/// tree and (implicitly, via the same value) nowhere else — the rendering
/// engine never reads `DockConfig::name`, it's Settings-only.
pub fn dock_label(index: usize, dock: &DockConfig) -> String {
    dock.name
        .clone()
        .unwrap_or_else(|| format!("Panel {}", index + 1))
}

pub fn build_dock_page(
    _: Size,
    config: &Signal<ShellConfig>,
    scroll: &ScrollController,
    tab: &Signal<DockTab>,
    dock_index: usize,
) -> BoxedWidget {
    let docks = config.get().docks;
    let dock_count = docks.len();
    let Some(dock) = docks.get(dock_index) else {
        return section("Panel", "This panel no longer exists.", Vec::new());
    };

    let selected_tab = tab.get();
    let mut body = Vec::new();
    match selected_tab {
        DockTab::Display => {
            body.push(top_card(config, dock_index, dock));
            if dock_count > 1 {
                body.push(group(vec![row(
                    "Panel",
                    button(ButtonVariant::Destructive, "Remove panel", {
                        let config = config.clone();
                        move || {
                            update_config(&config, |c| {
                                if dock_index < c.docks.len() {
                                    c.docks.remove(dock_index);
                                }
                            });
                        }
                    }),
                )]));
            }
        }
        DockTab::Appearance => body.push(appearance_card(config, dock_index, dock)),
        DockTab::Content => {
            let section_count = dock.sections.len();
            for (section_index, dock_section) in dock.sections.iter().enumerate() {
                body.push(insert_section_button(config, dock_index, section_index));
                body.push(section_card(
                    config,
                    dock_index,
                    section_index,
                    section_count,
                    dock_section,
                ));
            }
            body.push(insert_section_button(config, dock_index, section_count));
        }
    }

    let content: BoxedWidget = Box::new(Flex::column().gap(10.0).with_children(body));
    fixed_body(dock_tabs(tab, selected_tab), content, scroll.clone())
}

fn dock_tabs(tab: &Signal<DockTab>, active: DockTab) -> BoxedWidget {
    let labels = ["Position", "Appearance", "Contents"];
    let styles = tab_styles(&labels, TabSizing::Fill, 38.0, 12.0);
    let colors = tab_colors();
    let mut tabs = Tabs::new(
        colors,
        LayoutStyle {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..Default::default()
        },
    )
    .gap(8.0);
    for (index, label) in labels.into_iter().enumerate() {
        let tab = tab.clone();
        let value = match index {
            0 => DockTab::Display,
            1 => DockTab::Appearance,
            _ => DockTab::Content,
        };
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            label,
            active == value,
            move || tab.set(value),
        )));
    }
    Box::new(tabs)
}

fn top_card(config: &Signal<ShellConfig>, dock_index: usize, dock: &DockConfig) -> BoxedWidget {
    let enabled_switch: BoxedWidget = Box::new(Switch::new(dock.enabled, {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    d.enabled = !d.enabled;
                }
            });
        }
    }));
    let selected = POSITIONS
        .iter()
        .position(|candidate| *candidate == dock.position)
        .unwrap_or(0);
    let position_options: [(&[u8], &str); 4] = [
        (
            include_bytes!("../../../../assets/icons/bar-top.svg"),
            "Top",
        ),
        (
            include_bytes!("../../../../assets/icons/bar-bottom.svg"),
            "Bottom",
        ),
        (
            include_bytes!("../../../../assets/icons/bar-left.svg"),
            "Left",
        ),
        (
            include_bytes!("../../../../assets/icons/bar-right.svg"),
            "Right",
        ),
    ];
    let position_control = icon_choice_row(&position_options, selected, {
        let config = config.clone();
        move |index: usize| {
            update_config(&config, |c| {
                if let Some(d) = c.docks.get_mut(dock_index) {
                    d.position = POSITIONS[index];
                }
            });
        }
    });
    let align_selected = ALIGNS
        .iter()
        .position(|candidate| *candidate == dock.align)
        .unwrap_or(0);
    // A dock laid along the top/bottom edge is a horizontal bar, so its
    // "Alignment" setting slides content left/center/right; a left/right
    // dock is vertical, so the same setting slides it up/center/down.
    let vertical_axis = matches!(dock.position, DockPosition::Left | DockPosition::Right);
    let align_options: [(&[u8], &str); 3] = if vertical_axis {
        [
            (
                include_bytes!("../../../../assets/icons/align-top.svg"),
                "Top",
            ),
            (
                include_bytes!("../../../../assets/icons/align-center-vertical.svg"),
                "Center",
            ),
            (
                include_bytes!("../../../../assets/icons/align-bottom.svg"),
                "Bottom",
            ),
        ]
    } else {
        [
            (
                include_bytes!("../../../../assets/icons/align-left.svg"),
                "Left",
            ),
            (
                include_bytes!("../../../../assets/icons/align-center-horizontal.svg"),
                "Center",
            ),
            (
                include_bytes!("../../../../assets/icons/align-right.svg"),
                "Right",
            ),
        ]
    };
    let align_control = icon_choice_row(&align_options, align_selected, {
        let config = config.clone();
        move |index: usize| {
            update_config(&config, |c| {
                if let Some(dock) = c.docks.get_mut(dock_index) {
                    dock.align = ALIGNS[index];
                }
            });
        }
    });
    let edge_selected = EDGE_GAPS
        .iter()
        .position(|candidate| *candidate == dock.edge_gap)
        .unwrap_or(1);
    let edge_control: BoxedWidget = Box::new(
        SegmentedControl::new(edge_selected, {
            let config = config.clone();
            move |index: usize| {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.edge_gap = EDGE_GAPS[index];
                    }
                });
            }
        })
        .option("None")
        .option("Normal")
        .option("Wide"),
    );
    let section_gap_selected = SECTION_GAPS
        .iter()
        .position(|candidate| *candidate == dock.section_gap)
        .unwrap_or(0);
    let section_gap_control: BoxedWidget = Box::new(
        SegmentedControl::new(section_gap_selected, {
            let config = config.clone();
            move |index: usize| {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.section_gap = SECTION_GAPS[index];
                    }
                });
            }
        })
        .option("None")
        .option("Small")
        .option("Large"),
    );
    let fill_switch: BoxedWidget = Box::new(Switch::new(dock.fill_available_space, {
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(dock) = c.docks.get_mut(dock_index) {
                    dock.fill_available_space = !dock.fill_available_space;
                }
            });
        }
    }));
    let length_selected = MAX_LENGTHS
        .iter()
        .position(|candidate| *candidate == dock.max_length)
        .unwrap_or(0);
    let length_control: BoxedWidget = Box::new(
        SegmentedControl::new(length_selected, {
            let config = config.clone();
            move |index: usize| {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.max_length = MAX_LENGTHS[index];
                    }
                });
            }
        })
        .option("Auto")
        .option("600 px")
        .option("900 px"),
    );
    let margin_selected = MARGINS
        .iter()
        .position(|candidate| *candidate == dock.margin)
        .unwrap_or(0);
    let margin_control: BoxedWidget = Box::new(
        SegmentedControl::new(margin_selected, {
            let config = config.clone();
            move |index: usize| {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.margin = MARGINS[index];
                    }
                });
            }
        })
        .option("None")
        .option("Small")
        .option("Large"),
    );
    Box::new(Flex::column().gap(14.0).with_children(vec![
        group(vec![
            row("Enabled", enabled_switch),
            row("Position", position_control),
        ]),
        group(vec![
            row("Alignment", align_control),
            row("Fill available space", fill_switch),
            row("Maximum length", length_control),
            row("Edge spacing", edge_control),
            row("Section spacing", section_gap_control),
            row("Margin", margin_control),
        ]),
    ]))
}

fn appearance_card(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    dock: &DockConfig,
) -> BoxedWidget {
    let toggle = |label: &'static str, enabled: bool, apply: fn(&mut DockConfig)| {
        row(
            label,
            Box::new(Switch::new(enabled, {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        if let Some(dock) = c.docks.get_mut(dock_index) {
                            apply(dock);
                        }
                    })
                }
            })) as BoxedWidget,
        )
    };
    Box::new(Flex::column().gap(14.0).with_children(vec![
        group(vec![
            toggle("Panel background", dock.show_background, |dock| {
                dock.show_background = !dock.show_background
            }),
            toggle("Panel border", dock.show_border, |dock| {
                dock.show_border = !dock.show_border
            }),
        ]),
        group(vec![
            toggle("Widget backgrounds", dock.show_island_background, |dock| {
                dock.show_island_background = !dock.show_island_background
            }),
            toggle("Widget borders", dock.show_island_border, |dock| {
                dock.show_island_border = !dock.show_island_border
            }),
        ]),
    ]))
}

fn insert_section_button(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    at: usize,
) -> BoxedWidget {
    let mut control = Flex::row().gap(10.0).align(Align::Center);
    control = control.child(connector());
    control = control.child(circle_button({
        let config = config.clone();
        move || {
            update_config(&config, |c| {
                if let Some(dock) = c.docks.get_mut(dock_index) {
                    dock.sections
                        .insert(at.min(dock.sections.len()), SectionConfig::default());
                }
            });
        }
    }));
    control = control.child(connector());
    Box::new(control)
}

#[allow(clippy::too_many_arguments)]
fn section_card(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    dock_section: &SectionConfig,
) -> BoxedWidget {
    let indicator = section_indicator(
        config,
        dock_index,
        section_index,
        section_count,
        dock_section.enabled,
    );
    let move_up = icon_button(
        include_bytes!("../../../../assets/icons/chevron-up.svg"),
        Symbol::ChevronLeft,
        false,
        section_index == 0,
        {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.sections.swap(section_index - 1, section_index);
                    }
                })
            }
        },
    );
    let move_down = icon_button(
        include_bytes!("../../../../assets/icons/chevron-down.svg"),
        Symbol::ChevronRight,
        false,
        section_index + 1 >= section_count,
        {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        dock.sections.swap(section_index, section_index + 1);
                    }
                })
            }
        },
    );
    let delete = icon_button(
        include_bytes!("../../../../assets/icons/trash.svg"),
        Symbol::Close,
        true,
        false,
        {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(dock) = c.docks.get_mut(dock_index) {
                        if section_index < dock.sections.len() {
                            dock.sections.remove(section_index);
                        }
                    }
                })
            }
        },
    );
    let controls: BoxedWidget = Box::new(
        Flex::column()
            .align(Align::Center)
            .justify(Justify::Between)
            .with_children(vec![
                indicator,
                Box::new(
                    Flex::column()
                        .gap(6.0)
                        .align(Align::Center)
                        .with_children(vec![move_up, move_down]),
                ) as BoxedWidget,
                delete,
            ]),
    );

    let mut island_rows = Vec::new();

    let island_count = dock_section.islands.len();
    for (island_index, entry) in dock_section.islands.iter().enumerate() {
        island_rows.push(row(
            widget_label(&entry.id),
            island_controls(
                config,
                dock_index,
                section_index,
                section_count,
                island_index,
                island_count,
            ),
        ));
    }

    let missing: Vec<(&'static str, &'static str)> = KNOWN_WIDGETS
        .iter()
        .copied()
        .filter(|(id, _)| !dock_section.islands.iter().any(|entry| entry.id == *id))
        .collect();
    if !missing.is_empty() {
        let controller = SelectController::new(0);
        let labels: Vec<&str> = missing.iter().map(|(_, label)| *label).collect();
        let picker: BoxedWidget = Box::new(
            Select::controlled(&labels, controller)
                .searchable()
                .on_select({
                    let config = config.clone();
                    move |index: usize| {
                        let Some(&(id, _)) = missing.get(index) else {
                            return;
                        };
                        update_config(&config, |c| {
                            if let Some(d) = c.docks.get_mut(dock_index) {
                                if let Some(s) = d.sections.get_mut(section_index) {
                                    s.islands.push(IslandEntry::with_id(id));
                                }
                            }
                        });
                    }
                }),
        );
        island_rows.push(row("Add widget", picker));
    }

    let last_row = island_rows.len().saturating_sub(1);
    let mut card_rows = Vec::with_capacity(island_rows.len() * 2);
    for (index, row) in island_rows.into_iter().enumerate() {
        card_rows.push(row);
        if index != last_row {
            card_rows.push(row_divider());
        }
    }
    let card: BoxedWidget = Box::new(
        Surface::new(
            SurfaceRole::Inset,
            LayoutStyle {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(148.0),
                },
                ..Default::default()
            },
        )
        .with_children(card_rows),
    );
    Box::new(
        Flex::row()
            .gap(10.0)
            .align(Align::Stretch)
            .child(controls)
            .child(card),
    )
}

fn row_divider() -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(LayoutStyle {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(1.0),
            },
            ..Default::default()
        })
        .background(theme.border),
    )
}

fn section_indicator(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    enabled: bool,
) -> BoxedWidget {
    let theme = use_theme();
    let segment_count = section_count.max(1);
    let content: BoxedWidget = if enabled {
        let segments: Vec<BoxedWidget> = (0..segment_count)
            .map(|index| {
                let color = if index == section_index {
                    theme.success
                } else {
                    theme.border_strong
                };
                Box::new(
                    RawView::new(LayoutStyle {
                        flex_grow: 1.0,
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        ..Default::default()
                    })
                    .background(color)
                    .corner_radius(3.0),
                ) as BoxedWidget
            })
            .collect();
        Box::new(
            Flex::column()
                .size(30.0, 30.0)
                .gap(3.0)
                .with_children(segments),
        )
    } else {
        Box::new(
            Flex::row()
                .size(30.0, 30.0)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(Box::new(RawText::new("Off", theme.danger, 12.0)) as BoxedWidget),
        )
    };
    Box::new(
        RawButton::new(
            Style::new()
                .layout(LayoutStyle {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(40.0),
                        height: Dimension::Length(40.0),
                    },
                    padding: creamui_core::layout::Rect {
                        left: LengthPercentage::Length(5.0),
                        right: LengthPercentage::Length(5.0),
                        top: LengthPercentage::Length(5.0),
                        bottom: LengthPercentage::Length(5.0),
                    },
                    ..Default::default()
                })
                .background(theme.surface_hover)
                .corner_radius(10.0)
                .hover(StateStyle::new().background(theme.surface_elevated))
                .pressed(StateStyle::new().background(theme.border_strong)),
            {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        if let Some(dock) = c.docks.get_mut(dock_index) {
                            if let Some(section) = dock.sections.get_mut(section_index) {
                                section.enabled = !section.enabled;
                            }
                        }
                    })
                }
            },
        )
        .child(content),
    )
}

fn connector() -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        RawView::new(LayoutStyle {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(1.0),
            },
            ..Default::default()
        })
        .background(theme.border),
    )
}

/// Move/remove controls for one island: "up" swaps it earlier in this
/// section, or — if it's already first — moves it to the end of the
/// previous section; "down" is the mirror image into the next section. At
/// either end of a dock's section list, the corresponding button is simply
/// omitted rather than wrapping around or crossing into another dock.
#[allow(clippy::too_many_arguments)]
fn island_controls(
    config: &Signal<ShellConfig>,
    dock_index: usize,
    section_index: usize,
    section_count: usize,
    island_index: usize,
    island_count: usize,
) -> BoxedWidget {
    let mut controls = Flex::row().gap(8.0).align(Align::Center);
    if island_index > 0 || section_index > 0 {
        controls = controls.child(icon_button(
            include_bytes!("../../../../assets/icons/chevron-up.svg"),
            Symbol::ChevronLeft,
            false,
            false,
            {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        move_island_up(c, dock_index, section_index, island_index);
                    });
                }
            },
        ));
    }
    if island_index + 1 < island_count || section_index + 1 < section_count {
        controls = controls.child(icon_button(
            include_bytes!("../../../../assets/icons/chevron-down.svg"),
            Symbol::ChevronRight,
            false,
            false,
            {
                let config = config.clone();
                move || {
                    update_config(&config, |c| {
                        move_island_down(c, dock_index, section_index, island_index);
                    });
                }
            },
        ));
    }
    controls = controls.child(icon_button(
        include_bytes!("../../../../assets/icons/trash.svg"),
        Symbol::Close,
        true,
        false,
        {
            let config = config.clone();
            move || {
                update_config(&config, |c| {
                    if let Some(d) = c.docks.get_mut(dock_index) {
                        if let Some(s) = d.sections.get_mut(section_index) {
                            if island_index < s.islands.len() {
                                s.islands.remove(island_index);
                            }
                        }
                    }
                });
            }
        },
    ));
    Box::new(controls)
}

fn move_island_up(
    config: &mut ShellConfig,
    dock_index: usize,
    section_index: usize,
    island_index: usize,
) {
    let Some(dock) = config.docks.get_mut(dock_index) else {
        return;
    };
    if island_index == 0 {
        if section_index == 0 || island_index >= dock.sections[section_index].islands.len() {
            return;
        }
        let entry = dock.sections[section_index].islands.remove(island_index);
        dock.sections[section_index - 1].islands.push(entry);
    } else if island_index < dock.sections[section_index].islands.len() {
        dock.sections[section_index]
            .islands
            .swap(island_index - 1, island_index);
    }
}

fn move_island_down(
    config: &mut ShellConfig,
    dock_index: usize,
    section_index: usize,
    island_index: usize,
) {
    let Some(dock) = config.docks.get_mut(dock_index) else {
        return;
    };
    let Some(current) = dock.sections.get(section_index) else {
        return;
    };
    let last = current.islands.len().saturating_sub(1);
    if island_index != last {
        if island_index + 1 < current.islands.len() {
            dock.sections[section_index]
                .islands
                .swap(island_index, island_index + 1);
        }
        return;
    }
    if section_index + 1 >= dock.sections.len() {
        return;
    }
    let entry = dock.sections[section_index].islands.remove(island_index);
    dock.sections[section_index + 1].islands.insert(0, entry);
}

fn button(variant: ButtonVariant, label: &str, on_click: impl Fn() + 'static) -> BoxedWidget {
    Box::new(Button::styled(
        variant,
        ButtonSize::Sm,
        label,
        ButtonState::Normal,
        on_click,
    ))
}

fn icon_button(
    source: &[u8],
    fallback: Symbol,
    destructive: bool,
    disabled: bool,
    on_click: impl Fn() + 'static,
) -> BoxedWidget {
    let theme = use_theme();
    let icon = ImageData::from_svg(source, SvgSize::Max(32))
        .map(|image| {
            IconSource::Image(IconImage {
                image: image.image().clone(),
                monochrome: true,
            })
        })
        .unwrap_or_else(|error| {
            eprintln!("settings: failed to decode a dock action icon: {error}");
            IconSource::Symbol(fallback)
        });
    let foreground = if destructive {
        theme.selection_text
    } else if disabled {
        theme.text_disabled
    } else {
        theme.text_primary
    };
    let background = if destructive {
        theme.danger
    } else if disabled {
        theme.surface
    } else {
        theme.surface_hover
    };
    let style = Style::new()
        .layout(LayoutStyle {
            size: creamui_core::layout::Size {
                width: Dimension::Length(32.0),
                height: Dimension::Length(32.0),
            },
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0)
        .hover(StateStyle::new().background(if destructive {
            theme.danger.mix(theme.text_primary, 0.10)
        } else {
            theme.surface_elevated
        }))
        .pressed(StateStyle::new().background(if destructive {
            theme.danger.mix(theme.text_primary, 0.20)
        } else {
            theme.border_strong
        }));
    Box::new(
        RawButton::new(style, on_click)
            .disabled(disabled)
            .child(Box::new(
                Flex::row()
                    .size(32.0, 32.0)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .child(Box::new(Icon::new(icon, foreground).size(16.0)) as BoxedWidget),
            )),
    )
}

/// A row of exclusive icon+label buttons, like [`SegmentedControl`] but with
/// a bundled SVG glyph alongside each option's text.
fn icon_choice_row(
    options: &[(&'static [u8], &'static str)],
    selected: usize,
    on_change: impl Fn(usize) + 'static,
) -> BoxedWidget {
    let on_change: Rc<dyn Fn(usize)> = Rc::new(on_change);
    let mut row = Flex::row().gap(8.0);
    for (index, (source, label)) in options.iter().enumerate() {
        let on_change = on_change.clone();
        row = row.child(icon_choice(source, label, index == selected, move || {
            on_change(index)
        }));
    }
    Box::new(row)
}

fn icon_choice(
    source: &[u8],
    label: &str,
    selected: bool,
    on_click: impl Fn() + 'static,
) -> BoxedWidget {
    let theme = use_theme();
    let icon = ImageData::from_svg(source, SvgSize::Max(32))
        .map(|image| {
            IconSource::Image(IconImage {
                image: image.image().clone(),
                monochrome: true,
            })
        })
        .unwrap_or_else(|error| {
            eprintln!("settings: failed to decode a dock choice icon: {error}");
            IconSource::Symbol(Symbol::Grid)
        });
    let background = if selected {
        theme.surface_elevated
    } else {
        theme.surface_hover
    };
    let foreground = if selected {
        theme.text_primary
    } else {
        theme.text_secondary
    };
    let style = Style::new()
        .layout(LayoutStyle {
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(72.0),
                height: Dimension::Length(30.0),
            },
            flex_grow: 1.0,
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(8.0),
                right: LengthPercentage::Length(8.0),
                top: LengthPercentage::Length(6.0),
                bottom: LengthPercentage::Length(6.0),
            },
            ..Default::default()
        })
        .background(background)
        .corner_radius(theme.tab_radius)
        .border(
            if selected {
                theme.border_strong
            } else {
                background
            },
            1.0,
        )
        .hover(StateStyle::new().background(background.mix(theme.text_primary, 0.04)))
        .pressed(StateStyle::new().background(background.mix(theme.text_primary, 0.09)));
    Box::new(
        RawButton::new(style, on_click).child(Box::new(
            Flex::row()
                .gap(6.0)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(Box::new(Icon::new(icon, foreground).size(14.0)) as BoxedWidget)
                .child(Box::new(RawText::new(label.to_owned(), foreground, 12.0)) as BoxedWidget),
        )),
    )
}

fn circle_button(on_click: impl Fn() + 'static) -> BoxedWidget {
    let theme = use_theme();
    let style = Style::new()
        .layout(LayoutStyle {
            size: creamui_core::layout::Size {
                width: Dimension::Length(28.0),
                height: Dimension::Length(28.0),
            },
            ..Default::default()
        })
        .background(theme.surface_hover)
        .corner_radius(14.0)
        .hover(StateStyle::new().background(theme.surface_elevated))
        .pressed(StateStyle::new().background(theme.border_strong));
    Box::new(
        RawButton::new(style, on_click).child(Box::new(
            Flex::row()
                .size(28.0, 28.0)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(Box::new(Text::new("+").size(TextSize::Lg)) as BoxedWidget),
        )),
    )
}
