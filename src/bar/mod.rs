pub mod widgets;

use crate::icons::pixel_icon;
use crate::platform::{OpenWindow, Playback};
use chrono::Local;
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{
    BoxedWidget, Painter, Rect, Size, StateStyle, Style, Styled, TextAlign, Widget,
};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::{Color, ColorScheme, Theme};
use creamui_widgets::layout::{fixed, Align, Flex, Justify};
use creamui_widgets::{RawButton, RawMarquee};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

pub const DOCK_HEIGHT: u32 = 44;
const BAR_HEIGHT: f32 = 44.0;
const TASK_SIZE: f32 = 32.0;
const MAX_TASKS: usize = 10;
const CLOCK_WIDTH: f32 = 60.0;
const EDGE_GAP: f32 = 16.0;

const BAR: Color = Color::rgba(20, 21, 22, 248);
const ISLAND: Color = Color::rgba(39, 40, 42, 245);
const ISLAND_BORDER: Color = Color::rgba(255, 255, 255, 20);
const CONTROL: Color = Color::rgba(255, 255, 255, 12);
const CONTROL_HOVER: Color = Color::rgba(255, 255, 255, 26);
const SELECTED: Color = Color::rgba(255, 255, 255, 42);
const PRIMARY: Color = Color::rgb(244, 244, 245);
const MUTED: Color = Color::rgb(166, 168, 171);

pub struct CreamTheme;

