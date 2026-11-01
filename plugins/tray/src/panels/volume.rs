use crate::shared::VolumeLevel;
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::{fat_slider, settings_panel_header};
use coconut_plugin_kit::{Panel, PanelRenderContext, SharedState};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

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
        build_content(&ctx.shared, &ctx.scratch, ctx.size, None)
    }
}

#[derive(Clone)]
struct SoundScroll(creamui_widgets::ScrollController);

pub fn build_content(
    shared: &SharedState,
    scratch: &SharedState,
    size: Size,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    use super::detail;
    use coconut_api::volume::AudioDirection;
    use creamui_widgets::{layout::Flex, SegmentedControl};
    let volume = shared
        .get::<Rc<dyn VolumeIntegration>>()
        .unwrap_or_else(|| Rc::new(VolumeFallback));
    let volume_level = shared
        .get::<VolumeLevel>()
        .unwrap_or_else(|| VolumeLevel(Signal::new(volume.level())))
        .0;
    volume_level.get();
    let revision = scratch.get_or_insert_with(|| Signal::new(0u64));
    revision.get();
    let tab = scratch.get_or_insert_with(|| Signal::new(0usize));
    let scroll = scratch
        .get_or_insert_with(|| SoundScroll(creamui_widgets::ScrollController::new(0.0)))
        .0;
    let select_tab = tab.clone();
    let reset_scroll = scroll.clone();
    let tabs = SegmentedControl::new(tab.get(), move |index| {
        reset_scroll.set(0.0);
        select_tab.set(index);
    })
    .option("Output")
    .option("Input")
    .option("Applications");
    let mut content = Flex::column().gap(12.0).child(Box::new(tabs));
    if let Some(error) = volume.last_error() {
        content = content.child(detail::error(error));
    }
    let width = detail::slider_width(size);
    let gesture = scratch.get_or_insert_with(super::mixer_slider::MixerGesture::default);
    let devices = volume.devices();
    if tab.get() == 0 {
        let backend = volume.clone();
        let set_level = volume_level.clone();
        let master = fat_slider(
            "volume-high",
            "Output volume",
            volume.level(),
            format!("{}%", (volume.level() * 100.0).round()),
            width,
            None,
            move |level| {
                set_level.set(level);
                backend.set_level(level);
            },
        );
        let mut master = Flex::column().gap(12.0).child(master);
        if volume.supports_mute() {
            let backend = volume.clone();
            let changed = revision.clone();
            let muted = volume.muted();
            master = master.child(detail::toggle(
                "Mute output",
                muted,
                Rc::new(move || {
                    backend.set_muted(!muted);
                    changed.update(|value| *value += 1);
                }),
            ));
        }
        content = content
            .child(detail::card(Box::new(master)))
            .child(detail::text("Output devices", false, 12.0));
    } else if tab.get() == 1 {
        content = content.child(detail::text("Microphones & recording devices", false, 12.0));
    } else {
        let streams = volume.streams();
        if streams.is_empty() {
            content = content.child(detail::card(detail::text(
                if volume.loading() {
                    "Loading application audio…"
                } else {
                    "No applications are currently using audio."
                },
                false,
                13.0,
            )));
        }
        for stream in streams {
            let id = stream.id.clone();
            let backend = volume.clone();
            let changed = revision.clone();
            let mute_id = stream.id.clone();
            let mute_backend = volume.clone();
            let mute_changed = revision.clone();
            let muted = stream.muted;
            content = content.child(channel(
                if stream.direction == AudioDirection::Input {
                    "microphone"
                } else {
                    "volume-high"
                },
                &stream.name,
                &stream.description,
                stream.level,
                stream.muted,
                width,
                Rc::new(move |level| {
                    backend.set_stream_level(&id, level);
                    changed.update(|value| *value += 1);
                }),
                Rc::new(move || {
                    mute_backend.set_stream_muted(&mute_id, !muted);
                    mute_changed.update(|value| *value += 1);
                }),
                None,
                format!("stream:{}", stream.id),
                gesture.clone(),
            ));
        }
    }
    if tab.get() < 2 {
        let direction = if tab.get() == 0 {
            AudioDirection::Output
        } else {
            AudioDirection::Input
        };
        let devices: Vec<_> = devices
            .into_iter()
            .filter(|device| device.direction == direction)
            .collect();
        if devices.is_empty() {
            content = content.child(detail::card(detail::text(
                if volume.loading() {
                    "Loading audio devices…"
                } else {
                    "No audio devices are available."
                },
                false,
                13.0,
            )));
        }
        for device in devices {
            let id = device.id.clone();
            let backend = volume.clone();
            let changed = revision.clone();
            let mute_id = device.id.clone();
            let mute_backend = volume.clone();
            let mute_changed = revision.clone();
            let select_id = device.id.clone();
            let select_backend = volume.clone();
            let select_changed = revision.clone();
            let muted = device.muted;
            let select: Rc<dyn Fn()> = Rc::new(move || {
                select_backend.set_default_device(&select_id);
                select_changed.update(|value| *value += 1);
            });
            content = content.child(channel(
                if direction == AudioDirection::Input {
                    "microphone"
                } else {
                    "speaker"
                },
                &device.name,
                &device.description,
                device.level,
                device.muted,
                width,
                Rc::new(move |level| {
                    backend.set_device_level(&id, level);
                    changed.update(|value| *value += 1);
                }),
                Rc::new(move || {
                    mute_backend.set_device_muted(&mute_id, !muted);
                    mute_changed.update(|value| *value += 1);
                }),
                Some((device.active, select)),
                format!("device:{}", device.id),
                gesture.clone(),
            ));
        }
    }
    detail::panel(
        size,
        settings_panel_header("volume-high", "Sound", "/hardware", on_back),
        scroll,
        Box::new(content),
    )
}

