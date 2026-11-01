use super::{drag::Destination, State};
use coconut_core::{ordered_islands, BarEdit, DockConfig};
use coconut_plugin_kit::{
    bar_editor::{item_label, Target},
    Island, SharedState,
};
use creamui_core::{BoxedWidget, KeyInput, Painter, Point, Rect, Size, Style, Styled, Widget};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{
    layout::{Align, Flex},
    Button, ButtonSize, ButtonState, ButtonVariant, RawButton, RawText, RawView,
};
use std::rc::Rc;

pub fn build(
    viewport: Size,
    dock: &DockConfig,
    lookup: &dyn Fn(&str) -> Option<Rc<dyn Island>>,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
    context: &super::super::editing::Context,
) -> BoxedWidget {
    let state = &context.quick;
    state.drag.get();
    state.bounds.borrow_mut().clear();
    state.clips.borrow_mut().clear();
    state.viewport.set(Rect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    });
    let vertical = dock.position.is_vertical();
    let count = dock.sections.len().max(1);
    let length = if vertical {
        viewport.height
    } else {
        viewport.width
    };
    let slots = if dock.fill_available_space {
        count
    } else {
        count.max(3)
    };
    let section_length = ((length
        - if dock.show_border { 2.0 } else { 0.0 }
        - dock.section_gap.max(4.0) * count.saturating_sub(1) as f32)
        / slots as f32)
        .max(if vertical { 120.0 } else { 104.0 });
    let cross = dock.thickness.max(if vertical { 64.0 } else { 44.0 }) + 32.0;
    let mut groups = if vertical {
        Flex::column().width(dock.thickness.max(64.0) + 32.0)
    } else {
        Flex::row().height(dock.thickness.max(44.0) + 32.0)
    };
    groups = groups.gap(dock.section_gap.max(4.0));
    for (group, section) in dock.sections.iter().enumerate() {
        let leading = if group == 0 {
            dock.edge_gap.max(0.0)
        } else {
            0.0
        };
        let trailing = if group + 1 == count {
            dock.edge_gap.max(0.0)
        } else {
            0.0
        };
        let body_length = (section_length - 8.0 - leading - trailing).max(1.0);
        let state_add = state.clone();
        let state_remove = state.clone();
        let drag = drag_button(
            state,
            Target::Group(group),
            Style::new().width(20.0).height(20.0),
        )
        .child(coconut_plugin_kit::pixel_icon(
            "drag-handle",
            12.0,
            use_theme().text_primary,
        ));
        let header = Flex::row()
            .height(24.0)
            .align(Align::Center)
            .gap(2.0)
            .child(Box::new(drag))
            .child(Box::new(RawText::new(
                format!(
                    "{}{}",
                    group + 1,
                    if section.enabled || vertical {
                        ""
                    } else {
                        " · hidden"
                    }
                ),
                use_theme().text_secondary,
                10.0,
            )))
            .child(Box::new(Flex::row().grow(1.0)))
            .child(action("+", move || state_add.open_catalog(group)))
            .child(Box::new(
                Button::styled(
                    ButtonVariant::Tertiary,
                    ButtonSize::Xs,
                    "×",
                    ButtonState::Normal,
                    move || state_remove.apply(BarEdit::RemoveGroup(group)),
                )
                .disabled(!section.islands.is_empty() || state.persistence.busy.get()),
            ));
        let mut items = if vertical {
            Flex::column()
        } else {
            Flex::row()
        };
        let gap = coconut_plugin_kit::parse_gap(&section.gap);
        items = super::super::apply_gap(items.align(Align::Center), gap, body_length);
        if matches!(
            gap,
            coconut_plugin_kit::Gap::Between | coconut_plugin_kit::Gap::Evenly
        ) {
            items = if vertical {
                items.min_height(body_length)
            } else {
                items.min_width(body_length)
            };
        }
        for (item, entry) in ordered_islands(&section.islands).into_iter().enumerate() {
            let config = super::super::island_config(entry);
            let ctx = coconut_plugin_kit::IslandRenderContext {
                shared,
                config: &config,
                position: dock.position,
                scale: super::super::island_scale(dock),
                open_panel: open_panel.clone(),
            };
            let content = match lookup(&entry.id).filter(|island| island.is_visible(&ctx)) {
                Some(island) => coconut_plugin_kit::with_island_chrome(
                    coconut_plugin_kit::IslandChrome {
                        background: dock.show_island_background && !dock.unify_island_background,
                        border: dock.show_island_border && !dock.unify_island_background,
                        background_color: (!dock.unify_island_background)
                            .then(|| super::super::island_background_color(dock))
                            .flatten(),
                        background_opacity: dock.island_background_opacity,
                    },
                    || island.build(&ctx),
                ),
                None => Box::new(Flex::row().padding(8.0).child(Box::new(RawText::new(
                    item_label(&entry.id).to_owned(),
                    use_theme().text_secondary,
                    11.0,
                )))),
            };
            items = items.child(frame(state, Target::Item { group, item }, content, true));
        }
        if section.islands.is_empty() {
            items = items.child(Box::new(RawText::new(
                "Drop here",
                use_theme().text_secondary,
                11.0,
            )));
        }
        if dock.unify_island_background {
            items = super::super::section_chrome(
                items.padding(4.0),
                dock.show_island_background,
                dock.show_island_border,
                super::super::island_background_color(dock),
                dock.island_background_opacity,
                super::super::island_scale(dock),
            );
        }
        let mut group_view = Flex::column()
            .gap(2.0)
            .padding_xy(4.0, 2.0)
            .child(Box::new(header))
            .child(super::strip::Strip::wrap(
                state,
                group,
                vertical,
                if vertical {
                    Size {
                        width: cross - 8.0,
                        height: (body_length - 24.0).max(0.0),
                    }
                } else {
                    Size {
                        width: body_length,
                        height: cross - 32.0,
                    }
                },
                Box::new(items),
            ));
        if vertical {
            group_view = group_view
                .property(creamui_core::StyleProp::PaddingTop((2.0 + leading).into()))
                .property(creamui_core::StyleProp::PaddingBottom(
                    (2.0 + trailing).into(),
                ));
        } else {
            group_view = group_view
                .property(creamui_core::StyleProp::PaddingLeft((4.0 + leading).into()))
                .property(creamui_core::StyleProp::PaddingRight(
                    (4.0 + trailing).into(),
                ));
        }
        group_view = if vertical {
            group_view.height(section_length)
        } else {
            group_view.width(section_length)
        };
        groups = groups.child(frame(
            state,
            Target::Group(group),
            Box::new(group_view),
            false,
        ));
    }
    if dock.sections.is_empty() {
        let state = state.clone();
        groups = groups.child(action("+ Add section", move || {
            state.apply(BarEdit::AddGroup)
        }));
    }
    let groups = super::super::dock_chrome(groups, dock);
    let mut root = if vertical {
        Flex::row()
    } else {
        Flex::column()
    };
    root = root
        .size(viewport.width, viewport.height)
        .justify(super::super::edge_justify(dock.position));
    Box::new(root.child(super::strip::Strip::wrap(
        state,
        usize::MAX,
        vertical,
        if vertical {
            Size {
                width: cross,
                height: viewport.height,
            }
        } else {
            Size {
                width: viewport.width,
                height: cross,
            }
        },
        Box::new(groups),
    )))
}

