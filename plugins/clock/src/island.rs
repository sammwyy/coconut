use crate::shared::ClockText;
use coconut_plugin_kit::chrome::{island_style, shell_text};
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Painter, Rect, Style, TextAlign, Widget};
use creamui_macros::jsx;
use creamui_widgets::layout::{fixed, Align, Justify};
use creamui_widgets::{Icon, RawButton, Symbol};

const CLOCK_WIDTH: f32 = 60.0;
const SIDE_ITEM_SIZE: f32 = 36.0;

/// The dock's clock island: a compact digital-time chip on a horizontal
/// dock, a sun-symbol icon button on a vertical one. Opens the `"clock"`
/// panel on click either way.
///
/// Moved from `apps/shell/src/bar/mod.rs`'s `LiveClock` widget plus the
/// clock-button construction in `build_dock`/`build_side_dock`.
pub struct ClockIsland {
    pub format: String,
}

impl Island for ClockIsland {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let s = ctx.scale;
        let open = ctx.open_panel.clone();
        if ctx.position.is_vertical() {
            return Box::new(
                RawButton::new(side_button_style(s), || {})
                    .with_click_position(move |point| open("clock", point))
                    .child(Box::new(jsx! {
                        <Flex size={(SIDE_ITEM_SIZE * s, SIDE_ITEM_SIZE * s)} align={Align::Center} justify={Justify::Center}>
                            {Box::new(Icon::new(Symbol::Sun, shell_text()).size(19.0 * s)) as BoxedWidget}
                        </Flex>
                    })),
            );
        }

        let text = self.current_text(ctx);
        let clock: BoxedWidget = Box::new(LiveClock { text, scale: s });
        Box::new(
            RawButton::new(island_button_style(s), || {})
                .with_click_position(move |point| open("clock", point))
                .child(Box::new(jsx! {
                    <Flex direction={FlexDirection::Row} size={(96.0 * s, 32.0 * s)} padding={10.0 * s} gap={4.0 * s} justify={Justify::Center} align={Align::Center}>
                        {pixel_icon("clock-face", 12.0 * s, shell_text())}
                        {clock}
                    </Flex>
                })),
        )
    }
}

impl ClockIsland {
    /// Reads the live-updating text from [`ClockText`] in shared state, if
    /// `apps/shell/src/lib.rs` (Phase 4d) has wired it up; otherwise falls
    /// back to a one-shot `Local::now()` formatted with this island's own
    /// `format` (correct at build time, but it won't tick without the
    /// shared signal — see [`ClockText`]'s doc comment).
    fn current_text(&self, ctx: &IslandRenderContext) -> String {
        match ctx.shared.get::<ClockText>() {
            Some(ClockText(signal)) => signal.get(),
            None => chrono::Local::now().format(&self.format).to_string(),
        }
    }
}

struct LiveClock {
    text: String,
    scale: f32,
}

impl Widget for LiveClock {
    fn style(&self) -> Style {
        Style::new().layout(LayoutStyle {
            size: fixed(CLOCK_WIDTH * self.scale, 24.0 * self.scale),
            ..Default::default()
        })
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_text_font(
            rect,
            &self.text,
            shell_text(),
            11.0 * self.scale,
            TextAlign::Center,
            None,
            true,
            false,
        );
    }
}

fn island_button_style(scale: f32) -> Style {
    island_style(96.0 * scale, 32.0 * scale, scale)
}

fn side_button_style(scale: f32) -> Style {
    island_style(SIDE_ITEM_SIZE * scale, SIDE_ITEM_SIZE * scale, scale).corner_radius(12.0 * scale)
}
