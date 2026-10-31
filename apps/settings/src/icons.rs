use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource, Symbol};

/// Monochrome icons decoded once and shared across renders.
#[derive(Clone)]
pub struct SettingsIcons {
    pub paintbrush: IconSource,
    pub wallpaper: IconSource,
    pub status: IconSource,
    pub users: IconSource,
    pub devices: IconSource,
    pub windows: IconSource,
    pub system: IconSource,
    pub shortcuts: IconSource,
    pub applications: IconSource,
    pub privacy: IconSource,
    pub accessibility: IconSource,
    pub about: IconSource,
    pub search: IconSource,
    pub headphones: IconSource,
    pub network: IconSource,
    pub computer: IconSource,
    pub server: IconSource,
    pub lock: IconSource,
    pub bypass: IconSource,
    pub minimize: IconSource,
    pub maximize: IconSource,
    pub close: IconSource,
    pub drag: IconSource,
}

impl SettingsIcons {
    pub fn load() -> Self {
        Self {
            drag: decode(include_bytes!(
                "../../../assets/glyphs/actions/drag-handle.svg"
            )),
            paintbrush: decode(include_bytes!(
                "../../../assets/glyphs/settings/paintbrush.svg"
            )),
            wallpaper: decode(include_bytes!(
                "../../../assets/glyphs/settings/wallpaper.svg"
            )),
            // Connectivity is represented by Wi-Fi in the reference UI;
            // the former ellipsis glyph read as a placeholder in both the
            // sidebar and the page header.
            status: decode(include_bytes!(
                "../../../assets/glyphs/settings/connectivity.svg"
            )),
            users: decode(include_bytes!("../../../assets/glyphs/settings/users.svg")),
            devices: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-devices.svg"
            )),
            windows: decode(include_bytes!("../../../assets/glyphs/devices/window.svg")),
            system: decode(include_bytes!("../../../assets/glyphs/settings/system.svg")),
            shortcuts: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-shortcuts.svg"
            )),
            applications: decode(include_bytes!(
                "../../../assets/glyphs/navigation/appgrid.svg"
            )),
            privacy: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-privacy.svg"
            )),
            accessibility: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-accessibility.svg"
            )),
            about: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-about.svg"
            )),
            search: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-search.svg"
            )),
            headphones: decode(include_bytes!(
                "../../../assets/glyphs/devices/headphones.svg"
            )),
            network: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-network.svg"
            )),
            computer: decode(include_bytes!("../../../assets/glyphs/devices/laptop.svg")),
            server: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-server.svg"
            )),
            lock: decode(include_bytes!("../../../assets/glyphs/status/lock.svg")),
            bypass: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-bypass.svg"
            )),
            minimize: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-minimize.svg"
            )),
            maximize: decode(include_bytes!(
                "../../../assets/glyphs/settings/settings-maximize.svg"
            )),
            close: decode(include_bytes!("../../../assets/glyphs/actions/close.svg")),
        }
    }
}

/// Outline glyphs use the same optical grid and stroke as the React mock.
pub(crate) fn outline(paths: &str) -> IconSource {
    decode(format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"##).as_bytes())
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
            &icons.users,
            &icons.devices,
            &icons.windows,
            &icons.system,
            &icons.shortcuts,
            &icons.applications,
            &icons.privacy,
            &icons.accessibility,
            &icons.about,
            &icons.search,
            &icons.headphones,
            &icons.network,
            &icons.computer,
            &icons.server,
            &icons.lock,
            &icons.bypass,
            &icons.minimize,
            &icons.maximize,
            &icons.close,
            &icons.drag,
        ] {
            assert!(
                matches!(icon, IconSource::Image(_)),
                "expected a rasterized icon, fell back to a symbol instead"
            );
        }
    }
}
