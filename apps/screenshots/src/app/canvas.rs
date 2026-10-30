use super::{
    hud::{self, HudId, Tool},
    State,
};
use creamui_core::{
    layout::{self, Dimension},
    CursorIcon, Painter, Point, Rect, Style, TextAlign, Widget,
};
use creamui_theme::Color;
use creamui_widgets::Icon;
use std::rc::Rc;

const DIM: Color = Color::rgba(0, 0, 0, 150);
const PANEL: Color = Color::rgba(24, 24, 27, 235);
const PANEL_TEXT: Color = Color::rgb(230, 230, 235);
const INK: Color = Color::rgb(250, 250, 250);
const ACCENT: Color = Color::rgb(51, 154, 240);
const HANDLE: f32 = 9.0;

pub(super) struct Overlay {
    pub state: State,
}

impl Overlay {
    fn paint_dim(&self, painter: &mut dyn Painter, rect: Rect, selection: Option<Rect>) {
        let Some(selection) = selection else {
            painter.fill_rect(rect, DIM, 0.0);
            return;
        };
        let top = Rect {
            height: selection.y - rect.y,
            ..rect
        };
        let bottom = Rect {
            y: selection.y + selection.height,
            height: (rect.y + rect.height) - (selection.y + selection.height),
            ..rect
        };
        let left = Rect {
            y: selection.y,
            height: selection.height,
            width: selection.x - rect.x,
            ..rect
        };
        let right = Rect {
            x: selection.x + selection.width,
            y: selection.y,
            width: (rect.x + rect.width) - (selection.x + selection.width),
            height: selection.height,
        };
        for dim in [top, bottom, left, right] {
            if dim.width > 0.0 && dim.height > 0.0 {
                painter.fill_rect(dim, DIM, 0.0);
            }
        }
    }

    fn paint_selection(&self, painter: &mut dyn Painter, selection: Rect) {
        painter.stroke_rect(
            Rect {
                x: selection.x - 0.5,
                y: selection.y - 0.5,
                width: selection.width + 1.0,
                height: selection.height + 1.0,
            },
            INK,
            1.0,
            0.0,
        );
        for handle in super::HANDLES {
            let anchor = handle.anchor(selection);
            let rect = Rect {
                x: anchor.x - HANDLE / 2.0,
                y: anchor.y - HANDLE / 2.0,
                width: HANDLE,
                height: HANDLE,
            };
            painter.fill_rect(rect, INK, 2.0);
            painter.stroke_rect(rect, Color::rgb(30, 30, 34), 1.0, 2.0);
        }
        let label = format!(
            "{} × {}",
            (selection.width / self.state.transform().0).round() as u32,
            (selection.height / self.state.transform().0).round() as u32,
        );
        let chip = Rect {
            x: selection.x,
            y: (selection.y - 22.0).max(4.0),
            width: 96.0,
            height: 18.0,
        };
        painter.fill_rect(chip, PANEL, 4.0);
        painter.fill_text(chip, &label, PANEL_TEXT, 11.0, TextAlign::Center);
    }

