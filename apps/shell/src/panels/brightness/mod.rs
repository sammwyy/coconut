use crate::panels::chrome::{
    fat_slider, hero_card, panel_header, shell_border, shell_card, CARD_RADIUS,
};
use coconut_api::brightness::BrightnessIntegration;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

const SLIDER_W: f32 = WIDTH as f32 - 32.0;

pub fn build(
    _: Size,
    brightness: Rc<dyn BrightnessIntegration>,
    brightness_level: Signal<f32>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let value = brightness_level.get();
    let value_text = format!("{}%", (value * 100.0).round());
    let set_brightness = brightness_level.clone();

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("BRIGHTNESS", on_back)}
            {hero_card("brightness", value_text.clone(), "Display".to_owned())}
            {fat_slider("brightness", "Brightness", value, value_text, SLIDER_W, None, move |level| {
                set_brightness.set(level);
                brightness.set_level(level);
            })}
        </Flex>
    })
}
