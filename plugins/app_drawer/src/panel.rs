//! `app_drawer`'s `Panel` implementation, moved from
//! `apps/shell/src/panels/app_drawer/mod.rs` (Phase 4b of the dock/island/
//! panel refactor, `~/.claude/plans/luminous-prancing-wombat.md`). Opened by
//! the paired `coconut-plugin-app-launcher` crate's `AppLauncherIsland` via
//! `(ctx.open_panel)("app_drawer", at)`.
//!
//! **`AppCatalog`/`AppHandle`/icon-theme design note:** `AppCatalog::start_loading`
//! needs a live `creamui_render::AppHandle` (to run the filesystem scan and
//! icon resolution as background work) and the current icon-theme string —
//! neither of which [`PanelRenderContext`] carries (a popup's `build` only
//! gets `size`/`shared`/`scratch`/`open_panel`/`close`). Both are available,
//! however, at [`crate::AppDrawerPlugin::panels`] time: `PluginInitContext`
//! carries `app: AppHandle`, and the icon theme is read directly from
//! `coconut_core::ShellConfig::load()` there (mirroring how
//! `apps/shell/src/lib.rs` read `initial_config.appearance.icon_theme`
//! before `AppBuilder::run` even started). So the catalog is built and its
//! initial scan kicked off exactly once, at plugin-init time, then stashed
//! in the app-wide `SharedState` (not `scratch`, since it must survive
//! across popup opens/closes) so `Panel::build` can fetch the same instance
//! back out by type. This also gives Phase 4d's config-reload handling
//! (re-running `start_loading` when `appearance.icon_theme` changes, as
//! `apps/shell/src/lib.rs`'s `schedule_runtime_events` used to) a way to
//! reach the very same `AppCatalog` — `ctx.shared.get::<AppCatalog>()` —
//! without this crate needing to expose anything beyond the `Plugin`/`Panel`
//! traits.
//!
//! `DrawerState` (the search query, selected category, and scroll
//! positions) is exactly the kind of fresh-per-open UI state
//! `PanelRenderContext::scratch` exists for, so it's seeded there via
//! `get_or_insert_with` instead.

use coconut_plugin_kit::chrome::{
    shell_accent, shell_border, shell_card, shell_control, shell_control_hover, shell_muted,
    shell_panel, shell_selected, shell_text, CARD_RADIUS, ISLAND_RADIUS,
};
use coconut_plugin_kit::{build_icon_index, load_icon, resolve_icon, xdg_data_directories};
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_widgets::layout::{fixed, padding_xy, Align, Flex, Justify, Wrap};
use creamui_widgets::RawButton;
use creamui_widgets::{
    Icon, ScrollController, ScrollView, SidebarItem, Symbol, TabColors, TextController, TextInput,
};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::thread;

const WIDTH: u32 = 760;
const HEIGHT: u32 = 520;
const TILE: f32 = 104.0;
const TILE_H: f32 = 110.0;

#[derive(Clone)]
pub struct AppCatalog {
    entries: Signal<Vec<AppEntry>>,
}

#[derive(Clone)]
pub struct DrawerState {
    query: TextController,
    category: Signal<String>,
    scroll: ScrollController,
    category_scroll: ScrollController,
}

impl Default for DrawerState {
    fn default() -> Self {
        let query = TextController::default();
        let scroll = ScrollController::default();
        let search_scroll = scroll.clone();
        query.on_change(move |_, next| {
            search_scroll.set(0.0);
            Some(next.to_owned())
        });
        Self {
            query,
            category: Signal::new("All".into()),
            scroll,
            category_scroll: ScrollController::default(),
        }
    }
}

#[derive(Clone)]
struct AppEntry {
    name: String,
    exec: Vec<String>,
    terminal: bool,
    category: String,
    icon_name: Option<String>,
    icon: Option<ImageData>,
}

impl AppCatalog {
    pub fn new() -> Self {
        Self {
            entries: Signal::new(Vec::new()),
        }
    }

    pub fn start_loading(&self, app: &AppHandle, icon_theme: String) {
        let after_scan = self.entries.clone();
        let icon_app = app.clone();
        app.spawn_background(load_apps, move |apps: Vec<AppEntry>| {
            after_scan.set(apps.clone());
            let after_icons = after_scan.clone();
            icon_app.spawn_background(
                move || resolve_icons(apps, &icon_theme),
                move |resolved| after_icons.set(resolved),
            );
        });
    }

    fn apps(&self) -> Vec<AppEntry> {
        self.entries.get()
    }
}

