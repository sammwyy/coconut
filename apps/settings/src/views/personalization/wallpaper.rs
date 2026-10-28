use crate::components::{group, row, section, tab_colors, update_background_config};
use coconut_core::{
    BackgroundConfig, BackgroundMode, BuiltinShader, DesktopColor, GradientConfig, ShaderConfig,
    WallpaperSource, MAX_SHADER_COLORS,
};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{
    tab_styles, ColorPicker, ColorPickerController, RadioGroup, Slider, Tab, TabSizing, Tabs, Text,
    TextSize,
};
use delib::xdg::XdgPaths;
use image_rs::codecs::jpeg::JpegEncoder;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::UNIX_EPOCH;

const CARD: f32 = 112.0;
const THUMBNAIL_SIZE: u32 = 224;
const WALLPAPER_EXTENSIONS: [&str; 19] = [
    "avif", "bmp", "dds", "exr", "ff", "farbfeld", "gif", "hdr", "ico", "jpe", "jpg", "jpeg",
    "png", "pnm", "qoi", "tga", "tif", "tiff", "webp",
];

/// Persists across rebuilds so the `Wallpapers` folder is scanned for
/// thumbnails at most once per app run, off the UI thread.
#[derive(Clone)]
pub struct GalleryState {
    started: Rc<Cell<bool>>,
    paths: Signal<Option<Vec<PathBuf>>>,
}

impl GalleryState {
    pub fn new() -> Self {
        Self {
            started: Rc::new(Cell::new(false)),
            paths: Signal::new(None),
        }
    }
}

impl Default for GalleryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Popup open-state for every color picker on this page: one per endpoint is
/// required so the pickers don't open and close each other. `shader` covers
/// the slots of [`BuiltinShader::palette`].
#[derive(Clone)]
pub struct WallpaperPickers {
    pub solid: ColorPickerController,
    pub gradient_from: ColorPickerController,
    pub gradient_to: ColorPickerController,
    pub shader: [ColorPickerController; MAX_SHADER_COLORS],
}

impl WallpaperPickers {
    pub fn new() -> Self {
        Self {
            solid: ColorPickerController::new(),
            gradient_from: ColorPickerController::new(),
            gradient_to: ColorPickerController::new(),
            shader: std::array::from_fn(|_| ColorPickerController::new()),
        }
    }
}

impl Default for WallpaperPickers {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build(
    _: Size,
    background: &Signal<BackgroundConfig>,
    pickers: &WallpaperPickers,
    gallery: &GalleryState,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let config = background.get();
    let source = match creamui_router::use_location().query("tab").as_deref() {
        Some("image") => WallpaperSource::Image,
        Some("color") => WallpaperSource::SolidColor,
        Some("gradient") => WallpaperSource::Gradient,
        Some("shader") => WallpaperSource::Shader,
        _ => config.source,
    };
    let content = match source {
        WallpaperSource::Image => image_content(
            background,
            config.wallpaper,
            config.recent_wallpapers,
            config.mode,
            gallery,
            window,
        ),
        WallpaperSource::SolidColor => solid_color(background, config.solid_color, &pickers.solid),
        WallpaperSource::Gradient => gradient_content(background, &config.gradient, pickers),
        WallpaperSource::Shader => shader_content(
            background,
            config.shader,
            config.shader_config(config.shader),
            pickers,
        ),
    };
    section(
        "Wallpaper",
        "Choose an image, solid color, gradient or animated shader.",
        vec![mode_tabs(background, source), content],
    )
}

const SOURCE_TABS: [(&str, &str, WallpaperSource); 4] = [
    ("Image", "image", WallpaperSource::Image),
    ("Solid color", "color", WallpaperSource::SolidColor),
    ("Gradient", "gradient", WallpaperSource::Gradient),
    ("Shader", "shader", WallpaperSource::Shader),
];

fn mode_tabs(background: &Signal<BackgroundConfig>, source: WallpaperSource) -> BoxedWidget {
    let labels = SOURCE_TABS.map(|tab| tab.0);
    let colors = tab_colors();
    let styles = tab_styles(&labels, TabSizing::Fill, 36.0, 6.0);
    let mut tabs = Tabs::new(
        colors,
        creamui_core::layout::Style {
            size: fixed(460.0, 36.0),
            ..Default::default()
        },
    )
    .gap(8.0);
    for (index, (label, query, tab_source)) in SOURCE_TABS.into_iter().enumerate() {
        let background = background.clone();
        let router = creamui_router::use_router();
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            label,
            source == tab_source,
            move || {
                crate::routes::set_query(&router, "tab", query);
                update_background_config(&background, |config| config.source = tab_source)
            },
        )));
    }
    Box::new(tabs)
}

