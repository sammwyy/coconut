//! Native, illustrative previews use the explorer's actual icon pack.

use crate::{
    config::{ExplorerConfig, TabPosition},
    icons::Icons,
};
use creamui_core::{
    layout::{Dimension, Style},
    Painter, Rect, TextAlign, Widget,
};
use creamui_theme::{use_theme, Color, ColorScheme};
use creamui_widgets::Icon;

pub(super) struct ExplorerPreview {
    icons: Icons,
    config: ExplorerConfig,
    height: f32,
    window: bool,
}

impl ExplorerPreview {
    pub(super) fn new(icons: &Icons, config: ExplorerConfig, height: f32) -> Self {
        Self {
            icons: icons.clone(),
            config,
            height,
            window: true,
        }
    }

    pub(super) fn view(icons: &Icons, config: ExplorerConfig) -> Self {
        Self {
            icons: icons.clone(),
            config,
            height: 124.0,
            window: false,
        }
    }

    fn entries(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        let mut entries = vec![
            ("folder-large", "Projects", "Folder"),
            ("folder-large", "Photos", "Folder"),
            ("file", "Notes.txt", "Text file"),
        ];
        if self.config.show_hidden {
            entries.push(("folder-large", ".config", "Folder"));
        }
        entries
    }

    fn contents(&self, c: &mut Canvas<'_>, x: f32, y: f32, width: f32, height: f32) {
        let entries = self.entries();
        if self.config.default_grid_view {
            let columns = if height < 72.0 && width >= 240.0 {
                entries.len()
            } else if width < 210.0 || entries.len() == 4 {
                2
            } else {
                3
            };
            let rows = entries.len().div_ceil(columns);
            let cell_width = width / columns as f32;
            let cell_height = height / rows as f32;
            for (index, (icon, name, _)) in entries.iter().enumerate() {
                let left = x + (index % columns) as f32 * cell_width;
                let top = y + (index / columns) as f32 * cell_height;
                let size = (cell_height - 22.0)
                    .clamp(18.0, 38.0)
                    .min(cell_width - 16.0);
                if index == 0 {
                    c.fill(
                        left + 3.0,
                        top,
                        cell_width - 6.0,
                        cell_height - 2.0,
                        c.colors.selection_background,
                        6.0,
                    );
                }
                c.icon(
                    &self.icons,
                    icon,
                    left + (cell_width - size) / 2.0,
                    top + 2.0,
                    size,
                    c.colors.accent,
                );
                c.label(
                    left + 3.0,
                    top + size + 4.0,
                    cell_width - 6.0,
                    name,
                    10.0,
                    c.colors.text_primary,
                    TextAlign::Center,
                );
            }
        } else {
            let row_height = (height / entries.len() as f32).min(25.0);
            let icon_size = (row_height - 6.0).clamp(12.0, 18.0);
            for (index, (icon, name, kind)) in entries.iter().enumerate() {
                let top = y + index as f32 * row_height;
                if index == 0 {
                    c.fill(
                        x,
                        top,
                        width,
                        row_height - 1.0,
                        c.colors.selection_background,
                        5.0,
                    );
                }
                c.icon(
                    &self.icons,
                    icon,
                    x + 8.0,
                    top + (row_height - icon_size) / 2.0,
                    icon_size,
                    c.colors.accent,
                );
                let kind_width = if width > 240.0 { 64.0 } else { 0.0 };
                c.label(
                    x + 34.0,
                    top + (row_height - 15.0) / 2.0,
                    width - 42.0 - kind_width,
                    name,
                    11.0,
                    c.colors.text_primary,
                    TextAlign::Start,
                );
                if kind_width > 0.0 {
                    c.label(
                        x + width - kind_width,
                        top + (row_height - 15.0) / 2.0,
                        kind_width - 6.0,
                        kind,
                        10.0,
                        c.colors.text_secondary,
                        TextAlign::Start,
                    );
                }
            }
        }
    }