impl Default for AppCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// The app drawer panel: a searchable, categorized grid of installed
/// applications. See this module's doc comment for how `AppCatalog` is
/// built and shared, and where `DrawerState` comes from.
pub struct AppDrawerPanel;

impl Panel for AppDrawerPanel {
    fn id(&self) -> &'static str {
        "app_drawer"
    }

    fn title(&self) -> &'static str {
        "Coconut App Drawer"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        let catalog = ctx.shared.get::<AppCatalog>().unwrap_or_else(|| {
            eprintln!(
                "app_drawer: SharedState is missing AppCatalog (see AppDrawerPlugin::panels); showing an empty drawer"
            );
            AppCatalog::new()
        });
        let state = ctx.scratch.get_or_insert_with(DrawerState::default);
        build_drawer(catalog, state, ctx.close.clone())
    }
}

fn build_drawer(catalog: AppCatalog, state: DrawerState, on_launch: Rc<dyn Fn()>) -> BoxedWidget {
    let query = state.query.value();
    let selected_category = state.category.get();
    let catalog_apps = catalog.apps();
    let loaded = !catalog_apps.is_empty();
    let categories = categories(&catalog_apps);
    let apps: Vec<_> = catalog_apps
        .into_iter()
        .filter(|app| app_matches(app, &query, &selected_category))
        .collect();
    let content: BoxedWidget = if apps.is_empty() {
        let message = if loaded { "No matches" } else { "Scanning" };
        Box::new(jsx! {
            <Flex grow={1.0} size={(580.0, 396.0)} align={Align::Center} justify={Justify::Center}>
                <RawText color={shell_muted()} font_size={16.0}>{message}</RawText>
            </Flex>
        })
    } else {
        let mut grid = Flex::row()
            .gap(8.0)
            .justify(Justify::Start)
            .wrap(Wrap::Wrap)
            .padding(6.0);
        for app in apps {
            let launch = app.clone();
            let close = on_launch.clone();
            grid = grid.child(Box::new(
                RawButton::new(app_style(), move || {
                    launch_app(&launch);
                    close();
                })
                .child(app_card(&app)),
            ));
        }
        Box::new(grid)
    };

    let search = Box::new(
        TextInput::controlled_with_style(search_style(), &state.query)
            .placeholder("Search apps")
            .background(shell_panel())
            .border(shell_border(), 1.0)
            .corner_radius(ISLAND_RADIUS)
            .layout(search_style()),
    ) as BoxedWidget;
    let sidebar = category_sidebar(state.clone(), &categories);
    let settings = settings_program()
        .map(settings_button)
        .unwrap_or_else(|| Box::new(Flex::row().size(34.0, 34.0)) as BoxedWidget);
    let scroll = Box::new(
        ScrollView::controlled(scroll_style(), state.scroll.clone())
            .background(shell_panel())
            .border(shell_border(), 1.0)
            .corner_radius(ISLAND_RADIUS)
            .child(content),
    ) as BoxedWidget;
    let title = user_name();

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={14.0} gap={10.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
            <Flex direction={FlexDirection::Row} align={Align::Center}>
                <RawText color={shell_text()} font_size={18.0}>{title}</RawText>
                <Flex grow={1.0} />
                {settings}
            </Flex>
            <Flex direction={FlexDirection::Row} grow={1.0} gap={10.0}>
                {sidebar}
                <Flex direction={FlexDirection::Column} grow={1.0} gap={10.0}>
                    {search}
                    {scroll}
                </Flex>
            </Flex>
        </Flex>
    })
}

fn app_card(app: &AppEntry) -> BoxedWidget {
    let icon = app
        .icon
        .clone()
        .map(|data| {
            Box::new(
                Image::new(data)
                    .layout(LayoutStyle {
                        size: fixed(48.0, 48.0),
                        ..Default::default()
                    })
                    .fit(ImageFit::Contain),
            ) as BoxedWidget
        })
        .unwrap_or_else(|| {
            Box::new(jsx! {
                <Flex size={(48.0, 48.0)} align={Align::Center} justify={Justify::Center} background={shell_accent()} corner_radius={14.0}>
                    <RawText color={shell_text()} font_size={20.0}>{app.name.chars().next().unwrap_or('•').to_uppercase().to_string()}</RawText>
                </Flex>
            })
        });
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE, TILE_H)} padding={10.0} gap={8.0} align={Align::Center} justify={Justify::Center}>
            {icon}
            <RawText color={shell_muted()} font_size={11.0}>{short_name(&app.name)}</RawText>
        </Flex>
    })
}