fn placement_tabs(background: &Signal<BackgroundConfig>, selected: BackgroundMode) -> BoxedWidget {
    let labels = ["Full", "Centered", "Stretched", "Mosaic"];
    let modes = [
        BackgroundMode::Full,
        BackgroundMode::Centered,
        BackgroundMode::Stretched,
        BackgroundMode::Mosaic,
    ];
    let colors = tab_colors();
    let styles = tab_styles(&labels, TabSizing::Fill, 36.0, 6.0);
    let mut tabs = Tabs::new(
        colors,
        creamui_core::layout::Style {
            size: fixed(440.0, 36.0),
            ..Default::default()
        },
    )
    .gap(8.0);
    for (index, label) in labels.into_iter().enumerate() {
        let background = background.clone();
        let mode = modes[index];
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            label,
            selected == mode,
            move || update_background_config(&background, |config| config.mode = mode),
        )));
    }
    Box::new(tabs)
}

fn image_content(
    background: &Signal<BackgroundConfig>,
    current: Option<PathBuf>,
    mut recent: Vec<PathBuf>,
    mode: BackgroundMode,
    gallery: &GalleryState,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    recent.retain(|path| current.as_ref() != Some(path));
    recent.truncate(2);
    let mut cards = vec![add_image_card(background)];
    if let Some(path) = current.clone() {
        cards.push(image_card(background, path, true));
    }
    cards.extend(
        recent
            .into_iter()
            .map(|path| image_card(background, path, false)),
    );
    let theme = use_theme();
    let recent_row: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
    });
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_medium}>
            {placement_tabs(background, mode)}
            {Box::new(Text::secondary("Recent").size(TextSize::Sm)) as BoxedWidget}
            {recent_row}
            {wallpapers_folder_section(background, &current, gallery, window)}
        </Flex>
    })
}

fn wallpapers_folder_section(
    background: &Signal<BackgroundConfig>,
    current: &Option<PathBuf>,
    gallery: &GalleryState,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let theme = use_theme();
    let Some(directory) = wallpapers_directory() else {
        return Box::new(jsx! { <Flex /> });
    };

    if !gallery.started.get() {
        if let Some(app) = window.borrow().as_ref().map(WindowHandle::app) {
            gallery.started.set(true);
            let paths = gallery.paths.clone();
            let scan_directory = directory.clone();
            app.spawn_background(
                move || scan_wallpapers_folder(&scan_directory),
                move |found| paths.set(Some(found)),
            );
        }
    }

    let open = directory.clone();
    let open_button: BoxedWidget = Box::new(jsx! {
        <RawButton style={folder_button_style()} background={theme.surface_elevated} corner_radius={theme.button_radius} on_click={move || open_wallpapers_folder(open.clone())}>
            <Flex padding_xy={(12.0, 0.0)} align={Align::Center} justify={Justify::Center}>
                <RawText color={theme.text_secondary} font_size={13.0}>"Open folder"</RawText>
            </Flex>
        </RawButton>
    });
    let header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} justify={Justify::Between}>
            {Box::new(Text::secondary("Wallpapers").size(TextSize::Sm)) as BoxedWidget}
            {open_button}
        </Flex>
    });

    let gallery_body: BoxedWidget = match gallery.paths.get() {
        None => Box::new(jsx! { <Flex /> }),
        Some(paths) if paths.is_empty() => {
            Box::new(Text::secondary("No images in this folder yet.").size(TextSize::Sm))
        }
        Some(paths) => {
            let cards: Vec<BoxedWidget> = paths
                .into_iter()
                .map(|path| {
                    let selected = current.as_ref() == Some(&path);
                    image_card(background, path, selected)
                })
                .collect();
            Box::new(jsx! {
                <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
            })
        }
    };

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_medium}>
            {header}
            {gallery_body}
        </Flex>
    })
}

fn folder_button_style() -> creamui_core::Style {
    creamui_core::Style::new().layout(creamui_core::layout::Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Auto,
            height: creamui_core::layout::Dimension::Length(30.0),
        },
        ..Default::default()
    })
}

