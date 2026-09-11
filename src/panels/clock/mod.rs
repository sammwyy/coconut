use chrono::Local;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::layout::{Align, Justify};

pub const WIDTH: u32 = 260;
pub const HEIGHT: u32 = 156;

const CARD: Color = Color::rgba(27, 28, 30, 252);
const PANEL: Color = Color::rgba(39, 40, 42, 245);
const TEXT: Color = Color::rgb(244, 244, 245);
const MUTED: Color = Color::rgb(166, 168, 171);

pub fn build(_: Size) -> BoxedWidget {
    let now = Local::now();
    let time = now.format("%H:%M").to_string();
    let date = now.format("%A, %-d de %B").to_string();
    let year = now.format("%Y").to_string();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={18.0} gap={12.0} background={CARD} corner_radius={16.0}>
            <Flex direction={FlexDirection::Column} align={Align::Center} gap={3.0}>
                <RawText color={TEXT} font_size={38.0}>{time}</RawText>
                <RawText color={MUTED} font_size={12.0}>{date}</RawText>
            </Flex>
            <Flex direction={FlexDirection::Row} size={(224.0, 1.0)} background={Color::rgba(255, 255, 255, 26)} />
            <Flex direction={FlexDirection::Row} padding={10.0} background={PANEL} corner_radius={10.0} align={Align::Center} justify={Justify::Center}>
                <RawText color={MUTED} font_size={11.0}>{year}</RawText>
            </Flex>
        </Flex>
    })
}
