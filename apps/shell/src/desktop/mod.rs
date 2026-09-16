use crate::bar::DOCK_HEIGHT;
use crate::icon_theme::{build_icon_index, load_icon, resolve_icon};
use crate::icons::pixel_icon;
use coconut_core::{ClickAction, DesktopIconsConfig, IconShape, ShellConfig, WallpaperMode};
use creamui_core::layout::{
    Dimension, FlexDirection, LengthPercentageAuto, Position, Rect as LayoutRect,
    Size as LayoutSize, Style as LayoutStyle,
};
use creamui_core::{BoxedWidget, Point, Size, StateStyle, Style, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_theme::Color;
use creamui_widgets::layout::{fixed, Align, Justify};
use creamui_widgets::{clamp_to_lines, row_height_family, RawButton, RawView};
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const BACKGROUND: Color = Color::rgb(29, 37, 48);

const GRID_INSET: f32 = 24.0;
const ICON_LABEL: Color = Color::rgb(248, 249, 251);
const ICON_HOVER: Color = Color::rgba(255, 255, 255, 30);
const ICON_PRESSED: Color = Color::rgba(255, 255, 255, 46);
const ICON_BORDER: Color = Color::rgba(255, 255, 255, 34);
const ICON_SELECTED: Color = Color::rgba(120, 170, 255, 56);
const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
const UNSELECTED_LABEL_LINES: usize = 2;
const SELECTED_LABEL_LINES: usize = 4;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// Rasterizes SVG icons bigger than any configured icon size, so scaling
/// down (rather than up) is what `ImageFit::Contain` ends up doing.
const ICON_RASTER_SIZE: u32 = 160;

// Ratios lifted from the original hardcoded layout (58px icon box, 8px
// gaps), used to scale the tile, gaps and label together whenever the icon
// size changes.
const BASE_ICON: f32 = 58.0;
const BASE_SPACING: f32 = 8.0;
const BASE_LABEL_FONT: f32 = 12.0;
const BASE_ROUNDED_RADIUS: f32 = 18.0;

#[derive(Clone, Copy)]
struct IconLayout {
    icon_box: f32,
    glyph: f32,
    corner_radius: f32,
    hover_radius: f32,
    label_font: f32,
    label_line_height: f32,
    outer_padding: f32,
    gap: f32,
    tile_width: f32,
    tile_height: f32,
    grid_cell_x: f32,
    grid_cell_y: f32,
    background: bool,
    background_color: Color,
    click: ClickAction,
}

impl IconLayout {
    fn from_config(config: &DesktopIconsConfig) -> Self {
        let icon_box = config.size.max(24.0);
        let scale = icon_box / BASE_ICON;
        let padding = config.padding.clamp(0.0, icon_box / 2.0 - 4.0).max(0.0);
        let corner_radius = match config.shape {
            IconShape::Square => 0.0,
            IconShape::Rounded => BASE_ROUNDED_RADIUS * scale,
            IconShape::Circle => icon_box / 2.0,
        };
        let label_font = (BASE_LABEL_FONT * scale).max(9.0);
        let outer_padding = BASE_SPACING * scale;
        let gap = BASE_SPACING * scale;
        let label_line_height = row_height_family(label_font, None);
        let tile_width = icon_box + outer_padding * 2.0;
        let tile_height = tile_height_for(
            icon_box,
            outer_padding,
            gap,
            label_line_height,
            UNSELECTED_LABEL_LINES,
        );
        IconLayout {
            icon_box,
            glyph: (icon_box - padding * 2.0).max(8.0),
            corner_radius,
            hover_radius: BASE_ROUNDED_RADIUS * scale,
            label_font,
            label_line_height,
            outer_padding,
            gap,
            tile_width,
            tile_height,
            grid_cell_x: tile_width + config.spacing.max(0.0),
            grid_cell_y: tile_height + config.spacing.max(0.0),
            background: config.background,
            background_color: Color::rgb(
                config.background_color.r,
                config.background_color.g,
                config.background_color.b,
            ),
            click: config.click,
        }
    }

    /// The tile's footprint for `selected` — taller than the resting size,
    /// since a selected icon reserves room for up to
    /// [`SELECTED_LABEL_LINES`] instead of [`UNSELECTED_LABEL_LINES`].
    fn tile_size(&self, selected: bool) -> (f32, f32) {
        if !selected {
            return (self.tile_width, self.tile_height);
        }
        let height = tile_height_for(
            self.icon_box,
            self.outer_padding,
            self.gap,
            self.label_line_height,
            SELECTED_LABEL_LINES,
        );
        (self.tile_width, height)
    }
}

fn tile_height_for(
    icon_box: f32,
    outer_padding: f32,
    gap: f32,
    label_line_height: f32,
    label_lines: usize,
) -> f32 {
    outer_padding * 2.0 + icon_box + gap + label_line_height * label_lines as f32
}

#[derive(Clone)]
pub struct DesktopState {
    positions: Signal<HashMap<usize, Point>>,
    drag_preview: Signal<Option<DragPreview>>,
    entries: Signal<Vec<DesktopEntry>>,
    selected: Signal<Option<usize>>,
    /// The pointer's grab offset within the dragged icon, captured once at
    /// `with_drag_start`. Reconciliation reconstructs `desktop_icon`'s
    /// widget (and whatever it closes over) on every rebuild of the icon
    /// grid, so this can't live in a `Cell` local to that closure — it has
    /// to live on `DesktopState`, which reconciliation never touches, or a
    /// rebuild mid-drag would silently reset it to zero.
    grab: Rc<Cell<Point>>,
    /// The last single click's icon and time, for the same reconciliation
    /// reason as `grab` — used to detect a second click as a double click.
    last_click: Rc<Cell<Option<(usize, Instant)>>>,
}

impl Default for DesktopState {
    fn default() -> Self {
        Self {
            positions: Signal::new(HashMap::new()),
            drag_preview: Signal::new(None),
            entries: Signal::new(Vec::new()),
            selected: Signal::new(None),
            grab: Rc::new(Cell::new(Point::default())),
            last_click: Rc::new(Cell::new(None)),
        }
    }
}

impl DesktopState {
    pub fn start_loading(&self, app: &AppHandle) {
        let entries = self.entries.clone();
        app.spawn_background(load_entries, move |next| entries.set(next));
    }
}

#[derive(Clone)]
enum DesktopTarget {
    Path(PathBuf),
    Launch {
        program: String,
        args: Vec<String>,
        terminal: bool,
    },
}

#[derive(Clone, Copy)]
enum EntryKind {
    Folder,
    File,
    Launcher,
}

#[derive(Clone)]
struct DesktopEntry {
    label: String,
    icon: Option<ImageData>,
    kind: EntryKind,
    target: DesktopTarget,
}

#[derive(Clone)]
struct DragPreview {
    position: Point,
    entry: DesktopEntry,
}

pub fn build(viewport: Size, state: DesktopState, config: Signal<ShellConfig>) -> BoxedWidget {
    let desktop = config.get().desktop;
    let layout = IconLayout::from_config(&desktop.icons);
    Box::new(
        RawView::new(
            Style::new()
                .layout(LayoutStyle {
                    size: fixed(viewport.width, viewport.height),
                    ..Default::default()
                })
                .background(BACKGROUND),
        )
        .with_children(vec![
            wallpaper_layer(desktop),
            widget_layer(),
            icon_layer(viewport, state, layout),
        ]),
    )
}

pub fn build_drag_overlay(
    viewport: Size,
    state: DesktopState,
    config: Signal<ShellConfig>,
) -> BoxedWidget {
    let layout = IconLayout::from_config(&config.get().desktop.icons);
    let children = state
        .drag_preview
        .get()
        .map(|preview| {
            vec![Box::new(
                RawView::new(icon_style(preview.position, &layout, false)).child(icon_content(
                    &preview.entry,
                    &layout,
                    false,
                )),
            ) as BoxedWidget]
        })
        .unwrap_or_default();
    Box::new(
        RawView::new(LayoutStyle {
            size: fixed(viewport.width, viewport.height),
            ..Default::default()
        })
        .with_children(children),
    )
}

fn wallpaper_layer(config: coconut_core::DesktopConfig) -> BoxedWidget {
    if config.wallpaper_mode == WallpaperMode::SolidColor {
        return Box::new(RawView::new(fill_layout()).background(Color::rgb(
            config.solid_color.r,
            config.solid_color.g,
            config.solid_color.b,
        )));
    }
    configured_wallpaper(config.wallpaper)
        .map(|data| {
            Box::new(Image::new(data).layout(fill_layout()).fit(ImageFit::Cover)) as BoxedWidget
        })
        .unwrap_or_else(|| Box::new(RawView::new(fill_layout())))
}

fn widget_layer() -> BoxedWidget {
    Box::new(RawView::new(fill_layout()))
}

fn icon_layer(viewport: Size, state: DesktopState, layout: IconLayout) -> BoxedWidget {
    let positions = state.positions.get();
    let selected = state.selected.get();
    let entries = state.entries.get();
    let children = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let position = positions
                .get(&index)
                .copied()
                .unwrap_or_else(|| initial_position(index, &layout));
            desktop_icon(
                viewport,
                state.clone(),
                index,
                position,
                entry,
                layout,
                selected == Some(index),
            )
        })
        .collect();
    Box::new(
        RawView::new(LayoutStyle {
            size: fixed(viewport.width, viewport.height),
            ..Default::default()
        })
        .with_children(children),
    )
}

