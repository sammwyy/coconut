use coconut_core::{BackgroundSource, DesktopColor, DockAlign, DockConfig};
use creamui_core::BoxedWidget;
use creamui_widgets::{
    layout::{Align, Flex, Grid, Wrap},
    ColorPicker, ColorPickerController, SegmentedControl, Slider, Switch, Text, TextSize,
};
use std::rc::Rc;

pub type Mutation = Box<dyn FnOnce(&mut DockConfig)>;
pub type Change = Rc<dyn Fn(Mutation)>;

#[derive(Clone, Default)]
pub struct AppearanceControls {
    bar_color: ColorPickerController,
    island_color: ColorPickerController,
}

pub fn row(label: impl Into<String>, control: BoxedWidget) -> BoxedWidget {
    Box::new(
        Flex::row()
            .wrap(Wrap::Wrap)
            .padding_xy(12.0, 8.0)
            .min_height(40.0)
            .align(Align::Center)
            .gap(12.0)
            .child(Box::new(
                Flex::row()
                    .grow(1.0)
                    .min_width(100.0)
                    .child(Box::new(Text::new(label).size(TextSize::Sm))),
            ))
            .child(control),
    )
}

fn toggle(
    label: &str,
    value: bool,
    change: &Change,
    set: fn(&mut DockConfig, bool),
) -> BoxedWidget {
    let change = change.clone();
    row(
        label,
        Box::new(Switch::new(value, move || {
            change(Box::new(move |bar| set(bar, !value)))
        })),
    )
}

fn number(
    label: &str,
    value: f32,
    range: (f32, f32),
    change: &Change,
    set: fn(&mut DockConfig, f32),
) -> BoxedWidget {
    let change = change.clone();
    row(
        format!("{label}  {}", value.round() as i32),
        Box::new(Slider::new(
            ((value - range.0) / (range.1 - range.0)).clamp(0.0, 1.0),
            move |fraction| {
                let value = (range.0 + fraction * (range.1 - range.0)).round();
                change(Box::new(move |bar| set(bar, value)));
            },
        )),
    )
}

pub fn layout(bar: &DockConfig, change: &Change) -> Vec<BoxedWidget> {
    let apply = change.clone();
    vec![
        toggle("Auto-hide", bar.auto_hide, change, |bar, value| {
            bar.auto_hide = value
        }),
        toggle(
            "Fill available space",
            bar.fill_available_space,
            change,
            |bar, value| bar.fill_available_space = value,
        ),
        row(
            "Alignment",
            Box::new(
                SegmentedControl::new(
                    match bar.align {
                        DockAlign::Start => 0,
                        DockAlign::Center => 1,
                        DockAlign::End => 2,
                    },
                    move |index| {
                        apply(Box::new(move |bar| {
                            bar.align = [DockAlign::Start, DockAlign::Center, DockAlign::End][index]
                        }));
                    },
                )
                .option("Start")
                .option("Center")
                .option("End"),
            ),
        ),
        number(
            "Thickness",
            bar.thickness,
            (28.0, 100.0),
            change,
            |bar, value| bar.thickness = value,
        ),
        number(
            "Screen inset",
            bar.margin,
            (0.0, 48.0),
            change,
            |bar, value| bar.margin = value,
        ),
        number(
            "End padding",
            bar.edge_gap,
            (0.0, 48.0),
            change,
            |bar, value| bar.edge_gap = value,
        ),
        number(
            "Group spacing",
            bar.section_gap,
            (0.0, 48.0),
            change,
            |bar, value| bar.section_gap = value,
        ),
    ]
}

impl AppearanceControls {
    pub fn dismiss(&self) {
        self.bar_color.set_open(false);
        self.island_color.set_open(false);
    }

    pub fn dismiss_picker(&self) -> bool {
        let open = self.bar_color.is_open() || self.island_color.is_open();
        self.dismiss();
        open
    }

