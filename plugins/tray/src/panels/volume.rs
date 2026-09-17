use crate::shared::VolumeLevel;
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::{
    fat_slider, hero_card, panel_header, shell_border, shell_card, CARD_RADIUS,
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

/// The device panel wrapper opened directly by the `"tray.volume"` island
/// (individual tray mode) or by `PanelHost` for any other caller. Delegates
/// to [`build_content`] with `on_back: None`.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::volume::VolumeIntegration>`
/// - [`crate::VolumeLevel`] (`Signal<f32>`) — written directly by a
///   background poller/native-hook listener, so reading it here is already
///   enough for live reactivity; no separate revision signal is used (see
///   [`crate::shared`]'s doc comment).
pub struct VolumePanel;

impl Panel for VolumePanel {
    fn id(&self) -> &'static str {
        "volume"
    }

    fn title(&self) -> &'static str {
        "Coconut Volume"
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

/// The volume panel's actual content, shared between [`VolumePanel`] and
/// [`crate::panels::control_center::ControlCenterPanel`]'s embedded Volume
/// view. Ported from `apps/shell/src/panels/volume/mod.rs::build`.
pub fn build_content(shared: &SharedState, on_back: Option<Rc<dyn Fn()>>) -> BoxedWidget {
    let volume = shared
        .get::<Rc<dyn VolumeIntegration>>()
        .unwrap_or_else(|| Rc::new(VolumeFallback));
    let volume_level = shared
        .get::<VolumeLevel>()
        .unwrap_or_else(|| VolumeLevel(Signal::new(volume.level())))
        .0;
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
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("VOLUME", on_back)}
            {hero_card(icon, value_text.clone(), caption.to_owned())}
            {fat_slider(icon, "Volume", value, value_text, SLIDER_W, None, move |level| {
                set_volume.set(level);
                volume.set_level(level);
            })}
        </Flex>
    })
}
