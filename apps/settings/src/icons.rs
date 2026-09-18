use creamui_core::RgbaImage;
use creamui_image::{ImageData, SvgSize};
use creamui_theme::{Color, Theme};
use creamui_widgets::{IconImage, IconSource, Symbol};

/// Rasterized versions of the custom sidebar icons in `assets/icons/settings/`,
/// decoded once at startup and cloned (cheaply — an `Rc` underneath) into the
/// sidebar tree on every rebuild. Each is monochrome, so [`Icon::draw`]
/// recolors it to match the active/hover state and theme the same way a
/// built-in [`Symbol`] would.
pub struct SettingsIcons {
    pub paintbrush: IconSource,
    pub wallpaper: IconSource,
    pub status: IconSource,
    pub widgets: IconSource,
    pub users: IconSource,
    pub keyboard: IconSource,
    pub mouse: IconSource,
    pub cursor: IconSource,
    pub devices: IconSource,
    pub windows: IconSource,
    pub system: IconSource,
    pub titlebar: IconSource,
    pub workspaces: IconSource,
    pub shortcuts: IconSource,
    pub music_note: IconSource,
    pub eye: IconSource,
    pub exclamation: IconSource,
}

impl SettingsIcons {
    pub fn load() -> Self {
        Self {
            paintbrush: decode(include_bytes!(
                "../../../assets/icons/settings/paintbrush.svg"
            )),
            wallpaper: decode(include_bytes!(
                "../../../assets/icons/settings/wallpaper.svg"
            )),
            status: decode(include_bytes!("../../../assets/icons/settings/status.svg")),
            widgets: decode(include_bytes!("../../../assets/icons/settings/widgets.svg")),
            users: decode(include_bytes!("../../../assets/icons/settings/users.svg")),
            keyboard: decode(include_bytes!("../../../assets/icons/devices/keyboard.svg")),
            mouse: decode(include_bytes!("../../../assets/icons/devices/mouse.svg")),
            cursor: decode(include_bytes!("../../../assets/icons/settings/cursor.svg")),
            devices: decode(include_bytes!("../../../assets/icons/plug.svg")),
            windows: decode(include_bytes!("../../../assets/icons/devices/window.svg")),
            system: decode(include_bytes!("../../../assets/icons/devices/monitor.svg")),
            titlebar: decode(include_bytes!("../../../assets/icons/bar-top.svg")),
            workspaces: decode(include_bytes!("../../../assets/icons/briefcase.svg")),
            shortcuts: decode(include_bytes!("../../../assets/icons/arrow-down-left.svg")),
            music_note: decode(include_bytes!("../../../assets/icons/settings/music-note.svg")),
            eye: decode(include_bytes!("../../../assets/icons/eye.svg")),
            exclamation: decode(include_bytes!("../../../assets/icons/exclamation.svg")),
        }
    }
}

/// A fully transparent 1×1 pixel: an `IconSource` for sidebar leaves that
/// must supply one but are deliberately drawn with no visible icon (each
/// panel in the dock list, distinguished by name alone).
pub fn blank() -> IconSource {
    IconSource::Image(IconImage {
        image: RgbaImage::new(1, 1, vec![0, 0, 0, 0]).expect("1x1 transparent pixel is valid"),
        monochrome: true,
    })
}

/// A plain "+" glyph with no filled circle behind it, for the "Add panel"
/// leaf — unlike [`IconSource::Initial`]'s usual avatar-style background.
pub fn plus(theme: &Theme) -> IconSource {
    IconSource::Initial {
        letter: '+',
        background: Color::rgba(0, 0, 0, 0),
        text_color: theme.text_secondary,
    }
}

fn decode(source: &[u8]) -> IconSource {
    match ImageData::from_svg(source, SvgSize::Max(64)) {
        Ok(image) => IconSource::Image(IconImage {
            image: image.image().clone(),
            monochrome: true,
        }),
        Err(error) => {
            eprintln!("settings: failed to decode a bundled icon: {error}");
            IconSource::Symbol(Symbol::Check)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_icon_decodes() {
        let icons = SettingsIcons::load();
        for icon in [
            &icons.paintbrush,
            &icons.wallpaper,
            &icons.status,
            &icons.widgets,
            &icons.users,
            &icons.keyboard,
            &icons.mouse,
            &icons.cursor,
            &icons.devices,
            &icons.windows,
            &icons.system,
            &icons.titlebar,
            &icons.workspaces,
            &icons.shortcuts,
            &icons.music_note,
            &icons.eye,
            &icons.exclamation,
        ] {
            assert!(
                matches!(icon, IconSource::Image(_)),
                "expected a rasterized icon, fell back to a symbol instead"
            );
        }
    }
}
