use coconut_api::desktop::OpenWindow;
use coconut_core::DockPosition;
use coconut_plugin_kit::chrome::{
    island_style, shell_control, shell_control_hover, shell_island,
    shell_island_border as shell_border, shell_selected, shell_text,
};
use coconut_plugin_kit::{
    design, pixel_icon, ConfigField, ConfigValue, IconRequest, IconResolver, Island,
    IslandRenderContext, NumberRange,
};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Point, StateStyle, Style, Styled};
use creamui_image::{Image, ImageData, ImageFit, SvgSize};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::Color;
use creamui_widgets::layout::{fixed, Align, Flex, Justify};
use creamui_widgets::RawButton;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

const TASK_SIZE: f32 = 28.0;
const TASK_HEIGHT: f32 = 36.0;
const MAX_TASKS: usize = 10;
const SIDE_ITEM_SIZE: f32 = 36.0;

const ICON_SCALE_KEY: &str = "icon_scale";
const SHOW_BACKGROUND_KEY: &str = "show_background";
const ICON_SCALE_RANGE: NumberRange = NumberRange {
    min: 50.0,
    max: 200.0,
    step: 5.0,
};

/// The live open-window list plus the window-management callbacks
/// [`OpenWindowsIsland`] needs. A dedicated newtype (rather than a bare
/// `Signal<Vec<OpenWindow>>`/`Rc<dyn Fn()>`) since `SharedState` is keyed by
/// type alone and those bare shapes are common enough to collide.
#[derive(Clone)]
pub struct WindowListState {
    pub windows: Signal<Vec<OpenWindow>>,
    pub refresh: Rc<dyn Fn()>,
    pub activate: Rc<dyn Fn(String)>,
}

impl Default for WindowListState {
    fn default() -> Self {
        Self {
            windows: Signal::new(Vec::new()),
            refresh: Rc::new(|| {}),
            activate: Rc::new(|_| {}),
        }
    }
}

/// Whether the app drawer panel is open, used to highlight
/// [`AppLauncherIsland`]'s own button.
#[derive(Clone)]
pub struct LauncherOpenSignal(pub Signal<bool>);

#[derive(Clone)]
pub struct WindowIconResolver(pub Signal<IconResolver>);

impl Default for LauncherOpenSignal {
    fn default() -> Self {
        Self(Signal::new(false))
    }
}

/// Opens the app drawer panel. The open-window task strip is
/// [`OpenWindowsIsland`], a separate island so it can be placed, reordered,
/// or hidden independently.
pub struct AppLauncherIsland;

impl Island for AppLauncherIsland {
    fn id(&self) -> &'static str {
        "app_launcher"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let s = ctx.scale;
        let launcher_open = ctx
            .shared
            .get::<LauncherOpenSignal>()
            .unwrap_or_default()
            .0
            .get();
        let open_panel = ctx.open_panel.clone();
        let drawer_click: Rc<dyn Fn(Point)> = Rc::new(move |at| open_panel("app_drawer", at));

        if ctx.position.is_vertical() {
            return side_icon_button_styled("appgrid", drawer_click, launcher_open, s);
        }

        let background = creamui_core::LinearGradient::new(
            180.0,
            Color::rgb(155, 179, 188),
            Color::rgb(76, 105, 120),
        );
        let launcher: BoxedWidget = Box::new(
            RawButton::new(square_style(s), || {})
                .with_click_position(move |point| drawer_click(point))
                .child(Box::new(jsx! {
                    <Flex direction={FlexDirection::Column} size={(TASK_SIZE * s, TASK_HEIGHT * s)} gap={3.0 * s} align={Align::Center} justify={Justify::Center}>
                        <Flex size={(TASK_SIZE * s, TASK_SIZE * s)} background={background} border={(shell_border(), 1.0)} corner_radius={8.0 * s} box_shadow={design::card_shadow()} align={Align::Center} justify={Justify::Center}>{pixel_icon("appgrid", 17.0 * s, Color::rgb(255, 255, 255))}</Flex>
                        <Flex size={(2.5 * s, 2.5 * s)} background={if launcher_open { shell_text() } else { Color::rgba(0, 0, 0, 0) }} corner_radius={1.25 * s} />
                    </Flex>
                })),
        );
        if window_list_state(ctx).windows.get().is_empty() {
            launcher
        } else {
            Box::new(
                Flex::row()
                    .align(Align::Center)
                    .gap(6.0 * s)
                    .child(launcher)
                    .child(Box::new(
                        Flex::column()
                            .size(1.0 * s, TASK_SIZE * s)
                            .background(coconut_plugin_kit::apply_opacity(shell_text(), 0.12)),
                    )),
            )
        }
    }
}