#[allow(clippy::too_many_arguments)]
fn desktop_icon(
    viewport: Size,
    state: DesktopState,
    index: usize,
    position: Point,
    entry: DesktopEntry,
    layout: IconLayout,
    selected: bool,
) -> BoxedWidget {
    let click_target = entry.target.clone();
    let click_state = state.clone();
    let click_action = layout.click;
    let grab = state.grab.clone();
    let drag_start = grab.clone();
    let drag_state = state.clone();
    let drag_entry = entry.clone();
    let drop_state = state.clone();
    Box::new(
        RawButton::new(
            icon_style(position, &layout, selected),
            move || match click_action {
                ClickAction::Open => open_target(click_target.clone()),
                ClickAction::Select => {
                    let now = Instant::now();
                    let is_double_click =
                        click_state
                            .last_click
                            .get()
                            .is_some_and(|(last_index, at)| {
                                last_index == index && now.duration_since(at) <= DOUBLE_CLICK
                            });
                    if is_double_click {
                        click_state.last_click.set(None);
                        click_state.selected.set(None);
                        open_target(click_target.clone());
                    } else {
                        click_state.last_click.set(Some((index, now)));
                        click_state.selected.set(Some(index));
                    }
                }
            },
        )
        .with_drag_start(move |local, _| {
            drag_start.set(local);
        })
        .with_drag(move |local, rect| {
            let next = free_icon_position(
                Point {
                    x: rect.x + local.x - grab.get().x,
                    y: rect.y + local.y - grab.get().y,
                },
                viewport,
                &layout,
            );
            drag_state.drag_preview.set(Some(DragPreview {
                position: next,
                entry: drag_entry.clone(),
            }));
        })
        .with_drag_end(move || {
            let dropped = drop_state
                .drag_preview
                .peek()
                .map_or(position, |preview| preview.position);
            let position = icon_position(dropped, viewport, &layout);
            drop_state.drag_preview.set(None);
            if drop_state.positions.peek().get(&index) != Some(&position) {
                drop_state.positions.update(|positions| {
                    positions.insert(index, position);
                });
            }
        })
        .child(icon_content(&entry, &layout, selected)),
    )
}

