use crate::shared::ClockText;
use coconut_plugin_kit::chrome::{
    shell_border, shell_control_hover, shell_panel, shell_selected, shell_text,
};
use coconut_plugin_kit::{Island, IslandRenderContext};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Painter, Rect, StateStyle, Style, TextAlign, Widget};
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
        let open = ctx.open_panel.clone();
        if ctx.position.is_vertical() {
            return Box::new(
                RawButton::new(side_button_style(), || {})
                    .with_click_position(move |point| open("clock", point))
                    .child(Box::new(jsx! {
                        <Flex size={(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE)} align={Align::Center} justify={Justify::Center}>
                            {Box::new(Icon::new(Symbol::Sun, shell_text()).size(19.0)) as BoxedWidget}
                        </Flex>
                    })),
            );
        }

        let text = self.current_text(ctx);
        let clock: BoxedWidget = Box::new(LiveClock { text });
        Box::new(
            RawButton::new(island_button_style(), || {})
                .with_click_position(move |point| open("clock", point))
                .child(Box::new(jsx! {
                    <Flex direction={FlexDirection::Column} size={(68.0, 32.0)} padding={2.0} justify={Justify::Center} align={Align::Center}>
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
}

impl Widget for LiveClock {
    fn style(&self) -> Style {
        Style::new().layout(LayoutStyle {
            size: fixed(CLOCK_WIDTH, 24.0),
            ..Default::default()
        })
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_text_font(
            rect,
            &self.text,
            shell_text(),
            13.0,
            TextAlign::Center,
            None,
            false,
            false,
        );
    }
}

fn island_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(68.0, 32.0),
            ..Default::default()
        })
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(9.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn side_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE),
            ..Default::default()
        })
        .background(shell_panel())
        .corner_radius(8.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}