fn action(label: &str, action: impl Fn() + 'static) -> BoxedWidget {
    Box::new(Button::styled(
        ButtonVariant::Tertiary,
        ButtonSize::Xs,
        label,
        ButtonState::Normal,
        action,
    ))
}

fn absolute() -> Style {
    let mut style = Style::new().position(creamui_core::layout::Position::Absolute);
    style.layout.inset.left = creamui_core::layout::LengthPercentageAuto::Length(0.0);
    style.layout.inset.top = creamui_core::layout::LengthPercentageAuto::Length(0.0);
    style
}

fn drag_button(state: &State, target: Target, style: Style) -> RawButton {
    let start = state.clone();
    let update = state.clone();
    let end = state.clone();
    let keys = state.clone();
    let button = RawButton::new(style, || {})
        .disabled(state.persistence.busy.get())
        .with_drag_start(move |point, rect| {
            start.start_drag(
                target,
                Point {
                    x: rect.x + point.x,
                    y: rect.y + point.y,
                },
            )
        })
        .with_drag(move |point, rect| {
            update.update_drag(Point {
                x: rect.x + point.x,
                y: rect.y + point.y,
            })
        })
        .with_drag_end(move || end.finish_drag())
        .on_key_press(move |input: KeyInput| keys.item_key(target, input))
        .focus_style(creamui_core::StateStyle::new().outline(use_theme().accent, 2.0));
    if let Some(context) = creamui_reactive::try_use_context::<super::super::editing::Context>() {
        button.with_context_menu(move |point| context.open(point, target))
    } else {
        button
    }
}