    fn paint_view(&self, c: &mut Canvas<'_>) {
        let width = c.rect.width;
        let height = c.rect.height;
        c.fill(0.0, 0.0, width, height, c.colors.surface, 9.0);
        c.fill(
            0.0,
            0.0,
            width,
            29.0,
            c.colors.surface.mix(c.colors.accent, 0.035),
            0.0,
        );
        c.icon(
            &self.icons,
            "back",
            10.0,
            8.0,
            13.0,
            c.colors.text_secondary,
        );
        c.icon(&self.icons, "folder", 32.0, 7.0, 15.0, c.colors.accent);
        c.label(
            54.0,
            6.0,
            width - 86.0,
            "Documents",
            11.0,
            c.colors.text_primary,
            TextAlign::Start,
        );
        c.icon(
            &self.icons,
            if self.config.default_grid_view {
                "grid"
            } else {
                "list"
            },
            width - 24.0,
            8.0,
            13.0,
            c.colors.text_secondary,
        );
        c.fill(0.0, 29.0, width, 1.0, c.colors.border, 0.0);
        self.contents(c, 7.0, 37.0, width - 14.0, height - 43.0);
    }

    fn paint_window(&self, c: &mut Canvas<'_>) {
        let outer = c.rect;
        c.painter.fill_linear_gradient(
            outer,
            c.colors.surface.mix(c.colors.accent, 0.04),
            c.colors.surface.mix(c.colors.accent, 0.09),
            145.0,
            9.0,
        );
        let width = (outer.width - 24.0).clamp(1.0, 440.0);
        let height = (outer.height - 16.0).max(1.0);
        let frame = Rect {
            x: outer.x + (outer.width - width) / 2.0,
            y: outer.y + 8.0,
            width,
            height,
        };
        for (spread, alpha) in [(5.0, 4), (3.0, 6), (1.0, 10)] {
            c.painter.fill_rect(
                Rect {
                    x: frame.x - spread,
                    y: frame.y + 2.0 - spread,
                    width: frame.width + spread * 2.0,
                    height: frame.height + spread * 2.0,
                },
                Color::rgba(15, 25, 40, alpha),
                10.0 + spread,
            );
        }
        c.rect = frame;
        c.painter.push_clip_rounded(frame, 9.0);
        c.fill(0.0, 0.0, width, height, c.colors.surface, 9.0);
        let sidebar = if width >= 380.0 { 100.0 } else { 82.0 };
        c.fill(
            0.0,
            0.0,
            sidebar,
            height,
            c.colors.surface.mix(c.colors.accent, 0.045),
            0.0,
        );
        c.label(
            10.0,
            7.0,
            sidebar - 16.0,
            "PLACES",
            8.0,
            c.colors.text_secondary,
            TextAlign::Start,
        );
        for (index, (icon, label)) in [
            ("home", "Home"),
            ("folder", "Documents"),
            ("download", "Downloads"),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 29.0 + index as f32 * 23.0;
            if index == 1 {
                c.fill(
                    5.0,
                    y - 3.0,
                    sidebar - 10.0,
                    22.0,
                    c.colors.selection_background,
                    5.0,
                );
            }
            c.icon(&self.icons, icon, 10.0, y, 14.0, c.colors.accent);
            c.label(
                30.0,
                y,
                sidebar - 36.0,
                label,
                9.0,
                c.colors.text_secondary,
                TextAlign::Start,
            );
        }
        c.fill(sidebar, 0.0, 1.0, height, c.colors.border, 0.0);
        let main_width = width - sidebar;
        let top_tabs = self.config.tab_position == TabPosition::Top;
        let status_height = if self.config.show_statusbar {
            17.0
        } else {
            0.0
        };
        let toolbar_y = if top_tabs { 23.0 } else { 0.0 };
        let tabs_y = if top_tabs { 0.0 } else { height - 23.0 };
        // Tabs precede the address bar, just as they do in the real explorer.
        c.fill(
            sidebar,
            tabs_y,
            main_width,
            23.0,
            c.colors.surface.mix(c.colors.accent, 0.025),
            0.0,
        );
        for (index, label) in ["Documents", "Downloads"].into_iter().enumerate() {
            let tab_width = ((main_width - 22.0) / 2.0).min(106.0);
            let x = sidebar + 5.0 + index as f32 * (tab_width + 3.0);
            c.fill(
                x,
                tabs_y + 3.0,
                tab_width,
                19.0,
                if index == 0 {
                    c.colors.surface_elevated
                } else {
                    c.colors.surface
                },
                5.0,
            );
            c.label(
                x + 8.0,
                tabs_y + 5.0,
                tab_width - 16.0,
                label,
                9.0,
                if index == 0 {
                    c.colors.text_primary
                } else {
                    c.colors.text_secondary
                },
                TextAlign::Start,
            );
        }
        c.fill(
            sidebar,
            toolbar_y,
            main_width,
            28.0,
            c.colors.surface_elevated,
            0.0,
        );
        c.icon(
            &self.icons,
            "back",
            sidebar + 9.0,
            toolbar_y + 7.0,
            13.0,
            c.colors.text_secondary,
        );
        c.icon(
            &self.icons,
            "forward",
            sidebar + 29.0,
            toolbar_y + 7.0,
            13.0,
            c.colors.text_secondary,
        );
        c.fill(
            sidebar + 51.0,
            toolbar_y + 5.0,
            main_width - 80.0,
            18.0,
            c.colors.surface,
            5.0,
        );
        c.label(
            sidebar + 59.0,
            toolbar_y + 7.0,
            main_width - 96.0,
            "Home / Documents",
            9.0,
            c.colors.text_secondary,
            TextAlign::Start,
        );
        c.icon(
            &self.icons,
            if self.config.default_grid_view {
                "grid"
            } else {
                "list"
            },
            width - 22.0,
            toolbar_y + 7.0,
            13.0,
            c.colors.text_secondary,
        );

        let content_y = toolbar_y + 34.0;
        let content_bottom = if top_tabs { height } else { tabs_y } - status_height;
        self.contents(
            c,
            sidebar + 7.0,
            content_y,
            main_width - 14.0,
            (content_bottom - content_y - 4.0).max(1.0),
        );
        if self.config.show_statusbar {
            let y = content_bottom;
            c.fill(
                sidebar,
                y,
                main_width,
                status_height,
                c.colors.surface_elevated,
                0.0,
            );
            c.label(
                sidebar + 10.0,
                y + 1.0,
                main_width - 20.0,
                if self.config.show_hidden {
                    "4 items"
                } else {
                    "3 items"
                },
                8.0,
                c.colors.text_secondary,
                TextAlign::Start,
            );
        }
        c.painter.pop_clip();
        c.painter
            .stroke_rect(frame, c.colors.border_strong, 1.0, 9.0);
        c.rect = outer;
    }
}

