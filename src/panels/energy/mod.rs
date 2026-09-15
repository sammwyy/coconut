use crate::integrations::battery::BatteryIntegration;
use crate::integrations::brightness::BrightnessIntegration;
use crate::panels::chrome::{
    fat_slider, hero_card, panel_header, toggle_row, BORDER, CARD, CARD_RADIUS,
};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 420;

const SLIDER_W: f32 = WIDTH as f32 - 32.0;

pub fn build(
    _: Size,
    battery: Rc<dyn BatteryIntegration>,
    brightness: Rc<dyn BrightnessIntegration>,
    brightness_level: Signal<f32>,
    awake: bool,
    toggle_awake: Rc<dyn Fn()>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let battery_percent = battery.percentage();
    let battery_icon = if battery.charging() {
        "battery-bolt"
    } else {
        match battery_percent.unwrap_or(0) {
            80.. => "battery-full",
            50..=79 => "battery-mid",
            20..=49 => "battery-low",
            _ => "battery-empty",
        }
    };
    let battery_value = battery_percent
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "--".into());
    let battery_caption = if battery_percent.is_none() {
        "Unavailable"
    } else if battery.charging() {
        "Charging"
    } else {
        "On battery"
    };

    let brightness_value = brightness_level.get();
    let brightness_text = format!("{}%", (brightness_value * 100.0).round());
    let set_brightness = brightness_level.clone();
    let brightness_backend = brightness.clone();

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("ENERGY", on_back)}
            {hero_card(battery_icon, battery_value, battery_caption.to_owned())}
            {fat_slider("brightness", "Brightness", brightness_value, brightness_text, SLIDER_W, move |level| {
                set_brightness.set(level);
                brightness_backend.set_level(level);
            })}
            {toggle_row("Keep awake", awake, toggle_awake)}
        </Flex>
    })
}
