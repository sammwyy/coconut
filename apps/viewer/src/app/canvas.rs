use super::*;
use creamui_core::{
    layout::{self, Dimension, Style},
    CursorIcon, Painter, Widget,
};
use creamui_theme::Color;

pub(super) struct Canvas {
    pub state: State,
    pub asset: Arc<Asset>,
    pub view: View,
    pub background: Color,
    pub checker: [Color; 2],
    pub accent: Color,
}
impl Widget for Canvas {
    fn style(&self) -> creamui_core::Style {
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            min_size: layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        }
        .into()
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.state.canvas.set(rect);
        false
    }
    fn clips_children(&self) -> bool {
        true
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, self.background, 0.0);
        painter.push_clip(rect);
        let viewport = Size {
            width: rect.width,
            height: rect.height,
        };
        let layout = self.view.layout(viewport, self.asset.size());
        let image = Rect {
            x: rect.x + layout.image.x,
            y: rect.y + layout.image.y,
            ..layout.image
        };
        if self.asset.metadata.has_alpha {
            if let Some(visible) = image.intersect(rect) {
                let tile = 18.0;
                let columns = (visible.width / tile).ceil() as i32;
                let rows = (visible.height / tile).ceil() as i32;
                painter.push_clip(visible);
                for y in 0..rows {
                    for x in 0..columns {
                        painter.fill_rect(
                            Rect {
                                x: visible.x + x as f32 * tile,
                                y: visible.y + y as f32 * tile,
                                width: tile,
                                height: tile,
                            },
                            self.checker[((x + y) & 1) as usize],
                            0.0,
                        );
                    }
                }
                painter.pop_clip();
            }
        }
        let effects = self.view.adjustments.effects(self.view.quarter_turns);
        painter.draw_image_effects(image, self.asset.data.image(), None, 1.0, effects);
        if let Some((map, visible)) = self.view.minimap(viewport, self.asset.size()) {
            let map = Rect {
                x: map.x + rect.x,
                y: map.y + rect.y,
                ..map
            };
            let visible = Rect {
                x: visible.x + rect.x,
                y: visible.y + rect.y,
                ..visible
            };
            painter.fill_rect(
                Rect {
                    x: map.x - 4.0,
                    y: map.y - 4.0,
                    width: map.width + 8.0,
                    height: map.height + 8.0,
                },
                self.background,
                6.0,
            );
            painter.draw_image_effects(map, self.asset.data.image(), None, 0.9, effects);
            painter.stroke_rect(map, self.checker[1], 1.0, 0.0);
            painter.stroke_rect(visible, self.accent, 2.0, 0.0);
        }
        painter.pop_clip();
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let state = self.state.clone();
        Some(Rc::new(move |point, _| state.drag_start(point)))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let state = self.state.clone();
        Some(Rc::new(move |point, _| state.drag_to(point)))
    }
    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        let state = self.state.clone();
        Some(Rc::new(move || state.drag.set(None)))
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        let state = self.state.clone();
        Some(Rc::new(move |delta| state.wheel(delta)))
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}
