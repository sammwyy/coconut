use crate::common::{group, row, section, update_config};
use coconut_core::{DesktopColor, ShellConfig, WallpaperMode};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{
    tab_styles, ColorPicker, ColorPickerController, Tab, TabColors, TabSizing, Tabs,
};
use image_rs::codecs::jpeg::JpegEncoder;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CARD: f32 = 112.0;
const THUMBNAIL_SIZE: u32 = 224;

pub fn build(_: Size, config: &Signal<ShellConfig>, picker: &ColorPickerController) -> BoxedWidget {
    let desktop = config.get().desktop;
    let image_mode = desktop.wallpaper_mode == WallpaperMode::Image;
    let content = if image_mode {
        image_content(config, desktop.wallpaper, desktop.recent_wallpapers)
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
) -> BoxedWidget {
    recent.retain(|path| current.as_ref() != Some(path));
    recent.truncate(2);
    let mut cards = vec![add_image_card(config)];
    if let Some(path) = current {
        cards.push(image_card(config, path, true));
    }
    cards.extend(
        recent
            .into_iter()
            .map(|path| image_card(config, path, false)),
    );
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
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
