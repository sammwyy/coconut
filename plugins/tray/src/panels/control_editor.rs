use crate::control_layout::COLUMNS;
use crate::{Control, ControlLayout, ControlTile};
use coconut_plugin_kit::chrome::{
    shell_border, shell_muted, shell_panel, shell_text, ISLAND_RADIUS,
};
use creamui_core::layout::{Dimension, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Key, Point, Size, Style, Styled};
use creamui_reactive::{batch, Signal};
use creamui_render::AppHandle;
use creamui_widgets::{
    layout::{Align, Flex, Justify, StyleExt},
    Button, ButtonSize, ButtonState, ButtonVariant, ContextMenuController, ContextMenuItem,
    RawButton, RawMarquee, RawText, RawView, ScrollController,
};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) const GAP: f32 = 10.0;
pub(super) const ROW_HEIGHT: f32 = 100.0;

#[derive(Clone)]
pub(super) struct Editor {
    pub layout: Signal<ControlLayout>,
    pub editing: Signal<bool>,
    pub busy: Signal<bool>,
    pub error: Signal<Option<String>>,
    pub menu: ContextMenuController,
    pub scroll: ScrollController,
    pub preview: Signal<Option<ControlTile>>,
    pending: Rc<RefCell<Option<ControlLayout>>>,
    drag: Rc<RefCell<Option<(Point, ControlLayout)>>>,
}

impl Editor {
    pub fn new(layout: ControlLayout) -> Self {
        Self {
            layout: Signal::new(layout),
            editing: Signal::new(false),
            busy: Signal::new(false),
            error: Signal::new(None),
            menu: ContextMenuController::new(),
            scroll: ScrollController::new(0.0),
            preview: Signal::new(None),
            pending: Rc::new(RefCell::new(None)),
            drag: Rc::new(RefCell::new(None)),
        }
    }

    pub fn cancel(&self, saved: &Signal<ControlLayout>) {
        if self.busy.peek() {
            return;
        }
        batch(|| {
            self.drag.borrow_mut().take();
            self.pending.borrow_mut().take();
            self.preview.set(None);
            self.layout.set(saved.peek());
            self.editing.set(false);
            self.error.set(None);
            self.menu.close();
        });
    }

    pub fn toolbar(
        &self,
        saved: Signal<ControlLayout>,
        app: Option<AppHandle>,
        available: Vec<Control>,
    ) -> BoxedWidget {
        if !self.editing.get() {
            let editor = self.clone();
            return action("Edit", false, move || editor.editing.set(true));
        }
        let editor = self.clone();
        let add = action("Add", self.busy.get(), move || {
            editor.open_add(Point { x: 16.0, y: 110.0 }, &available);
        });
        let editor = self.clone();
        let original = saved.clone();
        let cancel = action("Cancel", self.busy.get(), move || editor.cancel(&original));
        let editor = self.clone();
        let reset = action("Reset", self.busy.get(), move || {
            editor.drag.borrow_mut().take();
            editor.pending.borrow_mut().take();
            batch(|| {
                editor.preview.set(None);
                editor.layout.set(ControlLayout::default());
                editor.scroll.set(0.0);
            });
        });
        let editor = self.clone();
        let done = action(
            if self.busy.get() { "Saving…" } else { "Done" },
            self.busy.get(),
            move || {
                if editor.busy.peek() {
                    return;
                }
                let layout = editor.layout.peek();
                let Some(app) = &app else {
                    batch(|| {
                        saved.set(layout);
                        editor.editing.set(false);
                    });
                    return;
                };
                editor.busy.set(true);
                editor.error.set(None);
                let editor = editor.clone();
                let saved = saved.clone();
                app.spawn_background(
                    move || {
                        coconut_core::modules::save_module("control-center", &layout)
                            .map(|()| layout)
                            .map_err(|error| error.to_string())
                    },
                    move |result| {
                        batch(|| {
                            editor.busy.set(false);
                            match result {
                                Ok(layout) => {
                                    saved.set(layout);
                                    editor.editing.set(false);
                                }
                                Err(error) => {
                                    eprintln!("control center: could not save layout: {error}");
                                    editor
                                        .error
                                        .set(Some(format!("Could not save layout: {error}")));
                                }
                            }
                        })
                    },
                );
            },
        );
        Box::new(
            Flex::row()
                .gap(8.0)
                .align(Align::Center)
                .child(add)
                .child(reset)
                .child(Box::new(Flex::row().grow(1.0)))
                .child(cancel)
                .child(done),
        )
    }

    pub fn open_add(&self, point: Point, available: &[Control]) {
        if self.busy.peek() {
            return;
        }
        self.editing.set(true);
        let layout = self.layout.peek();
        let items = available
            .iter()
            .map(|control| {
                let control = *control;
                let editor = self.clone();
                ContextMenuItem::new(control.label(), control.label(), move || {
                    editor.layout.update(|layout| layout.add(control));
                })
                .enabled(!layout.tiles.iter().any(|tile| tile.control == control))
            })
            .collect();
        self.menu.open(point, items);
    }