/// The dock's open-window task strip. Hides itself with no windows open.
pub struct OpenWindowsIsland;

impl Island for OpenWindowsIsland {
    fn id(&self) -> &'static str {
        "open_windows"
    }

    fn config_schema(&self) -> Vec<ConfigField> {
        vec![
            ConfigField::slider(ICON_SCALE_KEY, "Icon size", 100.0, ICON_SCALE_RANGE, "%"),
            ConfigField::toggle(SHOW_BACKGROUND_KEY, "Icon background", false),
            ConfigField::toggle("compact", "Compact statusbar icons", false),
        ]
    }

    fn is_visible(&self, ctx: &IslandRenderContext) -> bool {
        !window_list_state(ctx).windows.get().is_empty()
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let icon_scale_pct = ctx
            .config
            .get(ICON_SCALE_KEY)
            .and_then(ConfigValue::as_f64)
            .unwrap_or(100.0);
        let scale = ctx.scale * (icon_scale_pct as f32 / 100.0).max(0.1);
        let show_background = ctx
            .config
            .get(SHOW_BACKGROUND_KEY)
            .and_then(ConfigValue::as_bool)
            .unwrap_or(false);
        let window_state = window_list_state(ctx);
        let windows = window_state.windows.get();
        let compact = ctx
            .config
            .get("compact")
            .and_then(ConfigValue::as_bool)
            .unwrap_or(ctx.position == DockPosition::Top);
        let resolver = ctx
            .shared
            .get::<WindowIconResolver>()
            .map(|resolver| resolver.0.get());

        if ctx.position.is_vertical() {
            return side_window_list(
                windows,
                window_state.activate,
                window_state.refresh,
                scale,
                show_background,
                resolver.as_ref(),
            );
        }
        window_list_widget(
            windows,
            window_state.activate,
            window_state.refresh,
            scale,
            show_background,
            compact,
            resolver.as_ref(),
        )
    }
}

fn window_list_state(ctx: &IslandRenderContext) -> WindowListState {
    ctx.shared.get::<WindowListState>().unwrap_or_default()
}

fn window_list_widget(
    windows: Vec<OpenWindow>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
    scale: f32,
    show_background: bool,
    compact: bool,
    resolver: Option<&IconResolver>,
) -> BoxedWidget {
    let count = windows.len().min(MAX_TASKS);
    let mut strip = Flex::row().gap(6.0 * scale).align(Align::Center);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        let icon = task_icon(&entry, scale, compact, resolver);
        strip = strip.child(Box::new(
            RawButton::new(window_style(scale, show_background, compact), move || {
                activate(id.clone());
                refresh();
            })
            .child(icon),
        ));
    }
    let width = count as f32 * if compact { 46.0 } else { TASK_SIZE } * scale
        + count.saturating_sub(1) as f32 * 6.0 * scale;
    let height = if compact { 32.0 } else { TASK_HEIGHT } * scale;
    Box::new(
        Flex::row()
            .size(width, height)
            .align(Align::Center)
            .child(Box::new(strip)),
    )
}

fn side_window_list(
    windows: Vec<OpenWindow>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
    scale: f32,
    show_background: bool,
    resolver: Option<&IconResolver>,
) -> BoxedWidget {
    let mut column = Flex::column()
        .padding(4.0 * scale)
        .gap(6.0 * scale)
        .align(Align::Center)
        .background(shell_island())
        .border(shell_border(), 1.0)
        .corner_radius(16.0 * scale);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        column = column.child(Box::new(
            RawButton::new(window_style(scale, show_background, false), move || {
                activate(id.clone());
                refresh();
            })
            .child(task_icon(&entry, scale, false, resolver)),
        ));
    }
    Box::new(column)
}

