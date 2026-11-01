use coconut_plugin_kit::chrome::shell_muted;
use creamui_core::{CursorIcon, KeyInput, Painter, Point, Rect, Style, Widget};
use creamui_widgets::RawButton;
use std::rc::Rc;

pub(super) struct ResizeHandle {
    button: RawButton,
    color: creamui_theme::Color,
}

impl ResizeHandle {
    pub fn new(button: RawButton) -> Self {
        Self {
            button,
            color: shell_muted(),
        }
    }
}

impl Widget for ResizeHandle {
    fn style(&self) -> Style {
        self.button.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        for offset in [0.0, 5.0, 10.0] {
            painter.stroke_line(
                Point {
                    x: rect.x + 8.0 + offset,
                    y: rect.y + rect.height - 6.0,
                },
                Point {
                    x: rect.x + rect.width - 6.0,
                    y: rect.y + 8.0 + offset,
                },
                self.color,
                1.8,
            );
        }
    }
    fn focusable(&self) -> bool {
        self.button.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.button.on_key()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.button.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::ResizeNwse
        })
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.button.on_click()
    }
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.button.on_click_at()
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.button.on_drag()
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.button.on_drag_start()
    }
    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        self.button.on_drag_end()
    }
}
