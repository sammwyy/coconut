use coconut_api::desktop::OpenWindow;
use coconut_plugin_kit::chrome::{
    shell_accent, shell_border, shell_control, shell_control_hover, shell_panel, shell_selected,
    shell_text,
};
use coconut_plugin_kit::{
    pixel_icon, ConfigField, ConfigValue, Island, IslandRenderContext, NumberRange,
};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Point, StateStyle, Style, Styled, TextAlign};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::Color;
use creamui_widgets::layout::{fixed, Align, Flex, Justify};
use creamui_widgets::RawButton;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

const TASK_SIZE: f32 = 32.0;
const TASK_ICON_SIZE: f32 = 24.0;
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

        let background = if launcher_open {
            shell_selected()
        } else {
            shell_control()
        };
        Box::new(
            RawButton::new(square_style(background, s), || {})
                .with_click_position(move |point| drawer_click(point))
                .child(Box::new(jsx! {
                    <Flex size={(TASK_SIZE * s, TASK_SIZE * s)} align={Align::Center} justify={Justify::Center}>{pixel_icon("appgrid", 21.0 * s, shell_text())}</Flex>
                })),
        )
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
            ConfigField::toggle(SHOW_BACKGROUND_KEY, "Icon background", true),
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
            .unwrap_or(true);
        let window_state = window_list_state(ctx);
        let windows = window_state.windows.get();

        if ctx.position.is_vertical() {
            return side_window_list(
                windows,
                window_state.activate,
                window_state.refresh,
                scale,
                show_background,
            );
        }
        window_list_widget(
            windows,
            window_state.activate,
            window_state.refresh,
            scale,
            show_background,
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
) -> BoxedWidget {
    let count = windows.len().min(MAX_TASKS);
    let mut strip = Flex::row().gap(6.0 * scale).align(Align::Center);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let selected = entry.active;
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        let icon = task_icon(&entry, selected, scale);
        strip = strip.child(Box::new(
            RawButton::new(window_style(selected, scale, show_background), move || {
                activate(id.clone());
                refresh();
            })
            .child(icon),
        ));
    }
    let padding = 4.0 * scale;
    let width =
        count as f32 * TASK_SIZE * scale + count.saturating_sub(1) as f32 * 6.0 * scale + padding * 2.0;
    let height = TASK_SIZE * scale + padding * 2.0;
    Box::new(
        Flex::row()
            .size(width, height)
            .padding(padding)
            .align(Align::Center)
            .background(shell_panel())
            .border(shell_border(), 1.0)
            .corner_radius(12.0 * scale)
            .child(Box::new(strip)),
    )
}

fn side_window_list(
    windows: Vec<OpenWindow>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
    scale: f32,
    show_background: bool,
) -> BoxedWidget {
    let mut column = Flex::column()
        .padding(4.0 * scale)
        .gap(6.0 * scale)
        .align(Align::Center)
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(12.0 * scale);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let active = entry.active;
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        column = column.child(Box::new(
            RawButton::new(window_style(active, scale, show_background), move || {
                activate(id.clone());
                refresh();
            })
            .child(task_icon(&entry, active, scale)),
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
                if selected { shell_selected() } else { shell_control() },
                scale,
            ),
            || {},
        )
        .with_click_position(move |point| on_click(point))
        .child(side_icon(name, scale)),
    )
}

fn task_icon(window: &OpenWindow, active: bool, scale: f32) -> BoxedWidget {
    let indicator_width = (if active { 10.0 } else { 3.0 }) * scale;
    let indicator_color = if active {
        shell_accent()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    let task_size = TASK_SIZE * scale;
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(task_size, task_size)} gap={1.0 * scale} align={Align::Center} justify={Justify::Center}>
            <Flex size={(TASK_ICON_SIZE * scale, TASK_ICON_SIZE * scale)} align={Align::Center} justify={Justify::Center}>{app_icon(window, scale)}</Flex>
            <Flex size={(indicator_width, 3.0 * scale)} background={indicator_color} corner_radius={1.5 * scale} />
        </Flex>
    })
}

fn app_icon(window: &OpenWindow, scale: f32) -> BoxedWidget {
    let icon_size = TASK_ICON_SIZE * scale;
    if let Some(data) = window.icon_path.as_ref().and_then(cached_icon_data) {
        return Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: fixed(22.0 * scale, 22.0 * scale),
                    ..Default::default()
                })
                .fit(ImageFit::Contain),
        );
    }
    Box::new(jsx! {
        <Flex size={(icon_size, icon_size)} align={Align::Center} justify={Justify::Center} background={shell_control()} corner_radius={7.0 * scale}>
            <RawText color={shell_text()} font_size={11.0 * scale} align={TextAlign::Center}>{fallback_icon(&window.app_name)}</RawText>
        </Flex>
    })
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

fn fallback_icon(app_name: &str) -> String {
    app_name
        .chars()
        .next()
        .unwrap_or('•')
        .to_uppercase()
        .to_string()
}

fn square_style(background: Color, scale: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE * scale, TASK_SIZE * scale),
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0 * scale)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_text()))
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

fn window_style(active: bool, scale: f32, show_background: bool) -> Style {
    let background = if active {
        shell_selected()
    } else if show_background {
        shell_control()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE * scale, TASK_SIZE * scale),
            ..Default::default()
        })
        .background(background)
        .corner_radius(10.0 * scale)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

#[cfg(test)]
mod tests {
    use super::fallback_icon;

    #[test]
    fn fallback_uses_the_app_initial() {
        assert_eq!(fallback_icon("firefox"), "F");
    }

    #[test]
    fn svg_window_icons_are_decoded() {
        use super::decode_svg_icon;
        use std::path::Path;
        let icon =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/brightness.svg");
        assert!(decode_svg_icon(&icon).is_some());
    }
}