    pub fn build(&self, bar: &DockConfig, change: &Change) -> Vec<BoxedWidget> {
        let mut rows = vec![
            toggle(
                "Bar background",
                bar.show_background,
                change,
                |bar, value| bar.show_background = value,
            ),
            toggle("Bar border", bar.show_border, change, |bar, value| {
                bar.show_border = value
            }),
        ];
        rows.extend(self.surface(bar, change, false));
        rows.extend([
            toggle(
                "Island backgrounds",
                bar.show_island_background,
                change,
                |bar, value| bar.show_island_background = value,
            ),
            toggle(
                "Island borders",
                bar.show_island_border,
                change,
                |bar, value| bar.show_island_border = value,
            ),
            toggle(
                "Join each island group",
                bar.unify_island_background,
                change,
                |bar, value| bar.unify_island_background = value,
            ),
        ]);
        rows.extend(self.surface(bar, change, true));
        rows
    }

    fn surface(&self, bar: &DockConfig, change: &Change, island: bool) -> Vec<BoxedWidget> {
        let (source, color, opacity, picker) = if island {
            (
                bar.island_background_source,
                bar.island_background_color,
                bar.island_background_opacity,
                &self.island_color,
            )
        } else {
            (
                bar.background_source,
                bar.background_color,
                bar.background_opacity,
                &self.bar_color,
            )
        };
        let prefix = if island { "Island" } else { "Bar" };
        let mut rows = vec![toggle(
            &format!("{prefix} theme color"),
            source == BackgroundSource::Theme,
            change,
            if island {
                |bar, value| {
                    bar.island_background_source = if value {
                        BackgroundSource::Theme
                    } else {
                        BackgroundSource::Custom
                    }
                }
            } else {
                |bar, value| {
                    bar.background_source = if value {
                        BackgroundSource::Theme
                    } else {
                        BackgroundSource::Custom
                    }
                }
            },
        )];
        let apply = change.clone();
        rows.push(row(
            format!("{prefix} color"),
            Box::new(
                ColorPicker::controlled_with_style(
                    creamui_core::layout::Style {
                        size: creamui_widgets::layout::fixed(160.0, 32.0),
                        flex_shrink: 0.0,
                        ..Default::default()
                    },
                    creamui_theme::Color::rgb(color.r, color.g, color.b),
                    picker,
                    move |color| {
                        let color = DesktopColor {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                        };
                        apply(Box::new(move |bar| {
                            if island {
                                bar.island_background_color = color;
                            } else {
                                bar.background_color = color;
                            }
                        }));
                    },
                )
                .disabled(source == BackgroundSource::Theme),
            ),
        ));
        let apply = change.clone();
        rows.push(row(
            format!("{prefix} opacity  {}%", (opacity * 100.0).round() as i32),
            Box::new(Slider::new(opacity, move |opacity| {
                apply(Box::new(move |bar| {
                    if island {
                        bar.island_background_opacity = opacity;
                    } else {
                        bar.background_opacity = opacity;
                    }
                }));
            })),
        ));
        rows
    }
}

pub fn spacing(gap: &str, change: Rc<dyn Fn(String)>) -> BoxedWidget {
    let (mode, value) = match crate::parse_gap(gap) {
        crate::Gap::Fixed(value) => (0, value),
        crate::Gap::Percent(fraction) => (1, fraction * 100.0),
        crate::Gap::Between => (2, 0.0),
        crate::Gap::Evenly => (3, 0.0),
    };
    let mut modes = Grid::new().columns(2).gap(4.0);
    for (index, label) in ["Pixels", "Percent", "Between", "Evenly"]
        .into_iter()
        .enumerate()
    {
        let change = change.clone();
        modes = modes.child(Box::new(creamui_widgets::Choice::new(
            label,
            mode == index,
            move || {
                change(["10px", "10%", "between", "evenly"][index].into());
            },
        )));
    }
    let mut root = Flex::column()
        .gap(8.0)
        .child(Box::new(Text::new("Item spacing").size(TextSize::Sm)))
        .child(Box::new(modes));
    if mode < 2 {
        let max = if mode == 0 { 48.0 } else { 100.0 };
        root = root.child(row(
            format!("{}{}", value.round(), if mode == 0 { " px" } else { "%" }),
            Box::new(Slider::new(value / max, move |fraction| {
                change(format!(
                    "{}{}",
                    (fraction * max).round(),
                    if mode == 0 { "px" } else { "%" }
                ));
            })),
        ));
    }
    Box::new(root)
}
