use coconut_plugin_kit::chrome::{shell_border, shell_muted, shell_panel, shell_text};
use coconut_plugin_kit::{Island, IslandRenderContext};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::{layout::Align, RawText};

pub struct LogoIsland;

impl Island for LogoIsland {
    fn id(&self) -> &'static str {
        "logo"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let s = ctx.scale;
        Box::new(jsx! {
            <Flex direction={FlexDirection::Row} size={(122.0 * s, 32.0 * s)} padding={4.0 * s} gap={2.0 * s} align={Align::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={16.0 * s}>
                {workspace_chip("1", true, s)}
                {workspace_chip("2", false, s)}
                {workspace_chip("3", false, s)}
                {workspace_chip("4", false, s)}
            </Flex>
        })
    }
}

fn workspace_chip(label: &str, active: bool, scale: f32) -> BoxedWidget {
    let background = if active {
        shell_text()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    let color = if active { shell_panel() } else { shell_muted() };
    Box::new(
        creamui_widgets::layout::Flex::row()
            .size(26.0 * scale, 24.0 * scale)
            .align(Align::Center)
            .justify(creamui_widgets::layout::Justify::Center)
            .background(background)
            .corner_radius(12.0 * scale)
            .child(Box::new(
                RawText::new(label, color, 11.0 * scale).bold(true),
            )),
    )
}