#[allow(clippy::too_many_arguments)]
fn channel(
    icon: &str,
    name: &str,
    description: &str,
    level: f32,
    muted: bool,
    width: f32,
    set_level: Rc<dyn Fn(f32)>,
    toggle_mute: Rc<dyn Fn()>,
    select: Option<(bool, Rc<dyn Fn()>)>,
    id: String,
    gesture: super::mixer_slider::MixerGesture,
) -> BoxedWidget {
    use super::detail;
    use coconut_plugin_kit::chrome::{icon_button, shell_fill, shell_on_accent, shell_track};
    use creamui_widgets::{
        layout::{Align, Flex, Justify},
        Button, ButtonSize, ButtonState, ButtonVariant, RawSlider,
    };
    let mut title =
        Flex::column()
            .gap(3.0)
            .grow(1.0)
            .child(detail::text(name.to_owned(), true, 13.0));
    if !description.is_empty() && description != name {
        title = title.child(detail::text(description.to_owned(), false, 11.0));
    }
    let active = select.as_ref().is_some_and(|(active, _)| *active);
    let mut header = Flex::row()
        .gap(8.0)
        .align(Align::Center)
        .child(coconut_plugin_kit::chrome::icon_badge(icon, 26.0))
        .child(Box::new(title))
        .child(icon_button(
            if icon == "microphone" {
                if muted {
                    "microphone-off"
                } else {
                    "microphone"
                }
            } else if muted {
                "volume-mute"
            } else {
                "volume-high"
            },
            26.0,
            toggle_mute,
        ));
    if let Some((active, select)) = select {
        if active {
            header = header.child(coconut_plugin_kit::pixel_icon(
                "check",
                18.0,
                coconut_plugin_kit::chrome::shell_accent(),
            ));
        } else {
            header = header.child(Box::new(Button::styled(
                ButtonVariant::Secondary,
                ButtonSize::Sm,
                "Use",
                ButtonState::Normal,
                move || select(),
            )));
        }
    }
    let card = detail::selected_card(
        Box::new(
            Flex::column()
                .gap(10.0)
                .child(Box::new(header))
                .child(Box::new(
                    Flex::row()
                        .justify(Justify::Between)
                        .child(detail::text(
                            if muted { "Muted" } else { "Volume" },
                            false,
                            11.0,
                        ))
                        .child(detail::text(
                            format!("{}%", (level * 100.0).round()),
                            true,
                            11.0,
                        )),
                ))
                .child(Box::new(super::mixer_slider::MixerSlider::new(
                    RawSlider::new(
                        creamui_core::Style::new().layout(creamui_core::layout::Style {
                            size: creamui_widgets::layout::fixed(width, 28.0),
                            ..Default::default()
                        }),
                        level,
                        shell_track(),
                        shell_fill(),
                        shell_on_accent(),
                        move |level| set_level(level),
                    )
                    .track(6.0, 3.0)
                    .handle(16.0, 8.0),
                    id.clone(),
                    gesture,
                ))),
        ),
        active,
    );
    creamui_core::keyed(
        creamui_widgets::RawView::new(creamui_core::layout::Style::default()).child(card),
        id,
    )
}