#[derive(Clone)]
pub struct BarActions {
    pub refresh_windows: Rc<dyn Fn()>,
    pub activate_window: Rc<dyn Fn(String)>,
    pub current_playback: Rc<dyn Fn() -> Option<Playback>>,
    pub toggle_playback: Rc<dyn Fn()>,
    pub open_weather: Rc<dyn Fn()>,
    pub open_clock: Rc<dyn Fn()>,
    pub open_current_playing: Rc<dyn Fn()>,
    pub open_app_drawer: Rc<dyn Fn()>,
    pub open_control_center: Rc<dyn Fn()>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemStatus {
    pub network_connected: bool,
    pub network_strength: Option<u8>,
    pub battery_percentage: Option<u8>,
    pub battery_charging: bool,
    pub bluetooth_powered: bool,
    pub bluetooth_connected: bool,
    pub volume: f32,
    pub volume_muted: bool,
}

impl CreamTheme {
    pub fn theme() -> Theme {
        Theme::dark().with_colors(ColorScheme {
            surface: BAR,
            surface_elevated: CONTROL,
            surface_hover: CONTROL_HOVER,
            accent: PRIMARY,
            accent_hover: Color::rgb(255, 255, 255),
            accent_pressed: Color::rgb(205, 206, 208),
            selection_background: SELECTED,
            selection_text: PRIMARY,
            text_primary: PRIMARY,
            text_secondary: MUTED,
            text_disabled: Color::rgb(104, 106, 109),
            border: ISLAND_BORDER,
            border_strong: Color::rgba(255, 255, 255, 36),
            danger: Color::rgb(220, 220, 220),
            warning: Color::rgb(220, 220, 220),
            success: Color::rgb(220, 220, 220),
        })
    }
}

pub fn build_dock(
    viewport: Size,
    windows: Vec<OpenWindow>,
    launcher_open: Signal<bool>,
    active_window: Signal<Option<String>>,
    status: SystemStatus,
    actions: BarActions,
) -> BoxedWidget {
    let launcher_background = if launcher_open.get() {
        SELECTED
    } else {
        CONTROL
    };
    let (tasks, task_count) = window_strip(
        windows,
        active_window,
        actions.activate_window.clone(),
        actions.refresh_windows.clone(),
    );
    let dock_width = dock_width(task_count);
    let clock: BoxedWidget = Box::new(LiveClock);
    let playback = (actions.current_playback)();
    let cover = music_cover(playback.as_ref());
    let section_width = viewport.width / 3.0;
    let weather_click = actions.open_weather.clone();
    let clock_click = actions.open_clock.clone();
    let music_click = actions.open_current_playing.clone();
    let drawer_click = actions.open_app_drawer.clone();
    let control_click = actions.open_control_center.clone();
    let network_icon = network_icon(status.network_connected, status.network_strength);
    let battery_icon = battery_icon(status.battery_percentage, status.battery_charging);
    let bluetooth_icon = if status.bluetooth_connected {
        "bluetooth-connected"
    } else if status.bluetooth_powered {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    };
    let volume_icon = volume_icon(status.volume, status.volume_muted);

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(viewport.width, viewport.height)} justify={Justify::End} align={Align::Center} background={Color::rgba(0, 0, 0, 0)}>
            <Flex direction={FlexDirection::Row} size={(viewport.width, BAR_HEIGHT)} align={Align::Center} background={BAR} border={(ISLAND_BORDER, 1.0)}>
                <Flex direction={FlexDirection::Row} size={(section_width, BAR_HEIGHT)} gap={6.0} justify={Justify::Start} align={Align::Center}>
                    <Flex size={(EDGE_GAP, BAR_HEIGHT)} />
                    <Flex direction={FlexDirection::Row} size={(48.0, 32.0)} padding={5.0} gap={2.0} align={Align::Center} background={ISLAND} border={(ISLAND_BORDER, 1.0)} corner_radius={9.0}>
                        <Flex size={(5.0, 5.0)} background={PRIMARY} corner_radius={2.5} />
                        <Flex size={(3.0, 3.0)} background={MUTED} corner_radius={1.5} />
                        <Flex size={(3.0, 3.0)} background={MUTED} corner_radius={1.5} />
                        <Flex size={(3.0, 3.0)} background={MUTED} corner_radius={1.5} />
                        <Flex size={(3.0, 3.0)} background={MUTED} corner_radius={1.5} />
                    </Flex>
                    <RawButton style={island_button_style(126.0, 32.0)} on_click={move || weather_click()}>
                        <Flex direction={FlexDirection::Row} size={(126.0, 32.0)} padding={6.0} gap={4.0} align={Align::Center}>
                            {pixel_icon("weather-sun-cloud", 14.0)}
                            <RawText color={MUTED} font_size={10.0} align={TextAlign::Start}>{widgets::weather::DEFAULT_SUMMARY}</RawText>
                        </Flex>
                    </RawButton>
                    <RawButton style={island_button_style(206.0, 32.0)} on_click={move || music_click()}>
                        <Flex direction={FlexDirection::Row} size={(206.0, 32.0)} padding={5.0} gap={4.0} align={Align::Center}>
                            {cover}
                            <RawButton style={media_control_style()} on_click={move || (actions.toggle_playback)()}><Flex size={(18.0, 24.0)} align={Align::Center} justify={Justify::Center}>{pixel_icon(if playback.as_ref().is_some_and(|item| item.status == "Playing") { "pause" } else { "play" }, 14.0)}</Flex></RawButton>
                            {Box::new(RawMarquee::new(widgets::current_playing::summary(playback.as_ref()), MUTED, 10.0, 142.0)) as BoxedWidget}
                        </Flex>
                    </RawButton>
                </Flex>
                <Flex direction={FlexDirection::Row} size={(section_width, BAR_HEIGHT)} justify={Justify::Center} align={Align::Center}>
                    <Flex direction={FlexDirection::Row} size={(dock_width, 40.0)} padding={4.0} gap={6.0} align={Align::Center} background={ISLAND} border={(ISLAND_BORDER, 1.0)} corner_radius={12.0}>
                        <RawButton style={square_style(launcher_background)} on_click={move || drawer_click()}>
                            <Flex size={(32.0, 32.0)} align={Align::Center} justify={Justify::Center}>{pixel_icon("appgrid", 21.0)}</Flex>
                        </RawButton>
                        {tasks}
                    </Flex>
                </Flex>
                <Flex direction={FlexDirection::Row} size={(section_width, BAR_HEIGHT)} gap={6.0} justify={Justify::End} align={Align::Center}>
                    <Flex size={(EDGE_GAP, BAR_HEIGHT)} />
                    <RawButton style={island_button_style(146.0, 32.0)} on_click={move || control_click()}>
                        <Flex direction={FlexDirection::Row} size={(146.0, 32.0)} padding={4.0} gap={7.0} justify={Justify::Center} align={Align::Center}>
                            {pixel_icon(network_icon, 16.0)}
                            {pixel_icon("brightness", 16.0)}
                            {pixel_icon(volume_icon, 16.0)}
                            {pixel_icon(bluetooth_icon, 16.0)}
                            {pixel_icon(battery_icon, 16.0)}
                            {pixel_icon("chevron-up", 13.0)}
                        </Flex>
                    </RawButton>
                    <RawButton style={island_button_style(68.0, 32.0)} on_click={move || clock_click()}>
                        <Flex direction={FlexDirection::Column} size={(68.0, 32.0)} padding={2.0} justify={Justify::Center} align={Align::Center}>
                            {clock}
                        </Flex>
                    </RawButton>
                    <Flex size={(EDGE_GAP, BAR_HEIGHT)} />
                </Flex>
            </Flex>
        </Flex>
    })
}

fn network_icon(connected: bool, strength: Option<u8>) -> &'static str {
    if !connected {
        return "wifi-off";
    }
    match strength.unwrap_or(100) {
        75.. => "wifi-high",
        50..=74 => "wifi-mid",
        25..=49 => "wifi-low",
        _ => "wifi-none",
    }
}

