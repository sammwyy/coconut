use crate::state::{condition_label, format_temperature, icon_for, HourlyForecast, WeatherState};
use coconut_plugin_kit::chrome::{shell_border, shell_muted, shell_panel, shell_text};
use coconut_plugin_kit::{pixel_icon, Panel, PanelRenderContext};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Justify};

const WIDTH: u32 = 280;
const HEIGHT: u32 = 240;

/// The expanded weather panel opened from [`crate::WeatherIsland`]. Ported
/// from `apps/shell/src/panels/weather/mod.rs`, reading the same
/// `Signal<WeatherState>` from shared state the island reads (see
/// [`crate::WeatherIsland`]'s doc comment for the `SharedState` contract).
pub struct WeatherPanel;

impl Panel for WeatherPanel {
    fn id(&self) -> &'static str {
        "weather"
    }

    fn title(&self) -> &'static str {
        "Coconut Weather"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        let state = ctx
            .shared
            .get::<Signal<WeatherState>>()
            .map(|signal| signal.get())
            .unwrap_or(WeatherState::Loading);
        let (temperature, condition, city, location_codes, high, low, icon, hourly) = match state
        {
            WeatherState::Ready(weather) => (
                format_temperature(weather.temperature_c),
                condition_label(&weather.condition).to_owned(),
                weather.city,
                weather.location_codes,
                format_temperature(weather.high_c),
                format_temperature(weather.low_c),
                icon_for(&weather.condition),
                weather.hourly,
            ),
            WeatherState::Loading => (
                "…".into(),
                "Loading weather".into(),
                "Loading your location".into(),
                "".into(),
                "--".into(),
                "--".into(),
                "weather_cloudly",
                Vec::new(),
            ),
            WeatherState::Unavailable(message) => (
                "--".into(),
                "Weather unavailable".into(),
                message,
                "".into(),
                "--".into(),
                "--".into(),
                "weather_cloudly",
                Vec::new(),
            ),
        };
        let mut strip = creamui_widgets::layout::Flex::row()
            .gap(8.0)
            .justify(Justify::Between);
        for hour in &hourly {
            strip = strip.child(hour_cell(hour));
        }

        Box::new(jsx! {
            <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={18.0} gap={14.0} background={shell_panel()} border={(shell_border(), 1.0)}>
                <Flex direction={FlexDirection::Row} gap={10.0} align={Align::Center}>
                    {pixel_icon(icon, 52.0, shell_text())}
                    <Flex direction={FlexDirection::Column} gap={3.0} shrink={0.0}>
                        <RawText color={shell_text()} font_size={52.0}>{temperature}</RawText>
                    </Flex>
                    <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                        <RawText color={shell_muted()} font_size={16.0}>{condition}</RawText>
                        <RawText color={shell_text()} font_size={13.0}>{city}</RawText>
                        <RawText color={shell_muted()} font_size={11.0}>{location_codes}</RawText>
                    </Flex>
                </Flex>
                <Flex direction={FlexDirection::Row} gap={10.0}>
                    {stat_chip("H", high)}
                    {stat_chip("L", low)}
                </Flex>
                <Flex direction={FlexDirection::Row} padding={8.0} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={12.0}>
                    {Box::new(strip) as BoxedWidget}
                </Flex>
            </Flex>
        })
    }
}

fn stat_chip(label: &str, value: String) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} grow={1.0} padding={10.0} gap={10.0} align={Align::Center} justify={Justify::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={10.0}>
            <RawText color={shell_muted()} font_size={12.0}>{label}</RawText>
            <RawText color={shell_text()} font_size={16.0}>{value}</RawText>
        </Flex>
    })
}

fn hour_cell(hour: &HourlyForecast) -> BoxedWidget {
    let label = chrono::DateTime::parse_from_rfc3339(&hour.time)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%H")
                .to_string()
        })
        .unwrap_or_else(|_| "--".into());
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(44.0, 64.0)} gap={5.0} align={Align::Center} justify={Justify::Center}>
            <RawText color={shell_muted()} font_size={10.0}>{label}</RawText>
            {pixel_icon(icon_for(&hour.condition), 18.0, shell_text())}
            <RawText color={shell_text()} font_size={13.0}>{format_temperature(hour.temperature_c)}</RawText>
        </Flex>
    })
}