fn icon_content(entry: &DesktopEntry, layout: &IconLayout, selected: bool) -> BoxedWidget {
    let icon = entry_icon(entry, layout.glyph);
    let fill = if layout.background {
        layout.background_color
    } else {
        TRANSPARENT
    };
    let border = if layout.background {
        (ICON_BORDER, 1.0)
    } else {
        (TRANSPARENT, 0.0)
    };
    let max_lines = if selected {
        SELECTED_LABEL_LINES
    } else {
        UNSELECTED_LABEL_LINES
    };
    let label = clamp_to_lines(
        &entry.label,
        layout.label_font,
        layout.icon_box,
        None,
        max_lines,
    );
    let (tile_width, tile_height) = layout.tile_size(selected);
    let selection_tint = if selected { ICON_SELECTED } else { TRANSPARENT };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(tile_width, tile_height)} padding={layout.outer_padding} gap={layout.gap} align={Align::Center} justify={Justify::Start} background={selection_tint} corner_radius={layout.hover_radius}>
            <Flex size={(layout.icon_box, layout.icon_box)} align={Align::Center} justify={Justify::Center} background={fill} border={border} corner_radius={layout.corner_radius}>
                {icon}
            </Flex>
            <RawText color={ICON_LABEL} font_size={layout.label_font} width={layout.icon_box}>{label}</RawText>
        </Flex>
    })
}

