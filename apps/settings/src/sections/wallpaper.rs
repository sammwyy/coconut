use crate::common::{group, row, section, update_config};
use coconut_core::{DesktopColor, ShellConfig, WallpaperMode};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{
    tab_styles, ColorPicker, ColorPickerController, Tab, TabColors, TabSizing, Tabs, Text,
    TextSize,
};
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
const WALLPAPER_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

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

pub fn build(
    _: Size,
    config: &Signal<ShellConfig>,
    picker: &ColorPickerController,
    gallery: &GalleryState,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let desktop = config.get().desktop;
    let image_mode = desktop.wallpaper_mode == WallpaperMode::Image;
    let content = if image_mode {
        image_content(
            config,
            desktop.wallpaper,
            desktop.recent_wallpapers,
            gallery,
            window,
        )
    } else {
        solid_color(config, desktop.solid_color, picker)
    };
    section(
        "Wallpaper",
        "Choose an image or a solid desktop color.",
        vec![mode_tabs(config, image_mode), content],
    )
}

fn mode_tabs(config: &Signal<ShellConfig>, image_mode: bool) -> BoxedWidget {
    let labels = ["Image", "Solid color"];
    let colors = TabColors::dark();
    let styles = tab_styles(&labels, TabSizing::Fill, 36.0, 6.0);
    let mut tabs = Tabs::new(
        colors,
        creamui_core::layout::Style {
            size: fixed(260.0, 36.0),
            ..Default::default()
        },
    )
    .gap(4.0);
    for (index, label) in labels.into_iter().enumerate() {
        let config = config.clone();
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            styles[index].clone(),
            label,
            (index == 0) == image_mode,
            move || {
                update_config(&config, |config| {
                    config.desktop.wallpaper_mode = if index == 0 {
                        WallpaperMode::Image
                    } else {
                        WallpaperMode::SolidColor
                    };
                })
            },
        )));
    }
    Box::new(tabs)
}

fn image_content(
    config: &Signal<ShellConfig>,
    current: Option<PathBuf>,
    mut recent: Vec<PathBuf>,
    gallery: &GalleryState,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    recent.retain(|path| current.as_ref() != Some(path));
    recent.truncate(2);
    let mut cards = vec![add_image_card(config)];
    if let Some(path) = current.clone() {
        cards.push(image_card(config, path, true));
    }
    cards.extend(
        recent
            .into_iter()
            .map(|path| image_card(config, path, false)),
    );
    let theme = use_theme();
    let recent_row: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
    });
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_medium}>
            {Box::new(Text::secondary("Recent").size(TextSize::Sm)) as BoxedWidget}
            {recent_row}
            {wallpapers_folder_section(config, &current, gallery, window)}
        </Flex>
    })
}

fn wallpapers_folder_section(
    config: &Signal<ShellConfig>,
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
                    image_card(config, path, selected)
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

fn add_image_card(config: &Signal<ShellConfig>) -> BoxedWidget {
    let theme = use_theme();
    let config = config.clone();
    Box::new(jsx! {
        <RawButton
            style={card_style()}
            background={theme.surface_elevated}
            corner_radius={theme.card_radius}
            on_click={move || {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose a wallpaper")
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
                {
                    select_image(&config, path);
                }
            }}
        >
            <Flex size={(CARD, CARD)} align={Align::Center} justify={Justify::Center}>
                <RawText color={theme.text_secondary} font_size={32.0}>"+"</RawText>
            </Flex>
        </RawButton>
    })
}

fn image_card(config: &Signal<ShellConfig>, path: PathBuf, selected: bool) -> BoxedWidget {
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
    let choose = config.clone();
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

fn select_image(config: &Signal<ShellConfig>, path: PathBuf) {
    let _ = cache_thumbnail(&path);
    update_config(config, move |config| {
        if let Some(previous) = config.desktop.wallpaper.take() {
            if previous != path {
                config
                    .desktop
                    .recent_wallpapers
                    .retain(|recent| recent != &previous);
                config.desktop.recent_wallpapers.insert(0, previous);
            }
        }
        config.desktop.wallpaper = Some(path.clone());
        config.desktop.wallpaper_mode = WallpaperMode::Image;
        config
            .desktop
            .recent_wallpapers
            .retain(|recent| recent != &path);
        config.desktop.recent_wallpapers.truncate(2);
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
    config: &Signal<ShellConfig>,
    color: DesktopColor,
    controller: &ColorPickerController,
) -> BoxedWidget {
    let apply = config.clone();
    let picker: BoxedWidget = Box::new(ColorPicker::controlled(
        Color::rgb(color.r, color.g, color.b),
        controller,
        move |color| {
            update_config(&apply, |config| {
                config.desktop.wallpaper_mode = WallpaperMode::SolidColor;
                config.desktop.solid_color = DesktopColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                };
            })
        },
    ));
    group(vec![row("Color", picker)])
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
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_image_extensions_are_case_insensitive() {
        assert!(is_wallpaper_image(Path::new("beach.PNG")));
        assert!(is_wallpaper_image(Path::new("beach.jpeg")));
        assert!(!is_wallpaper_image(Path::new("beach.gif")));
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
        assert!(
            !directory.exists(),
            "scanning must never create the folder"
        );
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