fn category_sidebar(state: DrawerState, categories: &[String]) -> BoxedWidget {
    let selected = state.category.get();
    let colors = TabColors::sidebar();
    let mut category_list = Flex::column().gap(4.0);
    for label in categories {
        let category = state.category.clone();
        let scroll = state.scroll.clone();
        let label = label.clone();
        category_list = category_list.child(Box::new(SidebarItem::new(
            colors,
            category_item_style(),
            label.clone(),
            selected == label,
            move || {
                category.set(label.clone());
                scroll.set(0.0);
            },
        )) as BoxedWidget);
    }
    let list = Box::new(
        ScrollView::controlled(category_scroll_style(), state.category_scroll.clone())
            .child(Box::new(category_list)),
    ) as BoxedWidget;
    let mut sidebar = Flex::column()
        .width(142.0)
        .full_height()
        .padding(6.0)
        .gap(4.0);
    sidebar = sidebar.child(list);
    Box::new(sidebar)
}

fn categories(apps: &[AppEntry]) -> Vec<String> {
    const ORDER: [&str; 13] = [
        "All",
        "Multimedia",
        "Internet",
        "Graphics",
        "Office",
        "Development",
        "Education",
        "Science",
        "Games",
        "Accessories",
        "Settings",
        "System Tools",
        "Other",
    ];
    let present: BTreeSet<_> = apps.iter().map(|app| app.category.as_str()).collect();
    ORDER
        .into_iter()
        .filter(|category| *category == "All" || present.contains(category))
        .map(str::to_owned)
        .collect()
}

fn app_matches(app: &AppEntry, query: &str, category: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    let query_matches = query.is_empty()
        || app.name.to_ascii_lowercase().contains(&query)
        || app.category.to_ascii_lowercase().contains(&query);
    query_matches && (category == "All" || app.category == category)
}

fn launch_app(app: &AppEntry) {
    let Some((program, args)) = app
        .exec
        .split_first()
        .map(|(program, args)| (program.clone(), args.to_vec()))
    else {
        return;
    };
    let terminal = app.terminal;
    thread::spawn(move || {
        let mut command = if terminal {
            let terminal =
                std::env::var("TERMINAL").unwrap_or_else(|_| "x-terminal-emulator".into());
            let mut command = Command::new(terminal);
            command.arg("-e").arg(program).args(args);
            command
        } else {
            let mut command = Command::new(program);
            command.args(args);
            command
        };
        let _ = crate::process::spawn_detached(&mut command);
    });
}

fn load_apps() -> Vec<AppEntry> {
    #[cfg(target_os = "windows")]
    {
        return load_windows_apps();
    }
    #[cfg(not(target_os = "windows"))]
    load_linux_apps()
}

#[cfg(not(target_os = "windows"))]
fn load_linux_apps() -> Vec<AppEntry> {
    let mut desktop_files = HashMap::new();
    for directory in desktop_directories() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "desktop")
            {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    desktop_files.insert(name.to_owned(), path);
                }
            }
        }
    }
    let mut apps: Vec<_> = desktop_files
        .into_values()
        .filter_map(|path| parse_desktop_entry(&path))
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

#[cfg(target_os = "windows")]
fn load_windows_apps() -> Vec<AppEntry> {
    let script = "Get-StartApps | ForEach-Object { \"$($_.Name)`t$($_.AppID)\" }";
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default();
    let mut apps = output
        .lines()
        .filter_map(|line| {
            let (name, app_id) = line.split_once('\t')?;
            let name = name.trim();
            let app_id = app_id.trim();
            (!name.is_empty() && !app_id.is_empty()).then(|| AppEntry {
                name: name.to_owned(),
                exec: vec!["explorer.exe".into(), format!("shell:AppsFolder\\{app_id}")],
                terminal: false,
                category: "System Tools".into(),
                icon_name: None,
                icon: None,
            })
        })
        .collect::<Vec<_>>();
    apps.sort_by_key(|app| app.name.to_ascii_lowercase());
    apps
}

fn resolve_icons(mut apps: Vec<AppEntry>, icon_theme: &str) -> Vec<AppEntry> {
    let icon_index = build_icon_index(icon_theme);
    for app in &mut apps {
        app.icon = app
            .icon_name
            .as_deref()
            .and_then(|name| resolve_icon(name, &icon_index))
            .and_then(|path| load_icon(&path, 48));
    }
    apps
}