fn side_icon(name: &'static str, scale: f32) -> BoxedWidget {
    Box::new(jsx! {
        <Flex size={(SIDE_ITEM_SIZE * scale, SIDE_ITEM_SIZE * scale)} align={Align::Center} justify={Justify::Center}>
            {pixel_icon(name, 19.0 * scale, shell_text())}
        </Flex>
    })
}

fn side_icon_button_styled(
    name: &'static str,
    on_click: Rc<dyn Fn(Point)>,
    selected: bool,
    scale: f32,
) -> BoxedWidget {
    Box::new(
        RawButton::new(
            side_button_style(
                if selected {
                    shell_selected()
                } else {
                    shell_control()
                },
                scale,
            ),
            || {},
        )
        .with_click_position(move |point| on_click(point))
        .child(side_icon(name, scale)),
    )
}

fn task_icon(
    window: &OpenWindow,
    scale: f32,
    compact: bool,
    resolver: Option<&IconResolver>,
) -> BoxedWidget {
    if compact {
        return Box::new(
            Flex::row()
                .size(46.0 * scale, 32.0 * scale)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(app_icon(window, 24.0 * scale, resolver)),
        );
    }
    let indicator_color = if window.active {
        shell_text()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    let task_size = TASK_SIZE * scale;
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(task_size, TASK_HEIGHT * scale)} gap={3.0 * scale} align={Align::Center} justify={Justify::Center}>
            {app_icon(window, task_size, resolver)}
            <Flex size={(2.5 * scale, 2.5 * scale)} background={indicator_color} corner_radius={1.25 * scale} />
        </Flex>
    })
}

fn app_icon(window: &OpenWindow, size: f32, resolver: Option<&IconResolver>) -> BoxedWidget {
    let data = if window.app_name.is_empty() {
        None
    } else {
        resolver.and_then(|resolver| {
            resolver
                .resolve(IconRequest {
                    names: vec![
                        window.app_name.clone(),
                        window.app_name.to_ascii_lowercase(),
                    ],
                    size: size.ceil() as u32,
                    scale: 1,
                })
                .and_then(|path| cached_icon_data(&path))
        })
    }
    .or_else(|| window.icon_path.as_ref().and_then(cached_icon_data))
    .unwrap_or_else(|| fallback_icon_data(window));
    Box::new(
        Image::new(data)
            .layout(LayoutStyle {
                size: fixed(size, size),
                flex_shrink: 0.0,
                ..Default::default()
            })
            .fit(ImageFit::Contain)
            .corner_radius(size * 0.27)
            .box_shadow(design::card_shadow()),
    )
}

/// Retains decoded window icons across rebuilds (clock ticks, playback
/// updates, ...) instead of redecoding every icon each time. Failed decodes
/// are cached too, avoiding a retry on every rebuild.
fn cached_icon_data(path: &PathBuf) -> Option<ImageData> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<ImageData>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(data) = cache.lock().ok().and_then(|cache| cache.get(path).cloned()) {
        return data;
    }

    let data = if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        decode_svg_icon(path)
    } else {
        ImageData::from_path(path).ok()
    };
    if let Ok(mut cache) = cache.lock() {
        cache.insert(path.clone(), data.clone());
    }
    data
}

