//! Shared placement and surface for command-activated control modes.
//! Mode content and keyboard handling belong to the application, not the surface.
use crate::ui::{fixed, pad, positioned};
use creamui_core::{BoxedWidget, Rect, Size, Styled};
use creamui_theme::use_theme;
use creamui_widgets::{RawScrollView, RawView, ScrollController};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Search,
    PasteConfirm,
    Palette,
    RenameTab,
    SaveOutput,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Horizontal {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vertical {
    Top,
    Center,
    #[default]
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub orientation: Orientation,
    pub horizontal: Horizontal,
    pub vertical: Vertical,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            orientation: Orientation::Horizontal,
            horizontal: Horizontal::Center,
            vertical: Vertical::Bottom,
        }
    }
}
impl Config {
    /// Fit within the terminal's usable area, excluding chrome and status.
    pub fn bounds(&self, area: Rect, preferred: Size) -> Rect {
        let margin_x = 8.0_f32.min(area.width.max(0.0) / 2.0);
        let margin_y = 8.0_f32.min(area.height.max(0.0) / 2.0);
        let available_width = (area.width - 2.0 * margin_x).max(0.0);
        let available_height = (area.height - 2.0 * margin_y).max(0.0);
        let width = preferred.width.min(available_width);
        let height = preferred.height.min(available_height);
        let x = match self.horizontal {
            Horizontal::Left => 0.0,
            Horizontal::Center => (available_width - width) / 2.0,
            Horizontal::Right => available_width - width,
        };
        let y = match self.vertical {
            Vertical::Top => 0.0,
            Vertical::Center => (available_height - height) / 2.0,
            Vertical::Bottom => available_height - height,
        };
        Rect {
            x: area.x + margin_x + x,
            y: area.y + margin_y + y,
            width,
            height,
        }
    }
}

/// Any mode can supply its own content and preferred dimensions. The host
/// owns positioning, decoration, clipping and scrolling for small windows.
pub fn surface(
    config: &Config,
    area: Rect,
    preferred: Size,
    content: BoxedWidget,
    scroll: ScrollController,
) -> BoxedWidget {
    let bounds = config.bounds(area, preferred);
    let theme = use_theme();
    Box::new(
        RawView::new(positioned(
            pad(fixed(bounds.width, bounds.height), 8.0, 8.0),
            bounds.x,
            bounds.y,
        ))
        .background(theme.colors.surface_elevated)
        .corner_radius(10.0)
        .outline(theme.colors.border, 1.0)
        .child(Box::new(
            RawScrollView::controlled(
                creamui_core::layout::Style {
                    align_items: Some(creamui_core::layout::AlignItems::Stretch),
                    ..fixed(
                        (bounds.width - 16.0).max(0.0),
                        (bounds.height - 16.0).max(0.0),
                    )
                },
                scroll,
            )
            .scrollbar_width(3.0)
            .scrollbar(preferred.height > bounds.height)
            .child(content),
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_fits_all_anchors_and_tiny_terminal_areas() {
        for area in [
            Rect {
                x: 0.0,
                y: 44.0,
                width: 1000.0,
                height: 600.0,
            },
            Rect {
                x: 5.0,
                y: 44.0,
                width: 180.0,
                height: 120.0,
            },
            Rect {
                x: 0.0,
                y: 44.0,
                width: 1.0,
                height: 1.0,
            },
        ] {
            for horizontal in [Horizontal::Left, Horizontal::Center, Horizontal::Right] {
                for vertical in [Vertical::Top, Vertical::Center, Vertical::Bottom] {
                    let config = Config {
                        horizontal,
                        vertical,
                        ..Default::default()
                    };
                    let bounds = config.bounds(
                        area,
                        Size {
                            width: 560.0,
                            height: 244.0,
                        },
                    );
                    assert!(bounds.x >= area.x && bounds.y >= area.y);
                    assert!(bounds.x + bounds.width <= area.x + area.width);
                    assert!(bounds.y + bounds.height <= area.y + area.height);
                    if horizontal == Horizontal::Center {
                        assert_eq!(bounds.x + bounds.width / 2.0, area.x + area.width / 2.0);
                    }
                    if vertical == Vertical::Center {
                        assert_eq!(bounds.y + bounds.height / 2.0, area.y + area.height / 2.0);
                    }
                }
            }
        }
        let bounds = Config::default().bounds(
            Rect {
                x: 0.0,
                y: 44.0,
                width: 1000.0,
                height: 600.0,
            },
            Size {
                width: 560.0,
                height: 50.0,
            },
        );
        assert_eq!(
            bounds,
            Rect {
                x: 220.0,
                y: 586.0,
                width: 560.0,
                height: 50.0
            }
        );
    }
}