impl Widget for ExplorerPreview {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if rect.width < 80.0 || rect.height < 60.0 {
            return;
        }
        painter.push_clip_rounded(rect, 9.0);
        let mut c = Canvas {
            painter,
            rect,
            colors: use_theme().colors,
        };
        if self.window {
            self.paint_window(&mut c);
        } else {
            self.paint_view(&mut c);
        }
        c.painter.pop_clip();
        c.painter.stroke_rect(rect, c.colors.border, 1.0, 9.0);
    }
}

/// Local coordinates keep spacing consistent in every preview size.
struct Canvas<'a> {
    painter: &'a mut dyn Painter,
    rect: Rect,
    colors: ColorScheme,
}

impl Canvas<'_> {
    fn fill(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color, radius: f32) {
        if width > 0.0 && height > 0.0 {
            self.painter.fill_rect(
                Rect {
                    x: self.rect.x + x,
                    y: self.rect.y + y,
                    width,
                    height,
                },
                color,
                radius,
            );
        }
    }

    fn icon(&mut self, icons: &Icons, name: &str, x: f32, y: f32, size: f32, color: Color) {
        if size > 0.0 {
            Icon::draw(
                icons.get(name),
                self.painter,
                Rect {
                    x: self.rect.x + x,
                    y: self.rect.y + y,
                    width: size,
                    height: size,
                },
                color,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn label(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        text: &str,
        size: f32,
        color: Color,
        align: TextAlign,
    ) {
        if width < size {
            return;
        }
        let text = crate::components::short(text, (width / (size * 0.6)).floor() as usize);
        let rect = Rect {
            x: self.rect.x + x,
            y: self.rect.y + y,
            width,
            height: size + 5.0,
        };
        self.painter.push_clip(rect);
        self.painter.fill_text(rect, &text, color, size, align);
        self.painter.pop_clip();
    }
}
