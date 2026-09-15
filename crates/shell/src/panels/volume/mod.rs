use crate::panels::chrome::{fat_slider, hero_card, panel_header, BORDER, CARD, CARD_RADIUS};
use coconut_api::volume::VolumeIntegration;
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
    volume: Rc<dyn VolumeIntegration>,
    volume_level: Signal<f32>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let value = volume_level.get();
    let muted = volume.muted();
    let value_text = format!(
        "{}%{}",
        (value * 100.0).round(),
        if muted { "  M" } else { "" }
    );
    let icon = if muted || value <= 0.01 {
        "volume-mute"
    } else if value < 0.34 {
        "volume-low"
    } else if value < 0.67 {
        "volume-mid"
    } else {
        "volume-high"
    };
    let caption = if muted { "Muted" } else { "Volume" };
    let set_volume = volume_level.clone();

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("VOLUME", on_back)}
            {hero_card(icon, value_text.clone(), caption.to_owned())}
            {fat_slider(icon, "Volume", value, value_text, SLIDER_W, None, move |level| {
                set_volume.set(level);
                volume.set_level(level);
            })}
        </Flex>
    })
}