fn frame(state: &State, target: Target, content: BoxedWidget, item: bool) -> BoxedWidget {
    let mut style = creamui_core::layout::Style::default();
    if item {
        style.padding.right = creamui_core::layout::LengthPercentage::Length(20.0);
        if let Some(draft) = state
            .draft
            .peek()
            .filter(|draft| draft.current.position.is_vertical())
        {
            style.size.width = creamui_core::layout::Dimension::Length(
                draft.current.thickness.max(64.0) + 24.0
                    - if draft.current.unify_island_background {
                        8.0
                    } else {
                        0.0
                    },
            );
            style.min_size.width = creamui_core::layout::Dimension::Length(0.0);
        }
    }
    let mut root = RawView::new(style).child(content);
    if let Some(context) = creamui_reactive::try_use_context::<super::super::editing::Context>() {
        root = root.with_context_menu(move |point| context.open(point, target));
    }
    if item {
        root = root.child(Box::new(drag_button(
            state,
            target,
            absolute()
                .width(creamui_core::LengthValue::Percent(100.0))
                .height(creamui_core::LengthValue::Percent(100.0)),
        )));
        let remove = state.clone();
        let mut style = absolute().width(18.0).height(18.0);
        style.layout.inset.left = creamui_core::layout::LengthPercentageAuto::Auto;
        style.layout.inset.right = creamui_core::layout::LengthPercentageAuto::Length(0.0);
        root = root.child(Box::new(
            RawButton::new(style, move || {
                if let Target::Item { group, item } = target {
                    remove.apply(BarEdit::RemoveItem { group, item });
                }
            })
            .disabled(state.persistence.busy.get())
            .background(use_theme().surface_elevated)
            .corner_radius(6.0)
            .child(coconut_plugin_kit::pixel_icon(
                "close",
                12.0,
                use_theme().text_primary,
            )),
        ));
    }
    root = root.child(Box::new(Marker {
        state: state.clone(),
        target,
        color: use_theme().accent,
    }));
    let key = state
        .draft
        .peek()
        .and_then(|draft| match target {
            Target::Group(group) => draft.group_ids.get(group).copied(),
            Target::Item { group, item } => draft.item_ids.get(group)?.get(item).copied(),
            Target::Bar => None,
        })
        .map(|id| format!("quick-{id}"));
    Box::new(Measured {
        state: state.clone(),
        target,
        key,
        child: Some(Box::new(root)),
    })
}

struct Measured {
    state: State,
    target: Target,
    key: Option<String>,
    child: Option<BoxedWidget>,
}
impl Widget for Measured {
    fn key(&self) -> Option<creamui_core::WidgetKey> {
        self.key.clone().map(Into::into)
    }
    fn style(&self) -> Style {
        Style::new().flex_shrink(0.0)
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.state.bounds.borrow_mut().insert(self.target, rect);
        false
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.child.take().into_iter().collect()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}

struct Marker {
    state: State,
    target: Target,
    color: Color,
}
impl Widget for Marker {
    fn style(&self) -> Style {
        absolute()
            .width(creamui_core::LengthValue::Percent(100.0))
            .height(creamui_core::LengthValue::Percent(100.0))
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let destination = self.state.destination();
        painter.stroke_rect(
            rect,
            Color {
                a: if matches!(self.target, Target::Group(_)) {
                    100
                } else {
                    180
                },
                ..self.color
            },
            1.0,
            6.0,
        );
        let vertical = self
            .state
            .draft
            .peek()
            .is_some_and(|draft| draft.current.position.is_vertical());
        let after = match (self.target, destination) {
            (
                Target::Item { group, item },
                Some(Destination::Item {
                    group: to_group,
                    at,
                }),
            ) if group == to_group && (at == item || at == item + 1) => Some(at > item),
            (Target::Group(group), Some(Destination::Group(at)))
                if at == group || at == group + 1 =>
            {
                Some(at > group)
            }
            (
                Target::Group(group),
                Some(Destination::Item {
                    group: to_group, ..
                }),
            ) if group == to_group => {
                painter.stroke_rect(rect, self.color, 2.0, 6.0);
                None
            }
            _ => None,
        };
        if let Some(after) = after {
            painter.fill_rect(
                if vertical {
                    Rect {
                        x: rect.x,
                        y: if after {
                            rect.y + rect.height - 3.0
                        } else {
                            rect.y
                        },
                        width: rect.width,
                        height: 3.0,
                    }
                } else {
                    Rect {
                        x: if after {
                            rect.x + rect.width - 3.0
                        } else {
                            rect.x
                        },
                        y: rect.y,
                        width: 3.0,
                        height: rect.height,
                    }
                },
                self.color,
                1.0,
            );
        }
    }
}