#[cfg(not(target_os = "windows"))]
fn desktop_directories() -> Vec<PathBuf> {
    xdg_data_directories()
        .into_iter()
        .map(|directory| directory.join("applications"))
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn parse_desktop_entry(path: &Path) -> Option<AppEntry> {
    let contents = fs::read_to_string(path).ok()?;
    let mut fields = HashMap::new();
    let mut in_desktop_entry = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_desktop_entry {
            if let Some((key, value)) = line.split_once('=') {
                fields.insert(key, value);
            }
        }
    }
    if fields.get("Type") != Some(&"Application")
        || matches!(fields.get("Hidden"), Some(&"true"))
        || matches!(fields.get("NoDisplay"), Some(&"true"))
    {
        return None;
    }
    let name = fields
        .get("Name[en_US]")
        .or_else(|| fields.get("Name[en]"))
        .or_else(|| fields.get("Name"))?
        .to_string();
    let exec = parse_exec(fields.get("Exec")?);
    (!exec.is_empty()).then_some(AppEntry {
        name,
        exec,
        terminal: fields.get("Terminal") == Some(&"true"),
        category: category_for(fields.get("Categories").copied()),
        icon_name: fields.get("Icon").map(|icon| (*icon).to_owned()),
        icon: None,
    })
}

#[cfg(not(target_os = "windows"))]
fn category_for(categories: Option<&str>) -> String {
    let categories = categories.unwrap_or_default();
    if categories.contains("AudioVideo")
        || categories.contains("Audio;")
        || categories.contains("Video;")
    {
        "Multimedia"
    } else if categories.contains("Network") {
        "Internet"
    } else if categories.contains("Development") {
        "Development"
    } else if categories.contains("Graphics") {
        "Graphics"
    } else if categories.contains("Office") {
        "Office"
    } else if categories.contains("Education") {
        "Education"
    } else if categories.contains("Science") {
        "Science"
    } else if categories.contains("Game") {
        "Games"
    } else if categories.contains("Settings") {
        "Settings"
    } else if categories.contains("System") {
        "System Tools"
    } else if categories.contains("Utility") {
        "Accessories"
    } else {
        "Other"
    }
    .into()
}

#[cfg(not(target_os = "windows"))]
fn parse_exec(value: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                current.push(character);
            }
        } else if character.is_whitespace() && quote.is_none() {
            if !current.is_empty() {
                arguments.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    arguments
        .into_iter()
        .filter(|argument| !argument.starts_with('%'))
        .collect()
}

fn short_name(name: &str) -> String {
    const MAX: usize = 14;
    let mut shortened: String = name.chars().take(MAX).collect();
    if name.chars().count() > MAX {
        shortened.push('…');
    }
    shortened
}

fn app_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TILE, TILE_H),
            ..Default::default()
        })
        .background(shell_panel())
        .corner_radius(14.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn search_style() -> creamui_core::layout::Style {
    LayoutStyle {
        size: fixed(580.0, 42.0),
        ..Default::default()
    }
}

fn scroll_style() -> creamui_core::layout::Style {
    LayoutStyle {
        size: fixed(580.0, 396.0),
        ..Default::default()
    }
}

fn category_scroll_style() -> creamui_core::layout::Style {
    LayoutStyle {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Length(130.0),
            height: creamui_core::layout::Dimension::Auto,
        },
        flex_grow: 1.0,
        ..Default::default()
    }
}

fn category_item_style() -> LayoutStyle {
    padding_xy(
        LayoutStyle {
            size: fixed(130.0, 36.0),
            ..Default::default()
        },
        12.0,
        0.0,
    )
}

fn settings_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(34.0, 34.0),
            ..Default::default()
        })
        .background(shell_control())
        .corner_radius(9.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn settings_button(program: PathBuf) -> BoxedWidget {
    Box::new(
        RawButton::new(settings_button_style(), move || {
            open_settings(program.clone())
        })
        .child(Box::new(Icon::new(Symbol::Sliders, shell_muted()).size(18.0)) as BoxedWidget),
    )
}

fn user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "User".into())
}

fn settings_program() -> Option<PathBuf> {
    let name = format!("coconut-settings{}", std::env::consts::EXE_SUFFIX);
    let beside_shell = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join(&name)));
    beside_shell.filter(|path| path.is_file()).or_else(|| {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join(&name))
                .find(|path| path.is_file())
        })
    })
}

fn open_settings(program: PathBuf) {
    thread::spawn(move || {
        let mut command = Command::new(program);
        let _ = crate::process::spawn_detached(&mut command);
    });
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::parse_exec;

    #[test]
    fn exec_parser_keeps_quoted_arguments_and_drops_desktop_fields() {
        assert_eq!(
            parse_exec("code --profile \"Work Space\" %U"),
            ["code", "--profile", "Work Space"]
        );
    }
}