fn decode_svg_icon(path: &Path) -> Option<ImageData> {
    let source = std::fs::read(path).ok()?;
    let tree = resvg::usvg::Tree::from_data(&source, &resvg::usvg::Options::default()).ok()?;
    let source_size = tree.size();
    let scale = 64.0 / source_size.width().max(source_size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    ImageData::from_bytes(&pixmap.encode_png().ok()?).ok()
}

fn fallback_icon_kind(window: &OpenWindow) -> &'static str {
    let name = format!("{} {}", window.app_name, window.title).to_lowercase();
    if [
        "settings",
        "systemsettings",
        "preferencias",
        "configuración",
    ]
    .iter()
    .any(|key| name.contains(key))
    {
        "settings"
    } else if ["dolphin", "nautilus", "thunar", "files", "archivos"]
        .iter()
        .any(|key| name.contains(key))
    {
        "files"
    } else if ["spotify", "music", "rhythmbox", "elisa", "música"]
        .iter()
        .any(|key| name.contains(key))
    {
        "music"
    } else if ["notes", "notas", "gedit", "kate", "text editor"]
        .iter()
        .any(|key| name.contains(key))
    {
        "notes"
    } else if ["calendar", "calendario", "korganizer"]
        .iter()
        .any(|key| name.contains(key))
    {
        "calendar"
    } else if ["weather", "clima"].iter().any(|key| name.contains(key)) {
        "weather"
    } else {
        "generic"
    }
}

fn fallback_icon_data(window: &OpenWindow) -> ImageData {
    thread_local! {
        static ICONS: std::cell::RefCell<HashMap<&'static str, ImageData>> = std::cell::RefCell::new(HashMap::new());
    }
    let kind = fallback_icon_kind(window);
    ICONS.with(|icons| {
        icons
            .borrow_mut()
            .entry(kind)
            .or_insert_with(|| {
                let bytes: &[u8] = match kind {
                    "settings" => include_bytes!("../../../assets/icons/apps/app-settings.svg"),
                    "files" => include_bytes!("../../../assets/icons/apps/app-files.svg"),
                    "music" => include_bytes!("../../../assets/icons/apps/app-music.svg"),
                    "notes" => include_bytes!("../../../assets/icons/apps/app-notes.svg"),
                    "calendar" => include_bytes!("../../../assets/icons/apps/app-calendar.svg"),
                    "weather" => include_bytes!("../../../assets/icons/apps/app-weather.svg"),
                    _ => include_bytes!("../../../assets/icons/apps/app-generic.svg"),
                };
                ImageData::from_svg(bytes, SvgSize::Max(128))
                    .expect("bundled application icon must decode")
            })
            .clone()
    })
}

fn square_style(scale: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE * scale, TASK_HEIGHT * scale),
            ..Default::default()
        })
        .corner_radius(10.0 * scale)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn side_button_style(background: Color, scale: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(SIDE_ITEM_SIZE * scale, SIDE_ITEM_SIZE * scale),
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0 * scale)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_text()))
}

fn window_style(scale: f32, show_background: bool, compact: bool) -> Style {
    if compact {
        return island_style(46.0 * scale, 32.0 * scale, scale);
    }
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE * scale, TASK_HEIGHT * scale),
            ..Default::default()
        })
        .background(if show_background {
            shell_island()
        } else {
            Color::rgba(0, 0, 0, 0)
        })
        .corner_radius(10.0 * scale)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

#[cfg(test)]
mod tests {
    use super::{fallback_icon_data, fallback_icon_kind};
    use coconut_api::desktop::OpenWindow;

    #[test]
    fn unnamed_windows_use_the_title_for_a_decodable_icon() {
        for (title, kind) in [
            ("Coconut Settings", "settings"),
            ("Files", "files"),
            ("Music", "music"),
            ("Notes", "notes"),
            ("Calendar", "calendar"),
            ("Weather", "weather"),
            ("Untitled", "generic"),
        ] {
            let window = OpenWindow {
                id: "1".into(),
                app_name: String::new(),
                title: title.into(),
                icon_path: None,
                active: true,
            };
            assert_eq!(fallback_icon_kind(&window), kind);
            let data = fallback_icon_data(&window);
            assert_eq!(data.width(), 128);
            assert_eq!(data.height(), 128);
        }
    }

    #[test]
    fn svg_window_icons_are_decoded() {
        use super::decode_svg_icon;
        use std::path::Path;
        let icon = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/icons/settings/brightness.svg");
        assert!(decode_svg_icon(&icon).is_some());
    }
}