fn add_image_card(background: &Signal<BackgroundConfig>) -> BoxedWidget {
    let theme = use_theme();
    let background = background.clone();
    Box::new(jsx! {
        <RawButton
            style={card_style()}
            background={theme.surface_elevated}
            corner_radius={theme.card_radius}
            on_click={move || {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose a wallpaper")
                    .add_filter("Images", &WALLPAPER_EXTENSIONS)
                    .pick_file()
                {
                    select_image(&background, path);
                }
            }}
        >
            <Flex size={(CARD, CARD)} align={Align::Center} justify={Justify::Center}>
                <RawText color={theme.text_secondary} font_size={32.0}>"+"</RawText>
            </Flex>
        </RawButton>
    })
}

fn image_card(background: &Signal<BackgroundConfig>, path: PathBuf, selected: bool) -> BoxedWidget {
    let theme = use_theme();
    let preview: BoxedWidget = thumbnail_for(&path)
        .as_deref()
        .and_then(|path| ImageData::from_path(path).ok())
        .map(|data| {
            Box::new(Image::new(data).layout(fixed_style()).fit(ImageFit::Cover)) as BoxedWidget
        })
        .unwrap_or_else(|| {
            Box::new(
                jsx! { <Flex size={(CARD - 8.0, CARD - 8.0)} background={theme.surface_hover} /> },
            )
        });
    let choose = background.clone();
    let card: BoxedWidget = Box::new(jsx! {
        <RawButton style={card_style()} background={theme.surface_elevated} corner_radius={theme.card_radius} on_click={move || select_image(&choose, path.clone())}>
            <Flex padding={4.0}>{preview}</Flex>
        </RawButton>
    });
    Box::new(jsx! {
        <Flex size={(CARD + 4.0, CARD + 4.0)} padding={2.0} background={if selected { theme.accent } else { Color::rgba(0, 0, 0, 0) }} corner_radius={theme.card_radius + 2.0}>{card}</Flex>
    })
}

fn card_style() -> creamui_core::Style {
    creamui_core::Style::new().layout(creamui_core::layout::Style {
        size: fixed(CARD, CARD),
        ..Default::default()
    })
}

fn fixed_style() -> creamui_core::layout::Style {
    creamui_core::layout::Style {
        size: fixed(CARD - 8.0, CARD - 8.0),
        ..Default::default()
    }
}

fn select_image(background: &Signal<BackgroundConfig>, path: PathBuf) {
    let _ = cache_thumbnail(&path);
    update_background_config(background, move |config| {
        if let Some(previous) = config.wallpaper.take() {
            if previous != path {
                config
                    .recent_wallpapers
                    .retain(|recent| recent != &previous);
                config.recent_wallpapers.insert(0, previous);
            }
        }
        config.wallpaper = Some(path.clone());
        config.source = WallpaperSource::Image;
        config.recent_wallpapers.retain(|recent| recent != &path);
        config.recent_wallpapers.truncate(2);
    });
}

fn thumbnail_for(source: &Path) -> Option<PathBuf> {
    cache_thumbnail(source).ok()
}

fn cache_thumbnail(source: &Path) -> Result<PathBuf, ()> {
    let metadata = fs::metadata(source).map_err(|_| ())?;
    let cache_directory = thumbnail_cache_directory().ok_or(())?;
    fs::create_dir_all(&cache_directory).map_err(|_| ())?;

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|modified| (modified.as_secs(), modified.subsec_nanos()))
        .hash(&mut hasher);
    let thumbnail = cache_directory.join(format!("{:016x}.jpg", hasher.finish()));
    if thumbnail.is_file() {
        return Ok(thumbnail);
    }

    let image = image_rs::open(source).map_err(|_| ())?;
    let image = image.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let temporary = thumbnail.with_extension("tmp");
    let file = fs::File::create(&temporary).map_err(|_| ())?;
    JpegEncoder::new_with_quality(file, 82)
        .encode_image(&image)
        .map_err(|_| ())?;
    fs::rename(&temporary, &thumbnail).map_err(|_| ())?;
    Ok(thumbnail)
}

fn thumbnail_cache_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|directory| directory.join("coconut").join("wallpaper-thumbnails"))
}

fn solid_color(
    background: &Signal<BackgroundConfig>,
    color: DesktopColor,
    controller: &ColorPickerController,
) -> BoxedWidget {
    let apply = background.clone();
    let picker: BoxedWidget = Box::new(ColorPicker::controlled(
        Color::rgb(color.r, color.g, color.b),
        controller,
        move |color| {
            update_background_config(&apply, |config| {
                config.source = WallpaperSource::SolidColor;
                config.solid_color = DesktopColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                };
            })
        },
    ));
    group(vec![row("Color", picker)])
}