fn free_icon_position(position: Point, viewport: Size, layout: &IconLayout) -> Point {
    Point {
        x: position.x.clamp(
            GRID_INSET,
            (viewport.width - layout.tile_width - GRID_INSET).max(GRID_INSET),
        ),
        y: position.y.clamp(
            GRID_INSET,
            (viewport.height - layout.tile_height - DOCK_HEIGHT as f32 - GRID_INSET)
                .max(GRID_INSET),
        ),
    }
}

fn entry_icon(entry: &DesktopEntry, size: f32) -> BoxedWidget {
    if let Some(data) = entry.icon.clone() {
        return Box::new(
            Image::new(data)
                .layout(LayoutStyle {
                    size: fixed(size, size),
                    ..Default::default()
                })
                .fit(ImageFit::Contain),
        );
    }
    pixel_icon(
        match entry.kind {
            EntryKind::Folder => "desktop-folder",
            EntryKind::File => "desktop-file",
            EntryKind::Launcher => "appgrid",
        },
        size,
        ICON_LABEL,
    )
}

fn configured_wallpaper(wallpaper: Option<PathBuf>) -> Option<ImageData> {
    wallpaper
        .or_else(|| std::env::var_os("COCONUT_WALLPAPER").map(PathBuf::from))
        .filter(|path| path.is_file())
        .and_then(|path| ImageData::from_path(path).ok())
}

fn load_entries() -> Vec<DesktopEntry> {
    let icon_index = build_icon_index();
    let mut entries: Vec<_> = fs::read_dir(desktop_directory())
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| desktop_entry(entry.path(), &icon_index))
        .collect();
    entries.sort_by_key(|entry| entry.label.to_ascii_lowercase());
    entries
}

fn desktop_entry(path: PathBuf, icon_index: &HashMap<String, PathBuf>) -> Option<DesktopEntry> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    if path.is_dir() {
        return Some(DesktopEntry {
            label: name.to_owned(),
            icon: None,
            kind: EntryKind::Folder,
            target: DesktopTarget::Path(path),
        });
    }
    if path
        .extension()
        .is_some_and(|extension| extension == "desktop")
    {
        return launcher_entry(&path, icon_index);
    }
    Some(DesktopEntry {
        label: name.to_owned(),
        icon: None,
        kind: EntryKind::File,
        target: DesktopTarget::Path(path),
    })
}

fn launcher_entry(path: &Path, icon_index: &HashMap<String, PathBuf>) -> Option<DesktopEntry> {
    let contents = fs::read_to_string(path).ok()?;
    let mut fields = HashMap::new();
    let mut in_desktop_entry = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
        } else if in_desktop_entry {
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
    let args = parse_exec(fields.get("Exec")?);
    let (program, args) = args.split_first()?;
    let icon = fields
        .get("Icon")
        .and_then(|name| resolve_icon(name, icon_index))
        .and_then(|path| load_icon(&path, ICON_RASTER_SIZE));
    Some(DesktopEntry {
        label: fields
            .get("Name[en_US]")
            .or_else(|| fields.get("Name[en]"))
            .or_else(|| fields.get("Name"))?
            .to_string(),
        icon,
        kind: EntryKind::Launcher,
        target: DesktopTarget::Launch {
            program: program.clone(),
            args: args.to_vec(),
            terminal: fields.get("Terminal") == Some(&"true"),
        },
    })
}

fn parse_exec(value: &str) -> Vec<String> {
    let mut args = Vec::new();
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
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args.into_iter()
        .filter(|argument| !argument.starts_with('%'))
        .collect()
}

fn desktop_directory() -> PathBuf {
    std::env::var_os("XDG_DESKTOP_DIR")
        .map(PathBuf::from)
        .or_else(|| desktop_directory_from_config())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Desktop")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn desktop_directory_from_config() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let contents = fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("XDG_DESKTOP_DIR="))?
        .trim()
        .trim_matches('"');
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(PathBuf::from(
        value.replace("$HOME", &home.to_string_lossy()),
    ))
}

fn open_target(target: DesktopTarget) {
    std::thread::spawn(move || match target {
        DesktopTarget::Path(path) => open_path(path),
        DesktopTarget::Launch {
            program,
            args,
            terminal,
        } => launch(program, args, terminal),
    });
}

#[cfg(target_os = "windows")]
fn open_path(path: PathBuf) {
    let _ = crate::process::spawn_detached(Command::new("explorer.exe").arg(path));
}