    pub fn open_tile(&self, point: Point, tile: ControlTile) {
        if self.busy.peek() {
            return;
        }
        self.editing.set(true);
        let dimension = |width: bool| {
            (1..=if width { COLUMNS } else { 3 })
                .map(|size| {
                    let editor = self.clone();
                    let mut next = tile;
                    if width {
                        next.width = size;
                    } else {
                        next.height = size;
                    }
                    ContextMenuItem::new(size.to_string(), size.to_string(), move || {
                        editor.layout.update(|layout| layout.place(next));
                    })
                    .enabled(if width {
                        tile.width != size
                    } else {
                        tile.height != size
                    })
                })
                .collect()
        };
        let moves = [
            ("left", "Left", -1, 0),
            ("right", "Right", 1, 0),
            ("up", "Up", 0, -1),
            ("down", "Down", 0, 1),
        ]
        .into_iter()
        .map(|(id, label, dx, dy)| {
            let editor = self.clone();
            ContextMenuItem::new(id, label, move || editor.move_tile(tile, dx, dy, false)).enabled(
                match id {
                    "left" => tile.column > 0,
                    "right" => tile.column + tile.width < COLUMNS,
                    "up" => tile.row > 0,
                    _ => tile.row < crate::control_layout::MAX_ROW,
                },
            )
        })
        .collect();
        let editor = self.clone();
        let presets = [
            ("Small · 1 × 1", 1, 1),
            ("Medium · 2 × 1", 2, 1),
            ("Large · 2 × 2", 2, 2),
            ("Tall · 1 × 2", 1, 2),
            ("Wide · 4 × 1", COLUMNS, 1),
        ]
        .into_iter()
        .map(|(label, width, height)| {
            let editor = self.clone();
            let next = ControlTile {
                width,
                height,
                ..tile
            };
            ContextMenuItem::new(label, label, move || {
                editor.layout.update(|layout| layout.place(next))
            })
            .enabled(tile.width != width || tile.height != height)
        })
        .collect();
        self.menu.open(
            point,
            vec![
                ContextMenuItem::submenu("width", "Width", dimension(true)),
                ContextMenuItem::submenu("height", "Height", dimension(false)),
                ContextMenuItem::submenu("size", "Size", presets),
                ContextMenuItem::submenu("move", "Move", moves),
                ContextMenuItem::separator(),
                ContextMenuItem::new("remove", "Remove control", move || {
                    editor
                        .layout
                        .update(|layout| layout.tiles.retain(|item| item.control != tile.control));
                }),
            ],
        );
    }

    fn move_tile(&self, mut tile: ControlTile, dx: i32, dy: i32, resize: bool) {
        if self.busy.peek() {
            return;
        }
        if resize {
            tile.width = (i32::from(tile.width) + dx).clamp(1, i32::from(COLUMNS)) as u8;
            tile.height = (i32::from(tile.height) + dy).clamp(1, 3) as u8;
        } else {
            tile.column =
                (i32::from(tile.column) + dx).clamp(0, i32::from(COLUMNS - tile.width)) as u8;
            tile.row = (i32::from(tile.row) + dy)
                .clamp(0, i32::from(crate::control_layout::MAX_ROW)) as u16;
        }
        self.layout.update(|layout| layout.place(tile));
    }

    pub fn tile(&self, tile: ControlTile, size: Size, column_width: f32) -> BoxedWidget {
        let editor = self.clone();
        let resize = super::resize_handle::ResizeHandle::new(
            self.drag_button(tile, column_width, true).with_style(
                Style::new()
                    .layout(LayoutStyle {
                        size: creamui_widgets::layout::fixed(28.0, 28.0),
                        ..Default::default()
                    })
                    .corner_radius(6.0)
                    .hover(
                        creamui_core::StateStyle::new()
                            .background(coconut_plugin_kit::chrome::shell_control_hover()),
                    ),
            ),
        );
        let icon = match tile.control {
            Control::Wifi => "wifi-excellent",
            Control::Bluetooth => "bluetooth-on",
            Control::Battery => "battery-full",
            Control::KeepAwake => "monitor",
            Control::Brightness => "brightness",
            Control::Volume => "volume-high",
        };
        let compact = size.width < 100.0;
        let padding = if compact { 6.0 } else { 8.0 };
        let column_header = size.width < 150.0;
        let icon_size = if compact {
            if size.height > 150.0 {
                24.0
            } else {
                18.0
            }
        } else if size.height > 150.0 {
            28.0
        } else {
            20.0
        };
        let label_width =
            (size.width - padding * 2.0 - 2.0 - if column_header { 0.0 } else { icon_size + 6.0 })
                .max(1.0);
        let header = if column_header {
            Flex::column()
        } else {
            Flex::row().align(Align::Center)
        }
        .gap(if compact { 4.0 } else { 6.0 })
        .child(coconut_plugin_kit::chrome::icon_badge(icon, icon_size))
        .child(Box::new(RawMarquee::new(
            tile.control.label(),
            shell_text(),
            if compact { 10.0 } else { 12.0 },
            label_width,
        )));
        let mut footer = if compact {
            Flex::column().align(Align::End).gap(2.0)
        } else {
            Flex::row().align(Align::Center)
        };
        footer = footer.child(Box::new(RawText::new(
            format!("{}×{}", tile.width, tile.height),
            shell_muted(),
            if compact { 10.0 } else { 11.0 },
        )));
        if !compact {
            footer = footer.child(Box::new(Flex::row().grow(1.0)));
        }
        footer = footer.child(Box::new(resize));
        let body = Flex::column()
            .size(size.width, size.height)
            .padding(padding)
            .gap(6.0)
            .justify(Justify::Between)
            .child(Box::new(header))
            .child(Box::new(footer));
        Box::new(
            self.drag_button(tile, column_width, false)
                .with_context_menu(move |point| editor.open_tile(point, tile))
                .child(Box::new(body)),
        )
    }

