use super::*;
use creamui_core::{
    layout::{self, AlignItems, Dimension, FlexDirection, LengthPercentage, Style},
    BoxedWidget, StateStyle, Styled, TextAlign,
};
use creamui_theme::use_theme;
use creamui_widgets::{
    AppTitlebar, Button, ButtonSize, ButtonState, ButtonVariant, Checkbox, CommandPalette, Dialog,
    Icon, RawView, ScrollView, Slider, Spinner, Symbol, Text,
};

impl State {
    fn button(
        &self,
        label: &str,
        action: Action,
        width: f32,
        disabled: bool,
        active: bool,
    ) -> BoxedWidget {
        let state = self.clone();
        let theme = use_theme();
        let background = if active && !disabled {
            theme.colors.accent.mix(theme.colors.surface, 0.88)
        } else {
            theme.colors.surface
        };
        Box::new(
            Button::styled(
                ButtonVariant::Tertiary,
                ButtonSize::Sm,
                label,
                ButtonState::Normal,
                move || state.run(action),
            )
            .disabled(disabled)
            .background(background)
            .border(background, 0.0)
            .corner_radius(theme.button_radius)
            .disabled_style(
                StateStyle::new()
                    .background(background)
                    .border(background, 0.0),
            )
            .hover_style(StateStyle::new().background(theme.colors.surface_hover))
            .customize(move |button| {
                button.style.layout.size.width = Dimension::Length(width);
                button.style.layout.size.height = Dimension::Length(32.0 * theme.text_scale);
                button.style.layout.padding = layout::Rect::length(0.0_f32);
            }),
        )
    }
    pub fn build(&self, size: Size) -> BoxedWidget {
        self.revision.get();
        let theme = use_theme();
        let panel = self.panel.get();
        let loading = self.loading.get();
        let dialog = self.dialog_open.get();
        let (asset, view, index, count, error) = {
            let model = self.model.borrow();
            (
                model.asset.clone(),
                model.view,
                model.index,
                model.paths.len(),
                model.error.clone(),
            )
        };
        let loaded = asset.is_some();
        let scale = theme.text_scale.max(1.0);
        let header_height = 42.0 * scale;
        let footer_height = 48.0 * scale;
        let body_height = (size.height - header_height - footer_height).max(0.0);
        let panel_width = if panel != Panel::None && loaded {
            (244.0 * scale).min(size.width * 0.44)
        } else {
            0.0
        };
        self.canvas.set(Rect {
            x: 0.0,
            y: header_height,
            width: (size.width - panel_width).max(1.0),
            height: body_height.max(1.0),
        });
        let title = asset.as_ref().map_or_else(
            || "Image Viewer".into(),
            |asset| {
                asset
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            },
        );
        let titlebar = AppTitlebar::new(size.width)
            .height(header_height)
            .center_margin(112.0);
        let (lead, trail) = titlebar.reserved();
        let capacity = ((size.width - 2.0 * (lead.max(trail) + 112.0)) / (7.0 * theme.text_scale))
            .max(4.0) as usize;
        let controls = Box::new(
            titlebar
                .start_boxed(self.button("Open", Action::Open, 52.0, dialog, false))
                .center(Text::new(short(&title, capacity)).font_size(12.0))
                .end_boxed(self.button(
                    "Adjust",
                    Action::Panel(Panel::Adjustments),
                    58.0,
                    !loaded,
                    panel == Panel::Adjustments,
                ))
                .end_boxed(self.button(
                    "Info",
                    Action::Panel(Panel::Information),
                    46.0,
                    !loaded,
                    panel == Panel::Information,
                )),
        ) as BoxedWidget;
        let stage = if let Some(asset) = &asset {
            Box::new(canvas::Canvas {
                state: self.clone(),
                asset: asset.clone(),
                view,
                background: theme.colors.surface,
                checker: [theme.colors.surface_hover, theme.colors.border],
                accent: theme.colors.accent,
            }) as BoxedWidget
        } else {
            self.empty(loading, error.as_deref())
        };
        let mut body = vec![stage];
        if panel_width > 0.0 {
            body.push(self.inspector(panel, asset.as_ref().unwrap(), view, panel_width));
        }
        let zoom = asset
            .as_ref()
            .map(|asset| view.layout(self.viewport(), asset.size()).scale * 100.0);
        let toolbar = vec![
            self.button("‹", Action::Previous, 30.0, index == 0, false),
            Box::new(
                Text::secondary(if count == 0 {
                    "0 / 0".into()
                } else {
                    format!("{} / {count}", index + 1)
                })
                .font_size(11.0)
                .text_align(TextAlign::Center)
                .width(84.0),
            ) as BoxedWidget,
            self.button("›", Action::Next, 30.0, index + 1 >= count, false),
            spacer(12.0),
            self.button("−", Action::ZoomOut, 30.0, !loaded, false),
            Box::new(
                Text::secondary(zoom.map_or_else(|| "—".into(), |value| format!("{value:.0}%")))
                    .font_size(12.0)
                    .text_align(TextAlign::Center)
                    .width(54.0),
            ),
            self.button("+", Action::ZoomIn, 30.0, !loaded, false),
            self.button("Fit", Action::Fit, 40.0, !loaded, view.zoom == Zoom::Fit),
            self.button(
                "1:1",
                Action::ActualSize,
                42.0,
                !loaded,
                view.zoom == Zoom::Scale(1.0),
            ),
            spacer(12.0),
            self.button("Rotate", Action::Rotate(true), 60.0, !loaded, false),
        ];
        let footer = view_box(
            Style {
                justify_content: Some(layout::JustifyContent::Center),
                size: layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(footer_height),
                },
                flex_shrink: 0.0,
                ..row(4.0)
            },
            toolbar,
        );
        let mut children = vec![
            controls,
            view_box(
                grow(Style {
                    align_items: Some(AlignItems::Stretch),
                    ..row(0.0)
                }),
                body,
            ),
            footer,
        ];
        if self.palette_open.get() {
            let state = self.clone();
            children.push(Box::new(
                CommandPalette::new(&self.palette, self.catalog(), move || {
                    state.run(Action::Dismiss)
                })
                .viewport(size),
            ));
        } else if loaded {
            if let Some(error) = error {
                let state = self.clone();
                let close = self.clone();
                children.push(Box::new(
                    Dialog::new("Could not open image", short(&error, 220), move || {
                        state.run(Action::Dismiss)
                    })
                    .action("Close", ButtonVariant::Primary, move || {
                        close.run(Action::Dismiss)
                    }),
                ));
            }
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
    fn empty(&self, loading: bool, error: Option<&str>) -> BoxedWidget {
        let theme = use_theme();
        let mut content = vec![];
        if loading {
            content.push(Box::new(Spinner::new().size(30.0)) as BoxedWidget);
            content.push(Box::new(Text::new("Opening image…").font_size(19.0)));
        } else if let Some(error) = error {
            content.push(Box::new(
                Icon::new(Symbol::Image, theme.colors.text_secondary).size(44.0),
            ));
            content.push(Box::new(
                Text::new("Could not open this image").font_size(20.0),
            ));
            content.push(Box::new(
                Text::secondary(short(error, 220))
                    .font_size(12.0)
                    .width("100%"),
            ));
            content.push(view_box(
                row(8.0),
                vec![
                    self.button("Try again", Action::Retry, 84.0, false, false),
                    self.button(
                        "Open another",
                        Action::Open,
                        106.0,
                        self.dialog_open.peek(),
                        false,
                    ),
                ],
            ));
        } else {
            content.push(Box::new(
                Icon::new(Symbol::Image, theme.colors.accent).size(48.0),
            ));
            content.push(Box::new(Text::new("Open an image").font_size(24.0)));
            content.push(Box::new(
                Text::secondary("Drop images here or choose a file.").font_size(13.0),
            ));
            let state = self.clone();
            content.push(Box::new(
                Button::new("Open image", move || state.run(Action::Open))
                    .disabled(self.dialog_open.peek()),
            ));
        }
        view_box(
            Style {
                align_items: Some(AlignItems::Center),
                justify_content: Some(layout::JustifyContent::Center),
                ..grow(column(0.0))
            },
            vec![view_box(
                Style {
                    max_size: layout::Size {
                        width: Dimension::Length(420.0),
                        height: Dimension::Auto,
                    },
                    padding: layout::Rect::length(24.0_f32),
                    align_items: Some(AlignItems::Center),
                    ..column(14.0)
                },
                content,
            )],
        )
    }
    fn inspector(&self, panel: Panel, asset: &Asset, view: View, width: f32) -> BoxedWidget {
        let theme = use_theme();
        let title = if panel == Panel::Adjustments {
            "Adjustments"
        } else {
            "Information"
        };
        let header = view_box(
            row(6.0),
            vec![
                Box::new(Text::new(title).font_size(17.0)),
                view_box(grow(row(0.0)), vec![]),
                self.button("Close", Action::Panel(panel), 46.0, false, false),
            ],
        );
        let content = if panel == Panel::Adjustments {
            self.adjustment_controls(view.adjustments)
        } else {
            self.information(asset)
        };
        Box::new(
            RawView::new(Style {
                size: layout::Size {
                    width: Dimension::Length(width),
                    height: Dimension::Percent(1.0),
                },
                padding: layout::Rect::length(14.0_f32),
                flex_shrink: 0.0,
                ..column(14.0)
            })
            .background(
                theme
                    .colors
                    .surface_elevated
                    .mix(theme.colors.surface, 0.65),
            )
            .with_children(vec![
                header,
                Box::new(
                    ScrollView::controlled(grow(column(0.0)), self.panel_scroll.clone())
                        .content_gap(16.0)
                        .with_children(content),
                ),
            ]),
        )
    }
    fn adjustment_controls(&self, adjustments: Adjustments) -> Vec<BoxedWidget> {
        let mut controls = vec![
            Box::new(Text::secondary("Only affects this view.").font_size(11.0)) as BoxedWidget,
        ];
        for (label, value, action) in [
            (
                "Brightness",
                adjustments.brightness,
                Action::Brightness as fn(f32) -> Action,
            ),
            ("Contrast", adjustments.contrast, Action::Contrast),
            (
                "Saturation",
                if adjustments.grayscale {
                    0.0
                } else {
                    adjustments.saturation
                },
                Action::Saturation,
            ),
        ] {
            let state = self.clone();
            controls.push(view_box(
                column(8.0),
                vec![
                    view_box(
                        row(6.0),
                        vec![
                            Box::new(Text::new(label).font_size(12.0)),
                            view_box(grow(row(0.0)), vec![]),
                            Box::new(
                                Text::secondary(format!("{:.0}%", value * 100.0)).font_size(11.0),
                            ),
                        ],
                    ),
                    Box::new(
                        Slider::new(value / 2.0, move |value| {
                            state.run(action((value * 200.0).round() / 100.0))
                        })
                        .width("100%"),
                    ),
                ],
            ));
        }
        for (label, checked, action) in [
            ("Black and white", adjustments.grayscale, Action::Grayscale),
            ("Invert colors", adjustments.invert, Action::Invert),
        ] {
            let state = self.clone();
            controls.push(view_box(
                row(10.0),
                vec![
                    Box::new(Checkbox::new(checked, move || state.run(action))),
                    Box::new(Text::new(label).font_size(12.0)),
                ],
            ));
        }
        let state = self.clone();
        controls.push(Box::new(
            Button::secondary(ButtonSize::Sm, "Reset adjustments", move || {
                state.run(Action::ResetAdjustments)
            })
            .disabled(adjustments == Adjustments::default())
            .width("100%"),
        ));
        controls
    }
    fn information(&self, asset: &Asset) -> Vec<BoxedWidget> {
        let metadata = &asset.metadata;
        let modified = metadata.modified.map_or_else(
            || "Unavailable".into(),
            |time| {
                chrono::DateTime::<chrono::Local>::from(time)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            },
        );
        let mut fields = vec![
            (
                "File".into(),
                asset
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "Dimensions".into(),
                format!("{} × {} px", asset.data.width(), asset.data.height()),
            ),
            ("Format".into(), metadata.format.clone()),
            ("Color".into(), metadata.color.clone()),
            (
                "File size".into(),
                format!(
                    "{} ({:.2} MiB)",
                    metadata.bytes,
                    metadata.bytes as f64 / (1024.0 * 1024.0)
                ),
            ),
            ("Modified".into(), modified),
            ("Path".into(), asset.path.display().to_string()),
            (
                "Source dimensions".into(),
                format!(
                    "{} × {} px",
                    metadata.original_dimensions.0, metadata.original_dimensions.1
                ),
            ),
            ("EXIF orientation".into(), metadata.orientation.clone()),
            (
                "ICC profile".into(),
                if metadata.icc_bytes == 0 {
                    "None".into()
                } else {
                    format!("Embedded ({} bytes)", metadata.icc_bytes)
                },
            ),
        ];
        fields.extend(metadata.exif.iter().cloned());
        if metadata.exif.is_empty() {
            fields.push(("EXIF".into(), "No EXIF fields".into()));
        }
        for warning in &metadata.warnings {
            fields.push(("Metadata notice".into(), warning.clone()));
        }
        fields
            .into_iter()
            .map(|(label, value)| {
                view_box(
                    column(4.0),
                    vec![
                        Box::new(Text::secondary(label).font_size(10.0)),
                        Box::new(Text::new(short(&value, 256)).font_size(12.0).width("100%")),
                    ],
                )
            })
            .collect()
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
fn view_box(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(RawView::new(style).with_children(children))
}
fn spacer(width: f32) -> BoxedWidget {
    view_box(
        Style {
            size: layout::Size {
                width: Dimension::Length(width),
                height: Dimension::Length(1.0),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },
        vec![],
    )
}
fn short(value: &str, capacity: usize) -> String {
    let mut text: String = value.chars().take(capacity).collect();
    if value.chars().count() > capacity {
        text.push('…');
    }
    text
}
