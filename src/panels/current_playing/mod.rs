use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::layout::Align;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const CARD: Color = Color::rgba(31, 32, 34, 252);
const TEXT: Color = Color::rgb(244, 244, 245);
const MUTED: Color = Color::rgb(166, 168, 171);
const CONTROL: Color = Color::rgba(255, 255, 255, 18);

pub const WIDTH: u32 = 310;
pub const HEIGHT: u32 = 128;

pub fn build(_: Size, playback: Option<Playback>, toggle: std::rc::Rc<dyn Fn()>) -> BoxedWidget {
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
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} corner_radius={14.0}>
            <Flex direction={FlexDirection::Row} gap={12.0} align={Align::Center}>
                {cover}
                <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                    {Box::new(creamui_widgets::RawMarquee::expanding(playback.as_ref().map(|item| item.title.as_str()).unwrap_or("Nothing playing"), TEXT, 15.0)) as BoxedWidget}
                    <RawText color={MUTED} font_size={11.0}>{playback.as_ref().map(|item| item.artist.as_str()).unwrap_or("No MPRIS player")}</RawText>
                    <RawText color={MUTED} font_size={10.0}>{format!("{}  ·  {}", playback.as_ref().map(|item| item.position.as_str()).unwrap_or("0:00"), playback.as_ref().map(|item| item.length.as_str()).unwrap_or("0:00"))}</RawText>
                </Flex>
                <RawButton style={control_style()} on_click={move || toggle()}>
                    <Flex size={(34.0, 34.0)} align={Align::Center} justify={creamui_widgets::layout::Justify::Center}>{pixel_icon(if playback.as_ref().is_some_and(|item| item.status == "Playing") { "pause" } else { "play" }, 17.0)}</Flex>
                </RawButton>
            </Flex>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                <RawText color={MUTED} font_size={10.0}>"‹"</RawText>
                <Flex grow={1.0} size={(0.0, 2.0)} background={MUTED} corner_radius={1.0} />
                <RawText color={MUTED} font_size={10.0}>"›"</RawText>
            </Flex>
        </Flex>
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
            size: creamui_widgets::layout::fixed(34.0, 34.0),
            ..Default::default()
        })
        .background(CONTROL)
        .corner_radius(10.0)
}
use crate::icons::pixel_icon;
use crate::platform::Playback;
