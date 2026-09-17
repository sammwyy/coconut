use crate::shared::{CurrentPlayback, NextPlayback, PreviousPlayback, TogglePlayback};
use coconut_api::audio::Playback;
use coconut_plugin_kit::chrome::{shell_control_hover, shell_muted, shell_panel, shell_text};
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, StateStyle, Style, Styled, TextAlign};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_widgets::layout::{fixed, Align, Justify};
use creamui_widgets::RawButton;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

const SIDE_ITEM_SIZE: f32 = 36.0;

/// The dock's now-playing island: cover art, transport controls and elapsed
/// time on a horizontal dock, a fixed play icon on a vertical one. Hides
/// itself entirely (see [`Island::is_visible`]) when nothing is playing.
/// Opens the `"current_playing"` panel on click either way.
///
/// Moved from `apps/shell/src/bar/mod.rs`'s `compact_playback_widget`,
/// `music_cover` and `safe_image_data`. See [`crate::shared`] for the exact
/// `SharedState` entries this island expects `apps/shell/src/lib.rs`
/// (Phase 4d) to provide.
pub struct CurrentPlayingIsland;

impl Island for CurrentPlayingIsland {
    fn id(&self) -> &'static str {
        "current_playing"
    }

    fn is_visible(&self, ctx: &IslandRenderContext) -> bool {
        current_playback(ctx).is_some()
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let open = ctx.open_panel.clone();
        if ctx.position.is_vertical() {
            return Box::new(
                RawButton::new(side_button_style(), || {})
                    .with_click_position(move |point| open("current_playing", point))
                    .child(Box::new(jsx! {
                        <Flex size={(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE)} align={Align::Center} justify={Justify::Center}>
                            {pixel_icon("player_play", 19.0, shell_text())}
                        </Flex>
                    })),
            );
        }

        let playback = current_playback(ctx);
        let toggle = ctx
            .shared
            .get::<TogglePlayback>()
            .map(|value| value.0)
            .unwrap_or_else(|| Rc::new(|| {}));
        let previous = ctx
            .shared
            .get::<PreviousPlayback>()
            .map(|value| value.0)
            .unwrap_or_else(|| Rc::new(|| {}));
        let next = ctx
            .shared
            .get::<NextPlayback>()
            .map(|value| value.0)
            .unwrap_or_else(|| Rc::new(|| {}));

        compact_playback_widget(playback.as_ref(), open, previous, toggle, next)
    }
}

fn current_playback(ctx: &IslandRenderContext) -> Option<Playback> {
    ctx.shared
        .get::<CurrentPlayback>()
        .and_then(|current| (current.0)())
}

fn compact_playback_widget(
    playback: Option<&Playback>,
    open: Rc<dyn Fn(&'static str, creamui_core::Point)>,
    previous: Rc<dyn Fn()>,
    toggle: Rc<dyn Fn()>,
    next: Rc<dyn Fn()>,
) -> BoxedWidget {
    let duration = playback
        .map(|item| item.position.as_str())
        .filter(|position| !position.is_empty());
    let width = if duration.is_some() { 144.0 } else { 108.0 };
    let duration_widget: BoxedWidget = duration
        .map(|duration| {
            Box::new(jsx! {
                <RawText color={shell_muted()} font_size={10.0} align={TextAlign::End}>{duration}</RawText>
            }) as BoxedWidget
        })
        .unwrap_or_else(|| Box::new(creamui_widgets::layout::Flex::row().size(0.0, 0.0)));
    let play_icon = if playback.is_some_and(|item| item.status == "Playing") {
        "player_pause"
    } else {
        "player_play"
    };
    Box::new(
        RawButton::new(island_button_style(width), || {})
            .with_click_position(move |point| open("current_playing", point))
            .child(Box::new(jsx! {
                <Flex direction={FlexDirection::Row} size={(width, 32.0)} padding={4.0} gap={3.0} align={Align::Center}>
                    {music_cover(playback)}
                    {compact_media_button("player_previous", previous)}
                    {compact_media_button(play_icon, toggle)}
                    {compact_media_button("player_next", next)}
                    {duration_widget}
                </Flex>
            })),
    )
}

fn compact_media_button(icon: &'static str, on_click: Rc<dyn Fn()>) -> BoxedWidget {
    Box::new(
        RawButton::new(media_control_style(), || {})
            .with_click_position(move |_| on_click())
            .child(Box::new(jsx! {
                <Flex size={(18.0, 24.0)} align={Align::Center} justify={Justify::Center}>
                    {pixel_icon(icon, 13.0, shell_text())}
                </Flex>
            })),
    )
}

fn music_cover(playback: Option<&Playback>) -> BoxedWidget {
    let Some(data) = playback
        .and_then(|item| item.art_url.as_ref())
        .and_then(safe_image_data)
    else {
        let Some(data) = playback
            .and_then(|item| item.app_icon.as_ref())
            .and_then(safe_image_data)
        else {
            return Box::new(jsx! { <Flex size={(24.0, 24.0)} /> });
        };
        return Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: fixed(24.0, 24.0),
                    ..Default::default()
                })
                .fit(ImageFit::Contain),
        );
    };
    Box::new(
        Image::new(data)
            .layout(LayoutStyle {
                size: fixed(24.0, 24.0),
                ..Default::default()
            })
            .fit(ImageFit::Cover),
    )
}

fn safe_image_data(path: &PathBuf) -> Option<ImageData> {
    const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, ImageData>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(data) = cache.lock().ok().and_then(|cache| cache.get(path).cloned()) {
        return Some(data);
    }
    let data = std::fs::metadata(path)
        .ok()
        .filter(|meta| meta.len() <= MAX_ART_BYTES)
        .and_then(|_| ImageData::from_path(path).ok())?;
    if let Ok(mut cache) = cache.lock() {
        cache.insert(path.clone(), data.clone());
    }
    Some(data)
}

fn island_button_style(width: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(width, 32.0),
            ..Default::default()
        })
        .background(shell_panel())
        .corner_radius(9.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_control_hover()))
}

fn side_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE),
            ..Default::default()
        })
        .background(shell_panel())
        .corner_radius(8.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_control_hover()))
}

fn media_control_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(18.0, 24.0),
            ..Default::default()
        })
        .corner_radius(6.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_control_hover()))
}
