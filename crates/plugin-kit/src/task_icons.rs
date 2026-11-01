use crate::chrome::{shell_control_hover, shell_island, shell_selected, shell_text};
use crate::{chrome::shell_accent, AppEntry};
use crate::{ConfigValue, IslandConfig};
use creamui_core::layout::Position;
use creamui_core::{BoxedWidget, StateStyle, Style, Styled};
use creamui_image::{Image, ImageFit};
use creamui_theme::Color;
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::{Icon, Symbol};

#[derive(Clone, Copy)]
pub struct TaskIconStyle {
    pub scale: f32,
    pub compact: bool,
    pub background: bool,
}

impl TaskIconStyle {
    pub fn new(config: &IslandConfig, scale: f32, compact: bool) -> Self {
        Self {
            scale: scale
                * (config
                    .get("icon_scale")
                    .and_then(ConfigValue::as_f64)
                    .unwrap_or(100.0) as f32
                    / 100.0)
                    .clamp(0.5, 2.0),
            compact: config
                .get("compact")
                .and_then(ConfigValue::as_bool)
                .unwrap_or(compact),
            background: config
                .get("show_background")
                .and_then(ConfigValue::as_bool)
                .unwrap_or(false),
        }
    }

    pub fn width(self) -> f32 {
        (if self.compact { 46.0 } else { 32.0 }) * self.scale
    }

    pub fn height(self) -> f32 {
        (if self.compact { 32.0 } else { 36.0 }) * self.scale
    }

    pub fn icon_size(self) -> f32 {
        (if self.compact { 24.0 } else { 28.0 }) * self.scale
    }

    pub fn style(self) -> Style {
        Style::new()
            .width(self.width())
            .height(self.height())
            .flex_shrink(0.0)
            .background(if self.background {
                shell_island()
            } else {
                Color::rgba(0, 0, 0, 0)
            })
            .corner_radius(8.0 * self.scale)
            .hover(StateStyle::new().background(shell_control_hover()))
            .pressed(StateStyle::new().background(shell_selected()))
    }

    pub fn content(self, icon: BoxedWidget, active: bool) -> BoxedWidget {
        let mut content = Flex::column()
            .size(self.width(), self.height())
            .align(Align::Center)
            .justify(Justify::Center)
            .child(icon);
        if !self.compact {
            content = content.child(Box::new(
                Flex::row()
                    .size(2.5 * self.scale, 2.5 * self.scale)
                    .position(Position::Absolute)
                    .bottom(0.5 * self.scale)
                    .background(if active {
                        shell_text()
                    } else {
                        Color::rgba(0, 0, 0, 0)
                    })
                    .corner_radius(1.25 * self.scale),
            ));
        }
        Box::new(content)
    }
}

pub fn application_icon(app: Option<&AppEntry>, size: f32) -> BoxedWidget {
    let icon: BoxedWidget = app
        .and_then(|app| app.icon.clone())
        .map(|data| {
            Box::new(
                Image::new(data)
                    .layout(creamui_core::layout::Style {
                        size: creamui_widgets::layout::fixed(size, size),
                        ..Default::default()
                    })
                    .fit(ImageFit::Contain),
            ) as BoxedWidget
        })
        .unwrap_or_else(|| Box::new(Icon::new(Symbol::Grid, shell_accent()).size(size)));
    Box::new(
        Flex::row()
            .size(size, size)
            .align(Align::Center)
            .justify(Justify::Center)
            .child(icon),
    )
}
