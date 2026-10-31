mod panel;
mod view;

use coconut_plugin_kit::{
    chrome::{island_style, shell_text},
    pixel_icon, Island, IslandRenderContext, Panel, Plugin, PluginInitContext,
};
use creamui_core::{BoxedWidget, Painter, Point, Rect, Style, Widget};
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    RawButton,
};
use std::{cell::Cell, rc::Rc};

pub struct ClipboardPlugin;
pub struct ClipboardIsland;

impl Plugin for ClipboardPlugin {
    fn id(&self) -> &'static str {
        "clipboard"
    }
    fn islands(&self, _: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(ClipboardIsland)]
    }
    fn panels(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        vec![Rc::new(panel::ClipboardPanel {
            app: ctx.app.clone(),
        })]
    }
}

impl Island for ClipboardIsland {
    fn id(&self) -> &'static str {
        "clipboard"
    }
    fn build(&self, ctx: &IslandRenderContext<'_>) -> BoxedWidget {
        let size = 32.0 * ctx.scale;
        let open = ctx.open_panel.clone();
        let keyboard_open = open.clone();
        let anchor = Rc::new(Cell::new(Point { x: 0.0, y: 0.0 }));
        let keyboard_anchor = anchor.clone();
        Box::new(
            RawButton::new(island_style(size, size, ctx.scale), move || {
                keyboard_open("clipboard", keyboard_anchor.get())
            })
            .with_click_position(move |point| open("clipboard", point))
            .child(Box::new(
                Flex::row()
                    .size(size, size)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .child(Box::new(AnchoredIcon {
                        icon: pixel_icon("clipboard", 17.0 * ctx.scale, shell_text()),
                        anchor,
                    })),
            )),
        )
    }
}

struct AnchoredIcon {
    icon: BoxedWidget,
    anchor: Rc<Cell<Point>>,
}

impl Widget for AnchoredIcon {
    fn style(&self) -> Style {
        self.icon.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.icon.paint(painter, rect);
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.anchor.set(Point {
            x: rect.x + rect.width / 2.0,
            y: rect.y + rect.height / 2.0,
        });
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_plugin_kit::{design, IslandConfig, SharedState};
    use creamui_core::{Key, KeyInput, Renderer, Size};
    use creamui_render::SceneRecorder;
    use creamui_theme::{Color, Theme, ThemeProvider};

    #[test]
    fn island_opens_with_keyboard_and_pointer_at_its_actual_anchor() {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(Theme::dark());
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let shared = SharedState::default();
            let config = IslandConfig::default();
            let opened = Rc::new(Cell::new(None));
            let result = opened.clone();
            let ctx = IslandRenderContext {
                shared: &shared,
                config: &config,
                position: Default::default(),
                scale: 1.0,
                open_panel: Rc::new(move |id, point| {
                    assert_eq!(id, "clipboard");
                    result.set(Some(point));
                }),
            };
            let size = Size {
                width: 100.0,
                height: 80.0,
            };
            let mut recorder = SceneRecorder::new();
            recorder.begin(100, 80, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = Renderer::new().render(
                Box::new(
                    Flex::row()
                        .size(size.width, size.height)
                        .padding(20.0)
                        .child(ClipboardIsland.build(&ctx)),
                ),
                size,
                &mut recorder,
            );
            scene.on_key_at(0).unwrap()(KeyInput {
                key: Key::Enter,
                modifiers: Default::default(),
            });
            let point = opened.get().expect("keyboard opens the popup");
            assert!(point.x > 20.0 && point.y > 20.0);
            opened.set(None);
            scene.hit_test_at(point).unwrap()(point);
            assert_eq!(opened.get(), Some(point));
        });
    }
}