fn battery_icon(percentage: Option<u8>, charging: bool) -> &'static str {
    if charging {
        return "battery-bolt";
    }
    match percentage.unwrap_or(0) {
        80.. => "battery-full",
        50..=79 => "battery-mid",
        20..=49 => "battery-low",
        _ => "battery-empty",
    }
}

fn volume_icon(level: f32, muted: bool) -> &'static str {
    if muted || level <= 0.01 {
        "volume-mute"
    } else if level < 0.34 {
        "volume-low"
    } else if level < 0.67 {
        "volume-mid"
    } else {
        "volume-max"
    }
}

struct LiveClock;

impl Widget for LiveClock {
    fn style(&self) -> Style {
        Style::new().layout(LayoutStyle {
            size: fixed(CLOCK_WIDTH, 24.0),
            ..Default::default()
        })
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.animation_time();
        let now = Local::now();
        let time = now.format("%H:%M").to_string();
        painter.fill_text_font(
            rect,
            &time,
            PRIMARY,
            13.0,
            TextAlign::Center,
            None,
            false,
            false,
        );
    }
}

fn window_strip(
    windows: Vec<OpenWindow>,
    active_window: Signal<Option<String>>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
) -> (BoxedWidget, usize) {
    if windows.is_empty() {
        return (
            Box::new(jsx! {
                <Flex direction={FlexDirection::Row} grow={1.0} align={Align::Center}>
                    <RawText color={MUTED} font_size={13.0}>"No windows"</RawText>
                </Flex>
            }),
            0,
        );
    }

    let count = windows.len().min(MAX_TASKS);
    let mut strip = Flex::row().gap(6.0).align(Align::Center);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let selected = active_window.get().as_deref() == Some(entry.id.as_str()) || entry.active;
        let select = active_window.clone();
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        let color = if selected { SELECTED } else { CONTROL };
        let icon = app_icon(&entry);
        let centered_icon = Box::new(
            Flex::row()
                .size(TASK_SIZE, TASK_SIZE)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(icon),
        );
        let item: BoxedWidget = Box::new(
            RawButton::new(window_style(color), move || {
                select.set(Some(id.clone()));
                activate(id.clone());
                refresh();
            })
            .child(centered_icon),
        );
        strip = strip.child(item);
    }
    (Box::new(strip), count)
}

fn dock_width(tasks: usize) -> f32 {
    if tasks == 0 {
        120.0
    } else {
        46.0 + tasks as f32 * TASK_SIZE + (tasks - 1) as f32 * 6.0
    }
}

fn app_icon(window: &OpenWindow) -> BoxedWidget {
    if let Some(data) = window
        .icon_path
        .as_ref()
        .and_then(|path| ImageData::from_path(path).ok())
    {
        return Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: fixed(22.0, 22.0),
                    ..Default::default()
                })
                .fit(ImageFit::Contain),
        );
    }
    Box::new(jsx! {
        <RawText color={MUTED} font_size={15.0} align={TextAlign::Center}>{fallback_icon(&window.app_name)}</RawText>
    })
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
            return Box::new(jsx! { <Flex size={(30.0, 30.0)} /> });
        };
        return Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: fixed(30.0, 30.0),
                    ..Default::default()
                })
                .fit(ImageFit::Contain),
        );
    };
    Box::new(
        Image::new(data)
            .layout(LayoutStyle {
                size: fixed(30.0, 30.0),
                ..Default::default()
            })
            .fit(ImageFit::Cover),
    )
}

fn safe_image_data(path: &std::path::PathBuf) -> Option<ImageData> {
    const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf, ImageData>>> = OnceLock::new();
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

fn fallback_icon(app_name: &str) -> String {
    app_name
        .chars()
        .next()
        .unwrap_or('•')
        .to_uppercase()
        .to_string()
}

fn square_style(background: Color) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE, TASK_SIZE),
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(PRIMARY))
}

fn island_button_style(width: f32, height: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(width, height),
            ..Default::default()
        })
        .background(ISLAND)
        .corner_radius(9.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(SELECTED))
}

fn window_style(background: Color) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE, TASK_SIZE),
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(PRIMARY))
}

fn media_control_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(18.0, 24.0),
            ..Default::default()
        })
        .corner_radius(6.0)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(PRIMARY))
}

#[cfg(test)]
mod tests {
    use super::{dock_width, fallback_icon};

    #[test]
    fn dock_fits_visible_tasks() {
        assert_eq!(dock_width(0), 120.0);
        assert_eq!(dock_width(4), 192.0);
    }

    #[test]
    fn fallback_uses_the_app_initial() {
        assert_eq!(fallback_icon("firefox"), "F");
    }
}
