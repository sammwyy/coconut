use crate::integrations::battery::BatteryIntegration;
use crate::panels::chrome::{hero_card, panel_header, toggle_row, BORDER, CARD, CARD_RADIUS};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 420;

pub fn build(
    _: Size,
    battery: Rc<dyn BatteryIntegration>,
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

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("ENERGY", on_back)}
            {hero_card(battery_icon, battery_value, battery_caption.to_owned())}
            {toggle_row("Keep awake", awake, toggle_awake)}
        </Flex>
    })
}
