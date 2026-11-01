use creamui_core::{Painter, Rect, Style, TextAlign, Widget};
use creamui_widgets::ProgressRing;

pub(super) struct ChargeMeter {
    ring: ProgressRing,
    label: String,
    color: creamui_theme::Color,
}
impl ChargeMeter {
    pub fn new(percentage: u8, size: f32) -> Self {
        Self {
            ring: ProgressRing::new(f32::from(percentage) / 100.0).size(size),
            label: format!("{percentage}%"),
            color: coconut_plugin_kit::chrome::shell_text(),
        }
    }
}
impl Widget for ChargeMeter {
    fn style(&self) -> Style {
        self.ring.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.ring.paint(painter, rect);
        painter.fill_text(rect, &self.label, self.color, 12.0, TextAlign::Center);
    }
}
