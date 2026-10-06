use crate::state::WeatherState;
use coconut_plugin_kit::chrome::{island_style, shell_text};
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Style, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Justify};
use creamui_widgets::{RawButton, RawMarquee};

const SIDE_ITEM_SIZE: f32 = 36.0;

/// The dock's weather island: an icon/temperature/condition chip on a
/// horizontal dock, a fixed weather icon button on a vertical one. Opens the
/// `"weather"` panel on click either way.
///
/// Moved from `apps/shell/src/bar/mod.rs`'s weather-button construction in
/// `build_dock`/`build_side_dock`.
///
/// **Contract for `apps/shell/src/lib.rs` (Phase 4d):** insert a
/// `creamui_reactive::Signal<`[`WeatherState`]`>` into the app-wide
/// `SharedState`, seeded with [`WeatherState::Loading`] and updated by
/// running [`crate::refresh`] on a background thread (see its own doc
/// comment for when to call it). If nothing is ever inserted, this island
/// renders as though the forecast were still loading.
pub struct WeatherIsland;

impl Island for WeatherIsland {
    fn id(&self) -> &'static str {
        "weather"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let s = ctx.scale;
        let open = ctx.open_panel.clone();
        if ctx.position.is_vertical() {
            return Box::new(
                RawButton::new(side_button_style(s), || {})
                    .with_click_position(move |point| open("weather", point))
                    .child(Box::new(jsx! {
                        <Flex size={(SIDE_ITEM_SIZE * s, SIDE_ITEM_SIZE * s)} align={Align::Center} justify={Justify::Center}>
                            {pixel_icon("weather_cloud_sun", 19.0 * s, shell_text())}
                        </Flex>
                    })),
            );
        }

        let weather = ctx
            .shared
            .get::<Signal<WeatherState>>()
            .map(|signal| signal.get())
            .unwrap_or(WeatherState::Loading);
        let icon = weather_icon(&weather);
        let temperature = weather.bar_temperature();
        let condition: BoxedWidget = Box::new(RawMarquee::new(
            weather.bar_condition(),
            shell_text(),
            11.0 * s,
            58.0 * s,
        ));

        Box::new(
            RawButton::new(island_button_style(s), || {})
                .with_click_position(move |point| open("weather", point))
                .child(Box::new(jsx! {
                    <Flex direction={FlexDirection::Row} size={(120.0 * s, 32.0 * s)} padding={6.0 * s} gap={4.0 * s} align={Align::Center}>
                        {pixel_icon(icon, 14.0 * s, shell_text())}
                        <RawText color={shell_text()} font_size={11.0 * s} width={32.0 * s} align={TextAlign::Start}>{temperature}</RawText>
                        {condition}
                    </Flex>
                })),
        )
    }
}

fn weather_icon(weather: &WeatherState) -> &'static str {
    match weather {
        WeatherState::Ready(weather) => crate::state::icon_for(&weather.condition),
        WeatherState::Loading => "weather_cloudly",
        WeatherState::Unavailable(_) => "weather_cloudly",
    }
}

fn island_button_style(scale: f32) -> Style {
    island_style(120.0 * scale, 32.0 * scale, scale)
}

fn side_button_style(scale: f32) -> Style {
    island_style(SIDE_ITEM_SIZE * scale, SIDE_ITEM_SIZE * scale, scale).corner_radius(12.0 * scale)
}
