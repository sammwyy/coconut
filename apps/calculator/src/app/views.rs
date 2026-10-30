use super::*;
use creamui_core::{StateStyle, TextAlign};

#[derive(Clone, Copy)]
enum KeyStyle {
    Number,
    Operation,
    Utility,
    Function,
    Equal,
    Quiet,
}
impl State {
    fn key_button(
        &self,
        label: &str,
        action: Action,
        kind: KeyStyle,
        height: f32,
        disabled: bool,
    ) -> BoxedWidget {
        let theme = use_theme();
        let colors = theme.colors;
        let (background, foreground, font_size) = match kind {
            KeyStyle::Number => (colors.surface_elevated, colors.text_primary, 26.0),
            KeyStyle::Operation => (colors.accent.mix(colors.surface, 0.88), colors.accent, 28.0),
            KeyStyle::Utility => (colors.surface_hover, colors.text_primary, 17.0),
            KeyStyle::Function => (
                colors.surface_elevated.mix(colors.surface, 0.5),
                colors.text_secondary,
                14.0,
            ),
            KeyStyle::Equal => (colors.accent, colors.selection_text, 28.0),
            KeyStyle::Quiet => (colors.surface, colors.text_secondary, 12.0),
        };
        let radius = if matches!(kind, KeyStyle::Quiet) {
            theme.button_radius
        } else {
            (theme.card_radius * 1.4).min(height * 0.38)
        };
        let state = self.clone();
        let text = Text::new(label)
            .font_size(font_size)
            .color(if disabled {
                colors.text_disabled
            } else {
                foreground
            })
            .text_align(TextAlign::Center);
        Box::new(
            Button::styled(
                if matches!(kind, KeyStyle::Equal) {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                },
                ButtonSize::Md,
                label,
                ButtonState::Normal,
                move || state.run(action.clone()),
            )
            .disabled(disabled)
            .background(background)
            .border(background, 0.0)
            .corner_radius(radius)
            .hover_style(StateStyle::new().background(background.mix(colors.text_primary, 0.06)))
            .pressed_style(StateStyle::new().background(background.mix(colors.text_primary, 0.13)))
            .disabled_style(
                StateStyle::new()
                    .background(background)
                    .border(background, 0.0),
            )
            .customize(move |button| {
                button.style.layout.flex_grow = 1.0;
                button.style.layout.flex_basis = Dimension::Length(0.0);
                button.style.layout.min_size.width = Dimension::Length(0.0);
                button.style.layout.size.height = Dimension::Length(height);
                button.style.layout.padding = layout::Rect::length(0.0_f32);
                button.children = vec![Box::new(text)];
            }),
        )
    }
    pub fn build(&self, size: Size) -> BoxedWidget {
        self.revision.get();
        self.viewport.set(size);
        if self.pending_size.get() == Some((size.width.round() as u32, size.height.round() as u32))
        {
            self.pending_size.set(None);
        }
        RouterProvider::new(self.router.clone()).render(|| {
            creamui_reactive::provide_context(self.clone());
            self.frame(size)
        })
    }
    fn frame(&self, size: Size) -> BoxedWidget {
        let theme = use_theme();
        let mode = self.mode();
        let drawer = self.drawer_open.get();
        let leading = self.clone();
        let titlebar = AppTitlebar::new(size.width)
            .height(42.0)
            .center_margin(8.0)
            .center(Text::secondary("Calculator").font_size(12.0))
            .start(
                Button::styled(
                    ButtonVariant::Tertiary,
                    ButtonSize::Sm,
                    format!("{}  ▾", mode.name()),
                    ButtonState::Normal,
                    move || leading.run(Action::Drawer),
                )
                .width(if size.width < 400.0 { 120.0_f32 } else { 176.0 }),
            );
        let compact = size.height < 570.0;
        let gap = if compact { 6.0 } else { 10.0 };
        let inset = if compact { 12.0 } else { 18.0 };
        let display_height = if compact { 88.0 } else { 128.0 } * theme.text_scale;
        let available =
            (size.height - 42.0 - 30.0 - display_height - 28.0 - inset * 2.0 - gap * 3.0).max(0.0);
        let width = (size.width - inset * 2.0).max(1.0);
        let geometry = Geometry {
            height: available,
            gap,
            key_height: ((available - 5.0 * gap) / 6.0)
                .clamp(36.0 * theme.text_scale, 88.0 * theme.text_scale),
        };
        creamui_reactive::provide_context(geometry);
        let mut toolbar = vec![];
        if mode == Mode::Scientific {
            let angle = self.session.borrow().context.angle;
            let state = self.clone();
            toolbar.push(Box::new(
                SegmentedControl::new(usize::from(angle == Angle::Radians), move |index| {
                    state.run(Action::Angle(if index == 0 {
                        Angle::Degrees
                    } else {
                        Angle::Radians
                    }))
                })
                .option("Deg")
                .option("Rad"),
            ) as BoxedWidget);
        }
        toolbar.push(view(grow(row(0.0)), vec![]));
        toolbar.push(view(
            fixed(50.0, 28.0),
            vec![self.key_button("Copy", Action::Copy, KeyStyle::Quiet, 28.0, false)],
        ));
        toolbar.push(view(
            fixed(62.0, 28.0),
            vec![self.key_button("History", Action::History, KeyStyle::Quiet, 28.0, false)],
        ));
        let memory = self.session.borrow().memory;
        let content = vec![
            view(fixed_height(30.0, row(6.0)), toolbar),
            self.display(display_height, width, compact),
            view(
                fixed_height(28.0, row(2.0)),
                vec![
                    self.key_button(
                        "MC",
                        Action::Memory(Memory::Clear),
                        KeyStyle::Quiet,
                        28.0,
                        memory.is_none(),
                    ),
                    self.key_button(
                        "MR",
                        Action::Recall,
                        KeyStyle::Quiet,
                        28.0,
                        memory.is_none(),
                    ),
                    self.key_button(
                        "MS",
                        Action::Memory(Memory::Store),
                        KeyStyle::Quiet,
                        28.0,
                        false,
                    ),
                    self.key_button(
                        "M+",
                        Action::Memory(Memory::Add),
                        KeyStyle::Quiet,
                        28.0,
                        false,
                    ),
                    self.key_button(
                        "M−",
                        Action::Memory(Memory::Subtract),
                        KeyStyle::Quiet,
                        28.0,
                        false,
                    ),
                    self.key_button(
                        "Ans",
                        Action::Insert("ans".into()),
                        KeyStyle::Quiet,
                        28.0,
                        false,
                    ),
                ],
            ),
            if self.history_open.get() {
                self.history()
            } else {
                RouterOutlet::render()
            },
        ];
        let main = view(
            Style {
                padding: layout::Rect::length(inset),
                ..grow(column(gap))
            },
            content,
        );
        let mut children = vec![Box::new(titlebar) as BoxedWidget, main];
        if drawer {
            let close = self.clone();
            children.push(Box::new(
                RawButton::new(
                    Style {
                        position: layout::Position::Absolute,
                        inset: layout::Rect {
                            left: layout::LengthPercentageAuto::Length(0.0),
                            right: layout::LengthPercentageAuto::Length(0.0),
                            top: layout::LengthPercentageAuto::Length(42.0),
                            bottom: layout::LengthPercentageAuto::Length(0.0),
                        },
                        ..Default::default()
                    },
                    move || close.run(Action::Drawer),
                )
                .keyboard_focus(false),
            ));
            children.push(self.drawer(size));
        }
        if self.palette_open.get() {
            let state = self.clone();
            children.push(Box::new(
                CommandPalette::new(&self.palette, self.catalog(), move || {
                    state.run(Action::Dismiss)
                })
                .viewport(size),
            ));
        }
        Box::new(
            RawView::new(Style {
                size: layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                ..column(0.0)
            })
            .background(theme.colors.surface)
            .with_children(children),
        )
    }
    fn drawer(&self, size: Size) -> BoxedWidget {
        let theme = use_theme();
        let current = self.mode();
        let mut items = vec![];
        for (title, modes) in [
            ("CALCULATOR", vec![Mode::Standard, Mode::Scientific]),
            (
                "CONVERTER",
                Category::ALL.into_iter().map(Mode::Converter).collect(),
            ),
        ] {
            items.push(view(
                Style {
                    padding: layout::Rect {
                        left: LengthPercentage::Length(10.0),
                        top: LengthPercentage::Length(14.0),
                        bottom: LengthPercentage::Length(8.0),
                        right: LengthPercentage::Length(0.0),
                    },
                    ..row(0.0)
                },
                vec![Box::new(Text::secondary(title).font_size(10.0))],
            ));
            for mode in modes {
                let state = self.clone();
                let active = mode == current;
                let background = if active {
                    theme.colors.accent.mix(theme.colors.surface, 0.88)
                } else {
                    theme.colors.surface
                };
                let label = Text::new(mode.name()).font_size(13.0).color(if active {
                    theme.colors.accent
                } else {
                    theme.colors.text_secondary
                });
                items.push(Box::new(
                    Button::styled(
                        ButtonVariant::Tertiary,
                        ButtonSize::Sm,
                        mode.name(),
                        ButtonState::Normal,
                        move || state.run(Action::Navigate(mode)),
                    )
                    .background(background)
                    .border(background, 0.0)
                    .corner_radius(theme.sidebar_item_radius)
                    .hover_style(StateStyle::new().background(theme.colors.surface_hover))
                    .customize(move |button| {
                        button.style.layout.size.width = Dimension::Percent(1.0);
                        button.style.layout.size.height =
                            Dimension::Length(34.0 * theme.text_scale);
                        button.style.layout.justify_content = Some(layout::JustifyContent::Start);
                        button.children = vec![Box::new(label)];
                    }),
                ) as BoxedWidget);
            }
        }
        Box::new(
            Popover::new(Style {
                position: layout::Position::Absolute,
                inset: layout::Rect {
                    left: layout::LengthPercentageAuto::Length(8.0),
                    top: layout::LengthPercentageAuto::Length(42.0),
                    right: layout::LengthPercentageAuto::Auto,
                    bottom: layout::LengthPercentageAuto::Auto,
                },
                size: layout::Size {
                    width: Dimension::Length(DRAWER_WIDTH as f32),
                    height: Dimension::Length((size.height - 50.0).max(80.0)),
                },
                padding: layout::Rect::length(8.0_f32),
                ..column(0.0)
            })
            .child(Box::new(
                ScrollView::controlled(grow(column(0.0)), self.drawer_scroll.clone())
                    .content_gap(2.0)
                    .with_children(items),
            )),
        )
    }
    fn display(&self, height: f32, width: f32, compact: bool) -> BoxedWidget {
        let theme = use_theme();
        let entry = self.entry();
        let message = self.message.get();
        let (value, mut notice, error) = match self.result() {
            Ok(value) => (format_number(value), String::new(), false),
            Err(error) => ("—".into(), error, true),
        };
        let mut expression = entry.previous.unwrap_or_else(|| {
            if entry.text.is_empty() {
                "0".into()
            } else {
                entry.text
            }
        });
        if let Mode::Converter(category) = self.mode() {
            let (from, to) = self.selected_units(category);
            let units = category.units();
            expression = format!("{expression} {}", units[from].symbol);
            if !error {
                notice = units[to].name.into();
            }
        }
        if !message.is_empty() {
            notice = message.clone();
        } else if notice.is_empty() {
            if let Some(memory) = self.session.borrow().memory {
                notice = format!("M  {}", format_number(memory));
            }
        }
        let capacity = (width / (12.0 * theme.text_scale)).max(4.0) as usize;
        if expression.chars().count() > capacity {
            expression = format!(
                "…{}",
                expression
                    .chars()
                    .skip(expression.chars().count() - capacity)
                    .collect::<String>()
            );
        }
        let notice: String = notice
            .chars()
            .take((width / (6.0 * theme.text_scale)).max(8.0) as usize)
            .collect();
        let font_size =
            if compact { 38.0 } else { 56.0 } * if value.len() > 15 { 0.7 } else { 1.0 };
        view(
            fixed_height(height, column(0.0)),
            vec![
                Box::new(
                    Text::secondary(expression)
                        .font_size(if compact { 14.0 } else { 19.0 })
                        .height(if compact { 22.0 } else { 34.0 } * theme.text_scale)
                        .text_align(TextAlign::End)
                        .width("100%"),
                ),
                Box::new(
                    Text::new(value)
                        .font_size(font_size)
                        .text_align(TextAlign::End)
                        .width("100%"),
                ),
                Box::new(
                    Text::secondary(notice)
                        .font_size(11.0)
                        .text_align(TextAlign::End)
                        .width("100%")
                        .color(
                            if error || (!message.is_empty() && message != "Result copied") {
                                theme.colors.danger
                            } else {
                                theme.colors.text_secondary
                            },
                        ),
                ),
            ],
        )
    }
    pub(super) fn calculator_view(&self, scientific: bool, geometry: Geometry) -> BoxedWidget {
        let mut boards = vec![];
        if scientific {
            boards.push(self.scientific_keys(geometry.key_height, geometry.gap));
        }
        boards.push(self.standard_keys(geometry.key_height, geometry.gap));
        view(
            Style {
                max_size: layout::Size {
                    width: Dimension::Length(if scientific { 920.0 } else { 460.0 }),
                    height: Dimension::Auto,
                },
                align_self: Some(layout::AlignSelf::Center),
                size: layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..grow(Style {
                    align_items: Some(AlignItems::Stretch),
                    ..row(if scientific { 18.0 } else { 0.0 })
                })
            },
            boards,
        )
    }
    pub(super) fn converter_view(&self, category: Category, geometry: Geometry) -> BoxedWidget {
        let units = category.units();
        let labels: Vec<_> = units.iter().map(|unit| unit.name).collect();
        let (from, to) = &self.units[&category];
        let source = self.clone();
        let target = self.clone();
        let field = |label: &str, controller: SelectController, state: Self, source: bool| {
            view(
                grow(column(6.0)),
                vec![
                    Box::new(Text::secondary(label).font_size(11.0)),
                    Box::new(
                        Select::controlled(&labels, controller)
                            .on_select(move |index| state.run(Action::Unit { source, index }))
                            .layout(Style {
                                size: layout::Size {
                                    width: Dimension::Percent(1.0),
                                    height: Dimension::Length(40.0),
                                },
                                ..Default::default()
                            }),
                    ),
                ],
            )
        };
        let selector = view(
            fixed_height(70.0, row(10.0)),
            vec![
                field("From", from.clone(), source, true),
                view(
                    fixed(46.0, 40.0),
                    vec![self.key_button("Swap", Action::SwapUnits, KeyStyle::Quiet, 40.0, false)],
                ),
                field("To", to.clone(), target, false),
            ],
        );
        let key_height = ((geometry.height - 80.0 - geometry.gap * 4.0) / 5.0)
            .clamp(36.0 * use_theme().text_scale, 88.0 * use_theme().text_scale);
        view(
            grow(column(geometry.gap)),
            vec![selector, self.numeric_keys(key_height, geometry.gap, false)],
        )
    }
    fn standard_keys(&self, height: f32, gap: f32) -> BoxedWidget {
        self.numeric_keys(height, gap, true)
    }
    fn numeric_keys(&self, height: f32, gap: f32, auxiliary: bool) -> BoxedWidget {
        let mut definitions = vec![
            vec![
                ("(", Action::Insert("(".into()), KeyStyle::Function),
                (")", Action::Insert(")".into()), KeyStyle::Function),
                ("x²", Action::Suffix("^2".into()), KeyStyle::Function),
                (
                    "1/x",
                    Action::Function("reciprocal".into()),
                    KeyStyle::Function,
                ),
            ],
            vec![
                ("C", Action::Clear, KeyStyle::Utility),
                ("±", Action::Sign, KeyStyle::Utility),
                ("%", Action::Suffix("%".into()), KeyStyle::Utility),
                ("÷", Action::Insert("÷".into()), KeyStyle::Operation),
            ],
            vec![
                ("7", Action::Insert("7".into()), KeyStyle::Number),
                ("8", Action::Insert("8".into()), KeyStyle::Number),
                ("9", Action::Insert("9".into()), KeyStyle::Number),
                ("×", Action::Insert("×".into()), KeyStyle::Operation),
            ],
            vec![
                ("4", Action::Insert("4".into()), KeyStyle::Number),
                ("5", Action::Insert("5".into()), KeyStyle::Number),
                ("6", Action::Insert("6".into()), KeyStyle::Number),
                ("−", Action::Insert("−".into()), KeyStyle::Operation),
            ],
            vec![
                ("1", Action::Insert("1".into()), KeyStyle::Number),
                ("2", Action::Insert("2".into()), KeyStyle::Number),
                ("3", Action::Insert("3".into()), KeyStyle::Number),
                ("+", Action::Insert("+".into()), KeyStyle::Operation),
            ],
            vec![
                ("Del", Action::Backspace, KeyStyle::Utility),
                ("0", Action::Insert("0".into()), KeyStyle::Number),
                (".", Action::Insert(".".into()), KeyStyle::Number),
                ("=", Action::Evaluate, KeyStyle::Equal),
            ],
        ];
        if !auxiliary {
            definitions.remove(0);
        }
        self.key_grid(definitions, height, gap, self.scroll.clone())
    }
    fn scientific_keys(&self, height: f32, gap: f32) -> BoxedWidget {
        let mut definitions = vec![];
        for entries in [
            [("sin", "sin"), ("cos", "cos"), ("tan", "tan"), ("ln", "ln")],
            [
                ("asin", "asin"),
                ("acos", "acos"),
                ("atan", "atan"),
                ("log", "log"),
            ],
            [
                ("sinh", "sinh"),
                ("cosh", "cosh"),
                ("tanh", "tanh"),
                ("exp", "exp"),
            ],
        ] {
            definitions.push(
                entries
                    .into_iter()
                    .map(|(label, name)| (label, Action::Function(name.into()), KeyStyle::Function))
                    .collect(),
            );
        }
        definitions.push(vec![
            ("π", Action::Insert("pi".into()), KeyStyle::Function),
            ("e", Action::Insert("e".into()), KeyStyle::Function),
            ("xʸ", Action::Insert("^".into()), KeyStyle::Function),
            ("n!", Action::Suffix("!".into()), KeyStyle::Function),
        ]);
        definitions.push(vec![
            ("√", Action::Function("sqrt".into()), KeyStyle::Function),
            ("∛", Action::Function("cbrt".into()), KeyStyle::Function),
            ("|x|", Action::Function("abs".into()), KeyStyle::Function),
            ("mod", Action::Insert(" mod ".into()), KeyStyle::Function),
        ]);
        definitions.push(vec![
            ("nCr", Action::Insert("ncr(".into()), KeyStyle::Function),
            ("nPr", Action::Insert("npr(".into()), KeyStyle::Function),
            (",", Action::Insert(",".into()), KeyStyle::Function),
            ("log₂", Action::Function("log2".into()), KeyStyle::Function),
        ]);
        self.key_grid(definitions, height, gap, self.scientific_scroll.clone())
    }
    fn key_grid(
        &self,
        definitions: Vec<Vec<(&str, Action, KeyStyle)>>,
        height: f32,
        gap: f32,
        scroll: ScrollController,
    ) -> BoxedWidget {
        let rows = definitions
            .into_iter()
            .map(|entries| {
                view(
                    fixed_height(height, row(gap)),
                    entries
                        .into_iter()
                        .map(|(label, action, style)| {
                            self.key_button(label, action, style, height, false)
                        })
                        .collect(),
                )
            })
            .collect();
        Box::new(
            ScrollView::controlled(grow(column(0.0)), scroll)
                .content_gap(gap)
                .with_children(rows),
        )
    }
    fn history(&self) -> BoxedWidget {
        let session = self.session.borrow();
        let mut rows = vec![];
        if session.history.is_empty() {
            rows.push(view(
                Style {
                    padding: layout::Rect::length(20.0_f32),
                    ..column(8.0)
                },
                vec![
                    Box::new(Text::new("No calculations yet").font_size(20.0)),
                    Box::new(Text::secondary("Calculations and conversions appear here.")),
                ],
            ));
        }
        for entry in &session.history {
            let expression: String = entry.expression.chars().take(54).collect();
            rows.push(Box::new(
                RawView::new(Style {
                    padding: layout::Rect::length(12.0_f32),
                    ..column(8.0)
                })
                .background(use_theme().colors.surface_elevated)
                .corner_radius(use_theme().card_radius)
                .with_children(vec![
                    Box::new(
                        Text::secondary(format!("{expression}  ·  {}", entry.detail))
                            .font_size(12.0),
                    ),
                    self.key_button(
                        &format!("= {}", format_number(entry.result)),
                        Action::Insert(entry.result.to_string()),
                        KeyStyle::Function,
                        40.0,
                        false,
                    ),
                ]),
            ) as BoxedWidget);
        }
        view(
            grow(column(12.0)),
            vec![
                view(
                    fixed_height(32.0, row(8.0)),
                    vec![
                        Box::new(Text::new("History").font_size(18.0)),
                        view(grow(row(0.0)), vec![]),
                        view(
                            fixed(52.0, 28.0),
                            vec![self.key_button(
                                "Back",
                                Action::History,
                                KeyStyle::Quiet,
                                28.0,
                                false,
                            )],
                        ),
                        view(
                            fixed(90.0, 28.0),
                            vec![self.key_button(
                                "Clear history",
                                Action::ClearHistory,
                                KeyStyle::Quiet,
                                28.0,
                                session.history.is_empty(),
                            )],
                        ),
                    ],
                ),
                Box::new(
                    ScrollView::controlled(grow(column(0.0)), self.history_scroll.clone())
                        .content_gap(8.0)
                        .with_children(rows),
                ),
            ],
        )
    }
}
fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        gap: layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        min_size: layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    }
}
fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        ..row(gap)
    }
}
fn grow(mut style: Style) -> Style {
    style.flex_grow = 1.0;
    style.flex_basis = Dimension::Length(0.0);
    style
}
fn fixed_height(height: f32, mut style: Style) -> Style {
    style.size.height = Dimension::Length(height);
    style.flex_shrink = 0.0;
    style
}
fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}
fn view(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(RawView::new(style).with_children(children))
}
