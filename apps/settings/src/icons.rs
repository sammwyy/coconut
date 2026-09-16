use creamui_widgets::{IconImage, IconSource, Symbol};
use std::rc::Rc;

/// Rasterized versions of the custom sidebar icons in `assets/icons/settings/`,
/// decoded once at startup and cloned (cheaply — an `Rc` underneath) into the
/// sidebar tree on every rebuild.
pub struct SettingsIcons {
    pub appearance: IconSource,
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
            appearance: decode(include_bytes!(
                "../../../assets/icons/settings/appearance.svg"
            )),
            paintbrush: decode(include_bytes!(
                "../../../assets/icons/settings/paintbrush.svg"
            )),
            wallpaper: decode(include_bytes!(
                "../../../assets/icons/settings/wallpaper.svg"
            )),
            position: decode(include_bytes!("../../../assets/icons/settings/move.svg")),
            status: decode(include_bytes!("../../../assets/icons/settings/status.svg")),
            widgets: decode(include_bytes!(
                "../../../assets/icons/settings/widgets.svg"
            )),
            users: decode(include_bytes!("../../../assets/icons/settings/users.svg")),
        }
    }
}

fn decode(source: &[u8]) -> IconSource {
    decode_svg(source).unwrap_or(IconSource::Symbol(Symbol::Check))
}

fn decode_svg(source: &[u8]) -> Option<IconSource> {
    let tree = resvg::usvg::Tree::from_data(source, &resvg::usvg::Options::default()).ok()?;
    let source_size = tree.size();
    let scale = 64.0 / source_size.width().max(source_size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let image = creamui_image::ImageData::from_bytes(&pixmap.encode_png().ok()?).ok()?;
    Some(IconSource::Image(IconImage {
        width: image.width(),
        height: image.height(),
        rgba: Rc::from(image.pixels()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_icon_decodes() {
        let icons = SettingsIcons::load();
        for icon in [
            &icons.appearance,
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
