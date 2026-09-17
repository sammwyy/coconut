use coconut_api::desktop::OpenWindow;
use coconut_plugin_kit::chrome::{
    shell_accent, shell_border, shell_control, shell_control_hover, shell_panel, shell_selected,
    shell_text,
};
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
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

/// The live open-window list plus the window-management callbacks the
/// launcher's task strip needs to draw and interact with open windows.
///
/// [`coconut_plugin_kit::SharedState`] is keyed by `TypeId` alone, so this is
/// a dedicated newtype rather than storing a bare `Signal<Vec<OpenWindow>>`/
/// `Rc<dyn Fn()>`/`Rc<dyn Fn(String)>` directly — those bare shapes are
/// common enough (other plugins have their own, unrelated callbacks of the
/// exact same type) that storing them directly would silently collide with
/// whichever one was inserted last.
///
/// **Contract for `apps/shell/src/lib.rs` (Phase 4d):** insert one of these
/// into the app-wide `SharedState` at startup, wired to the same desktop
/// integration that used to feed `BarActions::{refresh_windows,
/// activate_window}` and the bar's own `windows: Signal<Vec<OpenWindow>>`
/// local (`apps/shell/src/lib.rs:57-58,104-110`): `windows` should be the
/// same signal kept live by `schedule_window_events`, `refresh` should
/// re-fetch from the desktop integration and update it, and `activate`
/// should call the desktop integration's `activate_window`. If nothing is
/// ever inserted, [`AppLauncherIsland`] falls back to an empty window list
/// with no-op `refresh`/`activate` callbacks — silently, since `build` runs
/// on every dock rebuild and must never itself spam the log.
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

/// Whether the app drawer panel is currently open, used only to highlight
/// the launcher's own drawer button. A dedicated newtype for the same
/// `TypeId`-collision reason as [`WindowListState`] — a bare `Signal<bool>`
/// is far too generic a shape to own safely in a shared, type-keyed bag.
///
/// **Contract for `apps/shell/src/lib.rs` (Phase 4d):** insert one of these,
/// toggled whenever the `"app_drawer"` panel opens/closes. Ported as-is from
/// `apps/shell/src/bar/mod.rs`'s `launcher_open: Signal<bool>` parameter —
/// note that in the pre-refactor shell this signal was created
/// (`apps/shell/src/lib.rs:60`) but never actually set to `true` anywhere in
/// the codebase, so the highlight never actually lit up. That pre-existing
/// gap is preserved, not fixed, by this migration; Phase 4d may want to wire
/// it up for real while it builds `PanelHost`.
#[derive(Clone)]
pub struct LauncherOpenSignal(pub Signal<bool>);

impl Default for LauncherOpenSignal {
    fn default() -> Self {
        Self(Signal::new(false))
    }
}

/// The dock's app-launcher island: the button that opens the app drawer,
/// plus — in both the horizontal and vertical/side dock orientations — the
/// strip of currently open windows/tasks. Moved from
/// `apps/shell/src/bar/mod.rs`'s `app_launcher_widget`/
/// `side_app_launcher_widget` and their shared helpers (Phase 4b of the
/// dock/island/panel refactor,
/// `~/.claude/plans/luminous-prancing-wombat.md`). Opens the paired
/// `coconut-plugin-app-drawer` panel by id, `"app_drawer"`.
///
/// See [`WindowListState`] and [`LauncherOpenSignal`] for the exact
/// `SharedState` entries this island expects to find pre-populated.
pub struct AppLauncherIsland;

impl Island for AppLauncherIsland {
    fn id(&self) -> &'static str {
        "app_launcher"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
        let window_state = ctx.shared.get::<WindowListState>().unwrap_or_default();
        let launcher_open = ctx
            .shared
            .get::<LauncherOpenSignal>()
            .unwrap_or_default()
            .0
            .get();
        let windows = window_state.windows.get();
        let open_panel = ctx.open_panel.clone();
        let drawer_click: Rc<dyn Fn(Point)> = Rc::new(move |at| open_panel("app_drawer", at));

        if ctx.position.is_vertical() {
            return side_app_launcher_widget(
                windows,
                window_state.activate,
                window_state.refresh,
                drawer_click,
                launcher_open,
            );
        }

        let launcher_background = if launcher_open {
            shell_selected()
        } else {
            shell_control()
        };
        let (tasks, task_count) = window_strip(windows, window_state.activate, window_state.refresh);
        let width = dock_width(task_count);
        let drawer_button: BoxedWidget = Box::new(
            RawButton::new(square_style(launcher_background), || {})
                .with_click_position(move |point| drawer_click(point))
                .child(Box::new(jsx! {
                    <Flex size={(32.0, 32.0)} align={Align::Center} justify={Justify::Center}>{pixel_icon("appgrid", 21.0, shell_text())}</Flex>
                })),
        );
        app_launcher_widget(width, drawer_button, tasks)
    }
}

/// The horizontal dock's launcher island: the drawer button followed by the
/// open-window task strip, in one rounded container. Moved verbatim from
/// `apps/shell/src/bar/mod.rs::app_launcher_widget`.
fn app_launcher_widget(dock_width: f32, drawer_button: BoxedWidget, tasks: BoxedWidget) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(dock_width, 40.0)} padding={4.0} gap={6.0} align={Align::Center} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={12.0}>
            {drawer_button}
            {tasks}
        </Flex>
    })
}

