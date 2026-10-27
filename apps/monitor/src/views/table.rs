use super::*;

#[derive(Clone)]
pub(super) struct Column {
    pub title: String,
    pub summary: String,
    pub width: f32,
    pub weight: f32,
    pub end: bool,
    pub sort: Option<Sort>,
}
impl Column {
    pub fn fixed(
        title: &str,
        summary: impl Into<String>,
        width: f32,
        end: bool,
        sort: Option<Sort>,
    ) -> Self {
        Self {
            title: title.into(),
            summary: summary.into(),
            width,
            weight: 0.0,
            end,
            sort,
        }
    }
    pub fn flex(title: &str, summary: impl Into<String>, weight: f32, sort: Option<Sort>) -> Self {
        Self {
            title: title.into(),
            summary: summary.into(),
            width: 0.0,
            weight,
            end: false,
            sort,
        }
    }
    fn style(&self) -> Style {
        Style {
            flex_grow: self.weight,
            flex_basis: Dimension::Length(self.width),
            flex_shrink: 0.0,
            ..column(0.0)
        }
    }
}
pub(super) struct Row {
    pub key: String,
    pub values: Vec<String>,
    pub selected: bool,
    pub select: Rc<dyn Fn()>,
}

pub(super) fn build(
    state: &State,
    key: &str,
    size: Size,
    columns: Vec<Column>,
    rows: Vec<Row>,
) -> BoxedWidget {
    let t = use_theme();
    let (sort, descending) = state.sort.get();
    let header = Box::new(
        RawView::new(pad(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(52.0),
                },
                flex_shrink: 0.0,
                ..row(12.0)
            },
            14.0,
            0.0,
        ))
        .background(card_color())
        .with_children(
            columns
                .iter()
                .map(|column| {
                    let title = if column.sort == Some(sort) {
                        format!("{} {}", column.title, if descending { "↓" } else { "↑" })
                    } else {
                        column.title.clone()
                    };
                    let color = t.colors.text_secondary;
                    let title = Line::new(title, 11.0, color);
                    let summary = Line::new(&column.summary, 15.0, t.colors.text_primary);
                    let content = vec![
                        Box::new(if column.end { summary.end() } else { summary }) as BoxedWidget,
                        Box::new(if column.end { title.end() } else { title }) as BoxedWidget,
                    ];
                    if let Some(next) = column.sort {
                        let selection = state.sort.clone();
                        Box::new(
                            RawButton::new(column.style(), move || {
                                let (old, descending) = selection.peek();
                                selection.set((
                                    next,
                                    if old == next {
                                        !descending
                                    } else {
                                        next != Sort::Name
                                    },
                                ));
                            })
                            .hover_style(StateStyle::new().background(t.colors.surface_hover))
                            .focus_style(StateStyle::new().outline(t.colors.accent, 1.0))
                            .with_children(content),
                        ) as BoxedWidget
                    } else {
                        view(column.style(), content)
                    }
                })
                .collect(),
        ),
    );
    let body = if rows.is_empty() {
        state.scroll(key).report_max_offset(0.0);
        state.scroll(key).set(0.0);
        view(
            fill(),
            vec![empty(
                "Nothing to show",
                "Try changing the search or the selected filter.",
            )],
        )
    } else {
        let height = (30.0 * use_theme().text_scale).max(30.0);
        let controller = state.scroll(key);
        let first = ((controller.offset() / height).floor() as usize)
            .saturating_sub(3)
            .min(rows.len());
        let last = (((controller.offset() + size.height.max(height)) / height).ceil() as usize + 3)
            .min(rows.len());
        // Fixed-height rows stay in normal flow. Absolute virtual rows are treated
        // as portals by the native renderer and escape scroll clips/hit bounds.
        let mut visible = vec![view(fixed(0.0, first as f32 * height), vec![])];
        visible.extend((first..last).map(|index| {
            let item = &rows[index];
            let select = item.select.clone();
            creamui_core::keyed(
                RawButton::new(
                    pad(
                        Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Percent(1.0),
                                height: Dimension::Length(height),
                            },
                            flex_shrink: 0.0,
                            ..row(12.0)
                        },
                        14.0,
                        0.0,
                    ),
                    move || select(),
                )
                .background(if item.selected {
                    t.colors.accent.with_alpha(0.13)
                } else if index % 2 == 1 {
                    t.colors.text_secondary.with_alpha(0.035)
                } else {
                    CLEAR
                })
                .hover_style(StateStyle::new().background(t.colors.surface_hover))
                .focus_style(StateStyle::new().outline(t.colors.accent, 1.0))
                .with_children(
                    item.values
                        .iter()
                        .zip(columns.iter())
                        .map(|(value, column)| {
                            let text = Line::new(value, 12.0, t.colors.text_primary);
                            view(
                                column.style(),
                                vec![Box::new(if column.end { text.end() } else { text })],
                            )
                        })
                        .collect(),
                ),
                item.key.clone(),
            )
        }));
        visible.push(view(
            fixed(0.0, (rows.len() - last) as f32 * height),
            vec![],
        ));
        Box::new(
            RawScrollView::controlled(fill(), controller)
                .rebuild_on_scroll(true)
                .child(view(column(0.0), visible)),
        ) as BoxedWidget
    };
    view(fill(), vec![header, divider(), body])
}