fn gradient_content(
    background: &Signal<BackgroundConfig>,
    gradient: &GradientConfig,
    pickers: &WallpaperPickers,
) -> BoxedWidget {
    let theme = use_theme();
    let apply = background.clone();
    let from: BoxedWidget = Box::new(ColorPicker::controlled(
        Color::rgb(gradient.from.r, gradient.from.g, gradient.from.b),
        &pickers.gradient_from,
        move |color| {
            update_background_config(&apply, |config| {
                config.source = WallpaperSource::Gradient;
                config.gradient.from = DesktopColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                };
            })
        },
    ));
    let apply = background.clone();
    let to: BoxedWidget = Box::new(ColorPicker::controlled(
        Color::rgb(gradient.to.r, gradient.to.g, gradient.to.b),
        &pickers.gradient_to,
        move |color| {
            update_background_config(&apply, |config| {
                config.source = WallpaperSource::Gradient;
                config.gradient.to = DesktopColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                };
            })
        },
    ));
    let apply = background.clone();
    let slider: BoxedWidget = Box::new(Slider::new(gradient.angle_degrees / 360.0, move |value| {
        update_background_config(&apply, |config| {
            config.source = WallpaperSource::Gradient;
            config.gradient.angle_degrees = value * 360.0;
        })
    }));
    let degrees: BoxedWidget = Box::new(
        Text::secondary(format!("{}°", gradient.angle_degrees.round() as i32)).size(TextSize::Sm),
    );
    let angle: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={theme.spacing_medium}>
            {slider}
            {degrees}
        </Flex>
    });
    group(vec![
        row("Start color", from),
        row("End color", to),
        row("Angle", angle),
    ])
}

fn shader_content(
    background: &Signal<BackgroundConfig>,
    selected: BuiltinShader,
    tuning: ShaderConfig,
    pickers: &WallpaperPickers,
) -> BoxedWidget {
    let theme = use_theme();
    let apply = background.clone();
    let selected_index = BuiltinShader::ALL
        .iter()
        .position(|shader| *shader == selected)
        .unwrap_or(0);
    let mut shaders = RadioGroup::new(selected_index, move |index| {
        update_background_config(&apply, |config| {
            config.source = WallpaperSource::Shader;
            config.shader = BuiltinShader::ALL[index];
        })
    });
    for shader in BuiltinShader::ALL {
        shaders = shaders.option(shader.name());
    }

    let apply = background.clone();
    let speed: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={theme.spacing_medium}>
            {Box::new(Slider::new(((tuning.speed - 0.1) / 2.9).clamp(0.0, 1.0), move |value| {
                update_background_config(&apply, |config| {
                    config.source = WallpaperSource::Shader;
                    config.shader_settings(selected).speed = 0.1 + value * 2.9;
                })
            })) as BoxedWidget}
            {Box::new(Text::secondary(format!("{:.2}×", tuning.speed)).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    });
    let apply = background.clone();
    let fps: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={theme.spacing_medium}>
            {Box::new(Slider::new(tuning.fps.saturating_sub(1) as f32 / 59.0, move |value| {
                update_background_config(&apply, |config| {
                    config.source = WallpaperSource::Shader;
                    config.shader_settings(selected).fps = (1.0 + value * 59.0).round() as u32;
                })
            })) as BoxedWidget}
            {Box::new(Text::secondary(format!("{} fps", tuning.fps.clamp(1, 60))).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    });
    let mut tuning_rows = vec![row("Speed", speed), row("Frame rate", fps)];
    for (slot, (label, _)) in selected.palette().iter().enumerate() {
        let current = tuning.color(selected, slot);
        let apply = background.clone();
        let picker: BoxedWidget = Box::new(ColorPicker::controlled(
            Color::rgb(current.r, current.g, current.b),
            &pickers.shader[slot],
            move |color| {
                update_background_config(&apply, |config| {
                    config.source = WallpaperSource::Shader;
                    let entry = config.shader_settings(selected);
                    while entry.colors.len() <= slot {
                        let next = entry.colors.len();
                        let fill = entry.color(selected, next);
                        entry.colors.push(fill);
                    }
                    entry.colors[slot] = DesktopColor {
                        r: color.r,
                        g: color.g,
                        b: color.b,
                    };
                })
            },
        ));
        tuning_rows.push(row(label, picker));
    }

    let card: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} padding={16.0} gap={theme.spacing_medium}>
            {Box::new(shaders) as BoxedWidget}
            {Box::new(Text::secondary("Rendered by the GPU. If no graphics adapter is available the solid color is shown instead.").size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    });
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_medium}>
            {group(vec![card])}
            {group(tuning_rows)}
        </Flex>
    })
}