/// Side bars intentionally use icon-sized controls, and the launcher also
/// serves as the task switcher there (rather than turning every configured
/// side-bar section into its own separate task list). Moved verbatim from
/// `apps/shell/src/bar/mod.rs::side_app_launcher_widget`.
fn side_app_launcher_widget(
    windows: Vec<OpenWindow>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
    open_drawer: Rc<dyn Fn(Point)>,
    selected: bool,
) -> BoxedWidget {
    let drawer = side_icon_button_styled("appgrid", open_drawer, selected);
    let mut island = Flex::column()
        .padding(4.0)
        .gap(6.0)
        .align(Align::Center)
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(12.0)
        .child(drawer);

    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let active = entry.active;
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        island = island.child(Box::new(
            RawButton::new(window_style(active), move || {
                activate(id.clone());
                refresh();
            })
            .child(task_icon(&entry, active)),
        ));
    }

    Box::new(island)
}

fn side_icon(name: &'static str) -> BoxedWidget {
    Box::new(jsx! {
        <Flex size={(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE)} align={Align::Center} justify={Justify::Center}>
            {pixel_icon(name, 19.0, shell_text())}
        </Flex>
    })
}

fn side_icon_button_styled(name: &'static str, on_click: Rc<dyn Fn(Point)>, selected: bool) -> BoxedWidget {
    Box::new(
        RawButton::new(
            side_button_style(if selected { shell_selected() } else { shell_control() }),
            || {},
        )
        .with_click_position(move |point| on_click(point))
        .child(side_icon(name)),
    )
}

/// Builds the open-window task strip and reports how many tasks it holds
/// (capped at [`MAX_TASKS`]), so the caller can size the launcher island
/// around it. Moved verbatim from `apps/shell/src/bar/mod.rs::window_strip`.
fn window_strip(
    windows: Vec<OpenWindow>,
    activate_window: Rc<dyn Fn(String)>,
    refresh_windows: Rc<dyn Fn()>,
) -> (BoxedWidget, usize) {
    if windows.is_empty() {
        return (Box::new(Flex::row()), 0);
    }

    let count = windows.len().min(MAX_TASKS);
    let mut strip = Flex::row().gap(6.0).align(Align::Center);
    for entry in windows.into_iter().take(MAX_TASKS) {
        let id = entry.id.clone();
        let selected = entry.active;
        let activate = activate_window.clone();
        let refresh = refresh_windows.clone();
        let icon = task_icon(&entry, selected);
        let item: BoxedWidget = Box::new(
            RawButton::new(window_style(selected), move || {
                activate(id.clone());
                refresh();
            })
            .child(icon),
        );
        strip = strip.child(item);
    }
    (Box::new(strip), count)
}

/// The horizontal launcher island's total width for a given visible task
/// count. Moved verbatim from `apps/shell/src/bar/mod.rs::dock_width`.
fn dock_width(tasks: usize) -> f32 {
    if tasks == 0 {
        46.0
    } else {
        46.0 + tasks as f32 * TASK_SIZE + (tasks - 1) as f32 * 6.0
    }
}

fn task_icon(window: &OpenWindow, active: bool) -> BoxedWidget {
    let indicator_width = if active { 10.0 } else { 3.0 };
    let indicator_color = if active {
        shell_accent()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TASK_SIZE, TASK_SIZE)} gap={1.0} align={Align::Center} justify={Justify::Center}>
            <Flex size={(TASK_ICON_SIZE, TASK_ICON_SIZE)} align={Align::Center} justify={Justify::Center}>{app_icon(window)}</Flex>
            <Flex size={(indicator_width, 3.0)} background={indicator_color} corner_radius={1.5} />
        </Flex>
    })
}

fn app_icon(window: &OpenWindow) -> BoxedWidget {
    if let Some(data) = window.icon_path.as_ref().and_then(cached_icon_data) {
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
        <Flex size={(TASK_ICON_SIZE, TASK_ICON_SIZE)} align={Align::Center} justify={Justify::Center} background={shell_control()} corner_radius={7.0}>
            <RawText color={shell_text()} font_size={11.0} align={TextAlign::Center}>{fallback_icon(&window.app_name)}</RawText>
        </Flex>
    })
}

/// The dock widget tree is rebuilt for clock and playback updates. Retain
/// decoded window icons so those updates never reopen and decode every icon.
/// Failed decodes are cached as well, avoiding a retry on every rebuild.
/// Moved verbatim from `apps/shell/src/bar/mod.rs::cached_icon_data`.
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

/// Moved verbatim from `apps/shell/src/bar/mod.rs::decode_svg_icon`.
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

/// Moved verbatim from `apps/shell/src/bar/mod.rs::fallback_icon`.
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
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_text()))
}

fn side_button_style(background: Color) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(SIDE_ITEM_SIZE, SIDE_ITEM_SIZE),
            ..Default::default()
        })
        .background(background)
        .corner_radius(8.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_text()))
}

fn window_style(active: bool) -> Style {
    let background = if active {
        shell_selected()
    } else {
        Color::rgba(0, 0, 0, 0)
    };
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TASK_SIZE, TASK_SIZE),
            ..Default::default()
        })
        .background(background)
        .corner_radius(10.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

#[cfg(test)]
mod tests {
    use super::{decode_svg_icon, dock_width, fallback_icon};
    use std::path::Path;

    #[test]
    fn dock_fits_visible_tasks() {
        assert_eq!(dock_width(0), 46.0);
        assert_eq!(dock_width(4), 192.0);
    }

    #[test]
    fn fallback_uses_the_app_initial() {
        assert_eq!(fallback_icon("firefox"), "F");
    }

    #[test]
    fn svg_window_icons_are_decoded() {
        let icon =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/brightness.svg");
        assert!(decode_svg_icon(&icon).is_some());
    }
}
