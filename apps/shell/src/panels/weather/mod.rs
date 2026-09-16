use crate::icons::pixel_icon;
use crate::panels::chrome::{BORDER, CARD, CARD_RADIUS, MUTED, PANEL, TEXT};
use crate::weather::{condition_label, format_temperature, icon_for, HourlyForecast, WeatherState};
use chrono::{DateTime, Local};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Justify};

pub const WIDTH: u32 = 280;
pub const HEIGHT: u32 = 240;

pub fn build(_: Size, state: Signal<WeatherState>) -> BoxedWidget {
    let state = state.get();
    let (temperature, condition, city, location_codes, high, low, icon, hourly) = match state {
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
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={18.0} gap={14.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            <Flex direction={FlexDirection::Row} gap={10.0} align={Align::Center}>
                {pixel_icon(icon, 52.0, TEXT)}
                <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                    <RawText color={TEXT} font_size={52.0}>{temperature}</RawText>
                </Flex>
                <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                    <RawText color={MUTED} font_size={16.0}>{condition}</RawText>
                    <RawText color={TEXT} font_size={13.0}>{city}</RawText>
                    <RawText color={MUTED} font_size={11.0}>{location_codes}</RawText>
                </Flex>
            </Flex>
            <Flex direction={FlexDirection::Row} gap={10.0}>
                {stat_chip("H", high)}
                {stat_chip("L", low)}
            </Flex>
            <Flex direction={FlexDirection::Row} padding={8.0} background={PANEL} border={(BORDER, 1.0)} corner_radius={12.0}>
                {Box::new(strip) as BoxedWidget}
            </Flex>
        </Flex>
    })
}

fn stat_chip(label: &str, value: String) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} grow={1.0} padding={10.0} gap={10.0} align={Align::Center} justify={Justify::Center} background={PANEL} border={(BORDER, 1.0)} corner_radius={10.0}>
            <RawText color={MUTED} font_size={12.0}>{label}</RawText>
            <RawText color={TEXT} font_size={16.0}>{value}</RawText>
        </Flex>
    })
}

fn hour_cell(hour: &HourlyForecast) -> BoxedWidget {
    let label = DateTime::parse_from_rfc3339(&hour.time)
        .map(|time| time.with_timezone(&Local).format("%H").to_string())
        .unwrap_or_else(|_| "--".into());
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(44.0, 64.0)} gap={5.0} align={Align::Center} justify={Justify::Center}>
            <RawText color={MUTED} font_size={10.0}>{label}</RawText>
            {pixel_icon(icon_for(&hour.condition), 18.0, TEXT)}
            <RawText color={TEXT} font_size={13.0}>{format_temperature(hour.temperature_c)}</RawText>
        </Flex>
    })
}
