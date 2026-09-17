use coconut_plugin_kit::chrome::{shell_border, shell_muted, shell_panel, shell_text};
use coconut_plugin_kit::{Island, IslandRenderContext};
use creamui_core::layout::FlexDirection;
use creamui_core::BoxedWidget;
use creamui_macros::jsx;
use creamui_widgets::layout::Align;

/// The dock's own logo/brand mark. Purely decorative — never opens a panel,
/// never reads shared state or config. Moved verbatim from
/// `apps/shell/src/bar/mod.rs::logo_widget` (Phase 4a of the dock/island
/// refactor).
pub struct LogoIsland;

impl Island for LogoIsland {
    fn id(&self) -> &'static str {
        "logo"
    }

    fn build(&self, _ctx: &IslandRenderContext) -> BoxedWidget {
        Box::new(jsx! {
            <Flex direction={FlexDirection::Row} size={(48.0, 32.0)} padding={5.0} gap={2.0} align={Align::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={9.0}>
                <Flex size={(5.0, 5.0)} background={shell_text()} corner_radius={2.5} />
                <Flex size={(3.0, 3.0)} background={shell_muted()} corner_radius={1.5} />
                <Flex size={(3.0, 3.0)} background={shell_muted()} corner_radius={1.5} />
                <Flex size={(3.0, 3.0)} background={shell_muted()} corner_radius={1.5} />
                <Flex size={(3.0, 3.0)} background={shell_muted()} corner_radius={1.5} />
            </Flex>
        })
    }
}
