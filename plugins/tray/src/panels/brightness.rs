use crate::shared::BrightnessLevel;
use coconut_api::brightness::{BrightnessIntegration, Fallback as BrightnessFallback};
use coconut_plugin_kit::chrome::settings_panel_header;
use coconut_plugin_kit::{Panel, PanelRenderContext, SharedState};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

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
        build_content(&ctx.shared, &ctx.scratch, ctx.size, None)
    }
}

#[derive(Clone)]
struct BrightnessScroll(creamui_widgets::ScrollController);

pub fn build_content(
    shared: &SharedState,
    scratch: &SharedState,
    size: Size,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    use super::detail;
    use creamui_widgets::{
        layout::{Flex, Wrap},
        Button, ButtonSize, ButtonState, ButtonVariant,
    };
    let brightness = shared
        .get::<Rc<dyn BrightnessIntegration>>()
        .unwrap_or_else(|| Rc::new(BrightnessFallback));
    let level = shared
        .get::<BrightnessLevel>()
        .unwrap_or_else(|| BrightnessLevel(Signal::new(brightness.level())))
        .0;
    let value = level.get();
    let scroll = scratch
        .get_or_insert_with(|| BrightnessScroll(creamui_widgets::ScrollController::new(0.0)))
        .0;
    let mut body = Flex::column().gap(12.0);
    if let Some(error) = brightness.last_error() {
        body = body.child(detail::error(error));
    }
    if brightness.available() {
        let backend = brightness.clone();
        let write = level.clone();
        body = body.child(super::control_tiles::slider_tile(
            Size {
                width: (size.width - 42.0).max(80.0),
                height: 210.0,
            },
            "brightness",
            "Brightness",
            value,
            format!("{}%", (value * 100.0).round()),
            Rc::new(|| coconut_plugin_kit::open_settings("/hardware/displays")),
            move |value| {
                write.set(value);
                backend.set_level(value);
            },
        ));
        let mut presets = Flex::row().gap(6.0).wrap(Wrap::Wrap);
        for percent in [10, 25, 50, 75, 100] {
            let backend = brightness.clone();
            let level = level.clone();
            presets = presets.child(Box::new(Button::styled(
                if (value * 100.0 - percent as f32).abs() < 1.0 {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                },
                ButtonSize::Sm,
                format!("{percent}%"),
                ButtonState::Normal,
                move || {
                    let value = percent as f32 / 100.0;
                    level.set(value);
                    backend.set_level(value);
                },
            )));
        }
        body = body
            .child(detail::text("Quick levels", false, 12.0))
            .child(Box::new(presets));
    } else {
        body = body.child(detail::card(detail::text(
            "No controllable backlight was found. External displays need DDC/CI support.",
            false,
            13.0,
        )));
    }
    if let (Some(awake), Some(toggle)) = (
        shared.get::<crate::KeepAwakeState>(),
        shared.get::<crate::ToggleKeepAwake>(),
    ) {
        body = body.child(detail::card(Box::new(
            Flex::column()
                .gap(8.0)
                .child(detail::toggle("Keep awake", awake.0.get(), toggle.0))
                .child(detail::text(
                    "Prevent automatic sleep while you need the display.",
                    false,
                    12.0,
                )),
        )));
    }
    detail::panel(
        size,
        settings_panel_header("brightness", "Display", "/hardware/displays", on_back),
        scroll,
        Box::new(body),
    )
}