    fn paint_hud(&self, painter: &mut dyn Painter, rect: Rect) {
        let model = self.state.model();
        let selection = self.state.selection_view();
        let (bar, items) = hud::layout(
            selection,
            creamui_core::Size {
                width: rect.width,
                height: rect.height,
            },
        );
        painter.draw_box_shadow(bar, Color::rgba(0, 0, 0, 90), 0.0, 3.0, 14.0, 0.0, 10.0);
        painter.fill_rect(bar, PANEL, 10.0);
        painter.stroke_rect(bar, Color::rgba(255, 255, 255, 28), 1.0, 10.0);
        let pointer = self.state.pointer.get();
        for (id, item) in items {
            let hovered = item.contains(pointer);
            let active = match id {
                HudId::Tool(tool) => model.tool == tool,
                HudId::Color(index) => model.tool == Tool::Brush && model.color == index,
                HudId::Radius(index) => model.radius == index,
                _ => false,
            };
            if hovered || active {
                painter.fill_rect(
                    item,
                    if active {
                        Color::rgba(255, 255, 255, 40)
                    } else {
                        Color::rgba(255, 255, 255, 22)
                    },
                    7.0,
                );
            }
            match id {
                HudId::Tool(tool) => {
                    let source = match tool {
                        Tool::Select => model.icons.select.clone(),
                        Tool::Brush => model.icons.brush.clone(),
                        Tool::Blur => model.icons.blur.clone(),
                        Tool::Eraser => model.icons.eraser.clone(),
                    };
                    Icon::draw(
                        source,
                        painter,
                        inset(item, 6.0),
                        if active { ACCENT } else { PANEL_TEXT },
                    );
                }
                HudId::Color(index) => {
                    let [r, g, b] = hud::PALETTE[index];
                    let swatch = inset(item, 7.0);
                    painter.fill_rect(swatch, Color::rgb(r, g, b), 4.0);
                    if active {
                        painter.stroke_rect(swatch.inflate(1.5), INK, 1.5, 6.0);
                    }
                }
                HudId::Radius(index) => {
                    let radius = 2.0 + index as f32 * 2.5;
                    let center = Point {
                        x: item.x + item.width / 2.0,
                        y: item.y + item.height / 2.0,
                    };
                    painter.fill_rect(
                        Rect {
                            x: center.x - radius,
                            y: center.y - radius,
                            width: radius * 2.0,
                            height: radius * 2.0,
                        },
                        if active { ACCENT } else { PANEL_TEXT },
                        radius,
                    );
                }
                HudId::Copy => Icon::draw(
                    model.icons.copy.clone(),
                    painter,
                    inset(item, 6.0),
                    icon_ink(hovered),
                ),
                HudId::Save => Icon::draw(
                    model.icons.save.clone(),
                    painter,
                    inset(item, 6.0),
                    icon_ink(hovered),
                ),
                HudId::SaveAs => Icon::draw(
                    model.icons.save_as.clone(),
                    painter,
                    inset(item, 6.0),
                    icon_ink(hovered),
                ),
                HudId::Close => Icon::draw(
                    model.icons.close.clone(),
                    painter,
                    inset(item, 6.0),
                    icon_ink(hovered),
                ),
            }
        }
        if let Some(status) = &model.status {
            let chip = Rect {
                x: rect.x + rect.width / 2.0 - 220.0,
                y: rect.y + 14.0,
                width: 440.0,
                height: 26.0,
            };
            painter.fill_rect(chip, PANEL, 8.0);
            painter.fill_text(chip, status, PANEL_TEXT, 12.0, TextAlign::Center);
        }
    }
}

fn inset(rect: Rect, amount: f32) -> Rect {
    rect.inflate(-amount)
}

fn icon_ink(hovered: bool) -> Color {
    if hovered {
        INK
    } else {
        PANEL_TEXT
    }
}

impl Widget for Overlay {
    fn style(&self) -> Style {
        layout::Style {
            size: layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            ..Default::default()
        }
        .into()
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.state.viewport.set(rect);
        false
    }

    fn clips_children(&self) -> bool {
        true
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let state = &self.state;
        let model = state.model();
        let image_rect = state.image_rect();
        painter.fill_rect(rect, Color::rgb(10, 10, 12), 0.0);
        painter.draw_image(image_rect, &model.image, None);
        let annotations = model.annotation_image();
        if let Some(annotations) = annotations {
            painter.draw_image(image_rect, &annotations, None);
        }
        let selection = state.selection_view();
        self.paint_dim(painter, rect, selection);
        if let Some(selection) = selection {
            self.paint_selection(painter, selection);
        }
        drop(model);
        self.paint_hud(painter, rect);
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let state = self.state.clone();
        Some(Rc::new(move |point, widget| {
            state.drag_start_at(Point {
                x: widget.x + point.x,
                y: widget.y + point.y,
            })
        }))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let state = self.state.clone();
        Some(Rc::new(move |point, widget| {
            state.drag_to_at(Point {
                x: widget.x + point.x,
                y: widget.y + point.y,
            })
        }))
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        let state = self.state.clone();
        Some(Rc::new(move || state.drag_finished()))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Default)
    }
}