    fn drag_button(&self, tile: ControlTile, column_width: f32, resize: bool) -> RawButton {
        let editor = self.clone();
        let mouse_menu = self.clone();
        let start = self.clone();
        let drag = self.clone();
        let end = self.clone();
        let key = self.clone();
        RawButton::new(
            super::control_tiles::tile_style(Size {
                width: 0.0,
                height: 0.0,
            })
            .layout(LayoutStyle::default()),
            move || editor.open_tile(Point { x: 24.0, y: 120.0 }, tile),
        )
        .with_click_position(move |point| mouse_menu.open_tile(point, tile))
        .disabled(self.busy.get())
        .with_drag_start(move |point, rect| {
            *start.drag.borrow_mut() = Some((
                Point {
                    x: point.x + rect.x,
                    y: point.y + rect.y,
                },
                start.layout.peek(),
            ));
            start.pending.borrow_mut().take();
        })
        .with_drag(move |point, rect| {
            let point = Point {
                x: point.x + rect.x,
                y: point.y + rect.y,
            };
            let initial = drag.drag.borrow().clone();
            if let Some((anchor, mut layout)) = initial {
                let dx = ((point.x - anchor.x) / (column_width + GAP)).round() as i32;
                let dy = ((point.y - anchor.y) / (ROW_HEIGHT + GAP)).round() as i32;
                let mut next = tile;
                if resize {
                    next.width = (i32::from(tile.width) + dx).clamp(1, i32::from(COLUMNS)) as u8;
                    next.height = (i32::from(tile.height) + dy).clamp(1, 3) as u8;
                } else {
                    next.column = (i32::from(tile.column) + dx)
                        .clamp(0, i32::from(COLUMNS - tile.width))
                        as u8;
                    next.row = (i32::from(tile.row) + dy)
                        .clamp(0, i32::from(crate::control_layout::MAX_ROW))
                        as u16;
                }
                if next != tile {
                    layout.place(next);
                    *drag.pending.borrow_mut() = Some(layout);
                    if drag.preview.peek() != Some(next) {
                        drag.preview.set(Some(next));
                    }
                } else {
                    drag.pending.borrow_mut().take();
                    if drag.preview.peek().is_some() {
                        drag.preview.set(None);
                    }
                }
            }
        })
        .with_drag_end(move || {
            end.drag.borrow_mut().take();
            let pending = end.pending.borrow_mut().take();
            batch(|| {
                if let Some(layout) = pending {
                    end.layout.set(layout);
                }
                end.preview.set(None);
            });
        })
        .on_key_press(move |input| {
            let delta = match input.key {
                Key::Left => Some((-1, 0)),
                Key::Right => Some((1, 0)),
                Key::Up => Some((0, -1)),
                Key::Down => Some((0, 1)),
                Key::Delete => {
                    key.layout
                        .update(|layout| layout.tiles.retain(|item| item.control != tile.control));
                    None
                }
                _ => None,
            };
            if let Some((dx, dy)) = delta {
                key.move_tile(tile, dx, dy, input.modifiers.shift);
            }
        })
    }
}

fn action(label: &str, disabled: bool, callback: impl Fn() + 'static) -> BoxedWidget {
    Box::new(
        Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            label,
            ButtonState::Normal,
            callback,
        )
        .disabled(disabled),
    )
}

pub(super) fn positioned(
    child: BoxedWidget,
    tile: ControlTile,
    size: Size,
    key: &str,
) -> BoxedWidget {
    creamui_core::keyed(
        RawView::new(
            LayoutStyle {
                size: creamui_widgets::layout::fixed(size.width, size.height),
                ..Default::default()
            }
            .grid_cell(i16::from(tile.column) + 1, tile.row as i16 + 1)
            .grid_column_span(u16::from(tile.width))
            .grid_row_span(u16::from(tile.height)),
        )
        .child(child),
        key,
    )
}

pub(super) fn container(size: Size) -> Flex {
    Flex::column()
        .size(size.width, size.height)
        .property(creamui_core::StyleProp::Background(shell_panel().into()))
        .property(creamui_core::StyleProp::Border(creamui_core::Border::new(
            shell_border(),
            1.0,
        )))
        .property(creamui_core::StyleProp::CornerRadius(ISLAND_RADIUS))
}

pub(super) fn scroll_style(width: f32, height: f32) -> Style {
    Style::new().layout(LayoutStyle {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        ..Default::default()
    })
}
