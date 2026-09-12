use crate::icons::pixel_icon;
use crate::panels::chrome::{ACCENT, BORDER, CARD, CARD_RADIUS, FILL, MUTED, TEXT, TRACK};
use crate::platform::Playback;
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::{
    layout::{fixed, Align, Justify},
    RawSlider,
};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

pub const WIDTH: u32 = 310;
pub const HEIGHT: u32 = 190;

pub fn build(
    _: Size,
    playback: Option<Playback>,
    toggle: Rc<dyn Fn()>,
    previous: Rc<dyn Fn()>,
    next: Rc<dyn Fn()>,
    seek: Rc<dyn Fn(f64)>,
) -> BoxedWidget {
    let position = playback
        .as_ref()
        .and_then(|item| parse_time(&item.position));
    let length = playback.as_ref().and_then(|item| parse_time(&item.length));
    let progress = match (position, length) {
        (Some(position), Some(length)) if length > 0.0 => {
            (position / length).clamp(0.0, 1.0) as f32
        }
        _ => 0.0,
    };
    let seek_enabled = length.is_some_and(|length| length > 0.0);
    let cover: BoxedWidget = match cover_data(playback.as_ref()) {
        Some(data) => Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: creamui_widgets::layout::fixed(72.0, 72.0),
                    ..Default::default()
                })
                .fit(ImageFit::Cover),
        ),
        None => Box::new(jsx! { <Flex size={(72.0, 72.0)} /> }),
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={14.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            <Flex direction={FlexDirection::Row} gap={12.0} align={Align::Center}>
                {cover}
                <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                    {Box::new(creamui_widgets::RawMarquee::expanding(playback.as_ref().map(|item| item.title.as_str()).unwrap_or("Nothing playing"), TEXT, 15.0)) as BoxedWidget}
                    <RawText color={MUTED} font_size={11.0}>{playback.as_ref().map(|item| item.artist.as_str()).unwrap_or("No MPRIS player")}</RawText>
                    <RawText color={MUTED} font_size={10.0}>{format!("{}  ·  {}", playback.as_ref().map(|item| item.position.as_str()).unwrap_or("0:00"), playback.as_ref().map(|item| item.length.as_str()).unwrap_or("0:00"))}</RawText>
                </Flex>
                <RawButton style={control_style()} on_click={move || toggle()}>
                    <Flex size={(38.0, 38.0)} align={Align::Center} justify={Justify::Center}>{pixel_icon(if playback.as_ref().is_some_and(|item| item.status == "Playing") { "pause" } else { "play" }, 18.0)}</Flex>
                </RawButton>
            </Flex>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                {media_button("|<", previous)}
                <RawText color={MUTED} font_size={10.0}>{playback.as_ref().map(|item| item.position.as_str()).unwrap_or("0:00")}</RawText>
                {progress_slider(progress, seek_enabled, seek)}
                <RawText color={MUTED} font_size={10.0}>{playback.as_ref().map(|item| item.length.as_str()).unwrap_or("0:00")}</RawText>
                {media_button(">|", next)}
            </Flex>
        </Flex>
    })
}

fn progress_slider(progress: f32, enabled: bool, seek: Rc<dyn Fn(f64)>) -> BoxedWidget {
    let seek = move |value: f32| seek(value as f64);
    Box::new(
        RawSlider::new(
            Style::new()
                .layout(LayoutStyle {
                    size: fixed(154.0, 26.0),
                    ..Default::default()
                })
                .focus(StateStyle::new().outline(Color::rgba(0, 0, 0, 0), 0.0)),
            progress,
            TRACK,
            FILL,
            TEXT,
            seek,
        )
        .track(8.0, 4.0)
        .handle(14.0, 7.0)
        .hover_handle_color(Color::rgb(255, 255, 255))
        .pressed_handle_color(FILL)
        .disabled(!enabled),
    )
}

fn media_button(label: &str, on_click: Rc<dyn Fn()>) -> BoxedWidget {
    let label = label.to_owned();
    Box::new(jsx! {
        <RawButton style={small_control_style()} on_click={move || on_click()}>
            <Flex size={(24.0, 28.0)} align={Align::Center} justify={Justify::Center}>
                <RawText color={TEXT} font_size={11.0}>{label}</RawText>
            </Flex>
        </RawButton>
    })
}

fn cover_data(playback: Option<&Playback>) -> Option<ImageData> {
    playback
        .and_then(|item| item.art_url.as_ref())
        .and_then(|path| {
            const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
            std::fs::metadata(path)
                .ok()
                .filter(|meta| meta.len() <= MAX_ART_BYTES)
                .and_then(|_| ImageData::from_path(path).ok())
        })
        .or_else(|| {
            playback
                .and_then(|item| item.app_icon.as_ref())
                .and_then(safe_image_data)
        })
}

fn safe_image_data(path: &std::path::PathBuf) -> Option<ImageData> {
    const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf, ImageData>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(data) = cache.lock().ok().and_then(|cache| cache.get(path).cloned()) {
        return Some(data);
    }
    let data = std::fs::metadata(path)
        .ok()
        .filter(|meta| meta.len() <= MAX_ICON_BYTES)
        .and_then(|_| ImageData::from_path(path).ok())?;
    if let Ok(mut cache) = cache.lock() {
        cache.insert(path.clone(), data.clone());
    }
    Some(data)
}

fn control_style() -> creamui_core::Style {
    creamui_core::Style::new()
        .layout(creamui_core::layout::Style {
            size: creamui_widgets::layout::fixed(38.0, 38.0),
            ..Default::default()
        })
        .background(ACCENT)
        .corner_radius(11.0)
}

fn small_control_style() -> creamui_core::Style {
    creamui_core::Style::new()
        .layout(creamui_core::layout::Style {
            size: fixed(24.0, 28.0),
            ..Default::default()
        })
        .background(ACCENT)
        .corner_radius(8.0)
}

fn parse_time(value: &str) -> Option<f64> {
    let (minutes, seconds) = value.split_once(':')?;
    Some(minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?)
}