#[cfg(not(target_os = "windows"))]
fn open_path(path: PathBuf) {
    let _ = crate::process::spawn_detached(Command::new("xdg-open").arg(path));
}

fn launch(program: String, args: Vec<String>, terminal: bool) {
    let mut command = if terminal {
        let terminal = std::env::var("TERMINAL").unwrap_or_else(|_| "x-terminal-emulator".into());
        let mut command = Command::new(terminal);
        command.arg("-e").arg(program).args(args);
        command
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    let _ = crate::process::spawn_detached(&mut command);
}

fn fill_layout() -> LayoutStyle {
    LayoutStyle {
        position: Position::Absolute,
        inset: LayoutRect {
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Length(0.0),
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Length(0.0),
        },
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    }
}

fn icon_style(position: Point, layout: &IconLayout, selected: bool) -> Style {
    let (width, height) = layout.tile_size(selected);
    Style::new()
        .layout(LayoutStyle {
            position: Position::Absolute,
            inset: LayoutRect {
                left: LengthPercentageAuto::Length(position.x),
                right: LengthPercentageAuto::Auto,
                top: LengthPercentageAuto::Length(position.y),
                bottom: LengthPercentageAuto::Auto,
            },
            size: fixed(width, height),
            ..Default::default()
        })
        .corner_radius(layout.hover_radius)
        .hover(StateStyle::new().background(ICON_HOVER))
        .pressed(StateStyle::new().background(ICON_PRESSED))
}

fn initial_position(index: usize, layout: &IconLayout) -> Point {
    Point {
        x: GRID_INSET + (index % 4) as f32 * layout.grid_cell_x,
        y: GRID_INSET + (index / 4) as f32 * layout.grid_cell_y,
    }
}

fn icon_position(position: Point, viewport: Size, layout: &IconLayout) -> Point {
    let position = free_icon_position(position, viewport, layout);
    Point {
        x: (GRID_INSET
            + ((position.x - GRID_INSET) / layout.grid_cell_x).round() * layout.grid_cell_x)
            .clamp(GRID_INSET, grid_limit_x(viewport.width, layout)),
        y: (GRID_INSET
            + ((position.y - GRID_INSET) / layout.grid_cell_y).round() * layout.grid_cell_y)
            .clamp(
                GRID_INSET,
                grid_limit_y(viewport.height - DOCK_HEIGHT as f32, layout),
            ),
    }
}

fn grid_limit_x(length: f32, layout: &IconLayout) -> f32 {
    GRID_INSET
        + ((length - layout.tile_width - GRID_INSET * 2.0).max(0.0) / layout.grid_cell_x).floor()
            * layout.grid_cell_x
}

fn grid_limit_y(length: f32, layout: &IconLayout) -> f32 {
    GRID_INSET
        + ((length - layout.tile_height - GRID_INSET * 2.0).max(0.0) / layout.grid_cell_y).floor()
            * layout.grid_cell_y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> IconLayout {
        IconLayout::from_config(&DesktopIconsConfig::default())
    }

    #[test]
    fn icon_drop_snaps_to_the_nearest_slot() {
        let layout = test_layout();
        let viewport = Size {
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(
            icon_position(
                Point {
                    x: GRID_INSET + layout.grid_cell_x * 1.6,
                    y: GRID_INSET + layout.grid_cell_y * 2.4,
                },
                viewport,
                &layout,
            ),
            Point {
                x: GRID_INSET + layout.grid_cell_x * 2.0,
                y: GRID_INSET + layout.grid_cell_y * 2.0,
            }
        );
    }

    #[test]
    fn icon_drop_keeps_the_last_slot_within_bounds() {
        let layout = test_layout();
        let viewport = Size {
            width: 1920.0,
            height: 1080.0,
        };
        let position = icon_position(
            Point {
                x: viewport.width,
                y: viewport.height,
            },
            viewport,
            &layout,
        );
        assert_eq!(position.x, grid_limit_x(viewport.width, &layout));
        assert_eq!(
            position.y,
            grid_limit_y(viewport.height - DOCK_HEIGHT as f32, &layout)
        );
    }

    #[test]
    fn selected_tile_reserves_more_label_height_than_resting() {
        let layout = test_layout();
        let (_, resting_height) = layout.tile_size(false);
        let (_, selected_height) = layout.tile_size(true);
        assert!(selected_height > resting_height);
    }
}
