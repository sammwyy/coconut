use crate::shared::BrightnessLevel;
use coconut_api::brightness::{BrightnessIntegration, Fallback as BrightnessFallback};
use coconut_plugin_kit::chrome::{
    fat_slider, hero_card, panel_header, shell_border, shell_card,
};
use coconut_plugin_kit::{Panel, PanelRenderContext, SharedState};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;
const SLIDER_W: f32 = WIDTH as f32 - 32.0;

/// The device panel wrapper opened directly by the `"tray.brightness"`
/// island (individual tray mode) or by `PanelHost` for any other caller.
/// Delegates to [`build_content`] with `on_back: None`.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::brightness::BrightnessIntegration>`
/// - [`crate::BrightnessLevel`] (`Signal<f32>`) — written directly by a
///   background poller/native-hook listener, so reading it here is already
///   enough for live reactivity; no separate revision signal is used (see
///   [`crate::shared`]'s doc comment).
pub struct BrightnessPanel;

impl Panel for BrightnessPanel {
    fn id(&self) -> &'static str {
        "brightness"
    }

    fn title(&self) -> &'static str {
        "Coconut Brightness"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        build_content(&ctx.shared, None)
    }
}

/// The brightness panel's actual content, shared between [`BrightnessPanel`]
/// and [`crate::panels::control_center::ControlCenterPanel`]'s embedded
/// Brightness view. Ported from
/// `apps/shell/src/panels/brightness/mod.rs::build`.
pub fn build_content(shared: &SharedState, on_back: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let brightness = shared
        .get::<Rc<dyn BrightnessIntegration>>()
        .unwrap_or_else(|| Rc::new(BrightnessFallback));
    let brightness_level = shared
        .get::<BrightnessLevel>()
        .unwrap_or_else(|| BrightnessLevel(Signal::new(brightness.level())))
        .0;
    let value = brightness_level.get();
    let value_text = format!("{}%", (value * 100.0).round());
    let set_brightness = brightness_level.clone();

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)}>
            {panel_header("BRIGHTNESS", on_back)}
            {hero_card("brightness", value_text.clone(), "Display".to_owned())}
            {fat_slider("brightness", "Brightness", value, value_text, SLIDER_W, None, move |level| {
                set_brightness.set(level);
                brightness.set_level(level);
            })}
        </Flex>
    })
}
