use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::layout::{Align, Justify};

const CARD: Color = Color::rgba(31, 32, 34, 252);
const TEXT: Color = Color::rgb(244, 244, 245);
const MUTED: Color = Color::rgb(166, 168, 171);

pub const WIDTH: u32 = 240;
pub const HEIGHT: u32 = 112;

pub fn build(_: Size) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={8.0} background={CARD} corner_radius={14.0}>
            <Flex direction={FlexDirection::Row} gap={10.0} align={Align::Center}>
                {pixel_icon("weather-sun-cloud", 28.0)}
                <Flex direction={FlexDirection::Column} gap={1.0} grow={1.0}>
                    <RawText color={TEXT} font_size={16.0}>"24°  Clear"</RawText>
                    <RawText color={MUTED} font_size={10.0}>"Buenos Aires · Now"</RawText>
                </Flex>
            </Flex>
            <Flex direction={FlexDirection::Row} gap={10.0} justify={Justify::Between}>
                <RawText color={MUTED} font_size={10.0}>"High 27°"</RawText>
                <RawText color={MUTED} font_size={10.0}>"Low 17°"</RawText>
            </Flex>
        </Flex>
    })
}
use crate::icons::pixel_icon;