/// Where wallpapers live by convention: the platform's pictures directory,
/// in a `Wallpapers` subfolder. Never created just by looking — only
/// [`open_wallpapers_folder`] (an explicit user action) creates it.
fn wallpapers_directory() -> Option<PathBuf> {
    pictures_directory().map(|directory| directory.join("Wallpapers"))
}

#[cfg(target_os = "windows")]
fn pictures_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("Pictures"))
}

#[cfg(target_os = "macos")]
fn pictures_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Pictures"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn pictures_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("XDG_PICTURES_DIR") {
        return Some(PathBuf::from(directory));
    }
    if let Some(directory) = pictures_directory_from_user_dirs() {
        return Some(directory);
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Pictures"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn pictures_directory_from_user_dirs() -> Option<PathBuf> {
    let config = XdgPaths::default().config_home;
    let contents = fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    let value = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("XDG_PICTURES_DIR="))?
        .trim()
        .trim_matches('"');
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(PathBuf::from(
        value.replace("$HOME", &home.to_string_lossy()),
    ))
}

/// Scans `directory` for image files and pre-warms their thumbnail cache in
/// parallel, so the gallery that follows renders instantly. A missing
/// directory just yields no results — it is never created here.
fn scan_wallpapers_folder(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_wallpaper_image(path))
        .collect();
    paths.sort();

    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .max(1);
    let chunk_size = paths.len().max(1).div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for chunk in paths.chunks(chunk_size) {
            scope.spawn(move || {
                for path in chunk {
                    let _ = cache_thumbnail(path);
                }
            });
        }
    });
    paths
}

fn is_wallpaper_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| WALLPAPER_EXTENSIONS.contains(&extension.as_str()))
}

/// Opens `directory` in the platform's default file manager, creating it
/// first if this is the first time — an explicit action the user asked for
/// by clicking the button, unlike [`wallpapers_directory`] just being read.
fn open_wallpapers_folder(directory: PathBuf) {
    std::thread::spawn(move || {
        let _ = fs::create_dir_all(&directory);
        open_folder(&directory);
    });
}

#[cfg(target_os = "windows")]
fn open_folder(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}

#[cfg(target_os = "macos")]
fn open_folder(path: &Path) {
    let _ = Command::new("open").arg(path).spawn();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_folder(path: &Path) {
    let _ = Command::new("xdg-open").arg(path).spawn();
}

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let background = &context.background;
    let window = &context.window;
    let wallpaper_pickers = &context.wallpaper_pickers;
    let wallpaper_gallery = &context.wallpaper_gallery;
    build(
        size,
        background,
        wallpaper_pickers,
        wallpaper_gallery,
        window,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_image_extensions_are_case_insensitive() {
        assert!(is_wallpaper_image(Path::new("beach.PNG")));
        assert!(is_wallpaper_image(Path::new("beach.jpeg")));
        assert!(is_wallpaper_image(Path::new("beach.gif")));
        assert!(is_wallpaper_image(Path::new("beach.avif")));
        assert!(!is_wallpaper_image(Path::new("beach.svg")));
        assert!(!is_wallpaper_image(Path::new("beach")));
    }

    #[test]
    fn scanning_a_missing_folder_yields_nothing_and_does_not_create_it() {
        let directory = std::env::temp_dir().join(format!(
            "coconut-settings-test-{}-missing-wallpapers",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        assert!(scan_wallpapers_folder(&directory).is_empty());
        assert!(!directory.exists(), "scanning must never create the folder");
    }

    #[test]
    fn scanning_only_picks_up_image_files() {
        let directory = std::env::temp_dir().join(format!(
            "coconut-settings-test-{}-wallpapers",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("notes.txt"), b"not an image").unwrap();
        let image = image_rs::RgbaImage::from_pixel(4, 4, image_rs::Rgba([1, 2, 3, 255]));
        image_rs::DynamicImage::ImageRgba8(image)
            .save(directory.join("beach.png"))
            .unwrap();

        let found = scan_wallpapers_folder(&directory);

        fs::remove_dir_all(&directory).ok();
        assert_eq!(found, vec![directory.join("beach.png")]);
    }
}
