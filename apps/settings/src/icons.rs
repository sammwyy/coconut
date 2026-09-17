use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource, Symbol};
use std::rc::Rc;

/// Rasterized versions of the custom sidebar icons in `assets/icons/settings/`,
/// decoded once at startup and cloned (cheaply — an `Rc` underneath) into the
/// sidebar tree on every rebuild. Each is monochrome, so [`Icon::draw`]
/// recolors it to match the active/hover state and theme the same way a
/// built-in [`Symbol`] would.
pub struct SettingsIcons {
    pub paintbrush: IconSource,
    pub wallpaper: IconSource,
    pub position: IconSource,
    pub status: IconSource,
    pub widgets: IconSource,
    pub users: IconSource,
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
            position: decode(include_bytes!("../../../assets/icons/settings/move.svg")),
            status: decode(include_bytes!("../../../assets/icons/settings/status.svg")),
            widgets: decode(include_bytes!("../../../assets/icons/settings/widgets.svg")),
            users: decode(include_bytes!("../../../assets/icons/settings/users.svg")),
        }
    }
}

fn decode(source: &[u8]) -> IconSource {
    match ImageData::from_svg(source, SvgSize::Max(64)) {
        Ok(image) => IconSource::Image(IconImage {
            width: image.width(),
            height: image.height(),
            rgba: Rc::from(image.pixels()),
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
            &icons.position,
            &icons.status,
            &icons.widgets,
            &icons.users,
        ] {
            assert!(
                matches!(icon, IconSource::Image(_)),
                "expected a rasterized icon, fell back to a symbol instead"
            );
        }
    }
}
