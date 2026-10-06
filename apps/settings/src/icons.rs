use creamui_core::RgbaImage;
use creamui_image::{ImageData, SvgSize};
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
    pub islands: IconSource,
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
    pub applications: IconSource,
    pub privacy: IconSource,
    pub accessibility: IconSource,
    pub about: IconSource,
    pub search: IconSource,
    pub headphones: IconSource,
    pub minimize: IconSource,
    pub maximize: IconSource,
    pub close: IconSource,
}

impl SettingsIcons {
    pub fn load() -> Self {
        Self {
            paintbrush: outline(
                r#"<circle cx="13.5" cy="6.5" r=".5"/><circle cx="17.5" cy="10.5" r=".5"/><circle cx="8.5" cy="7.5" r=".5"/><circle cx="6.5" cy="12.5" r=".5"/><path d="M12 22a10 10 0 1 1 10-10 4 4 0 0 1-4 4h-1.8a1.8 1.8 0 0 0-1.4 3 1.8 1.8 0 0 1-1.4 3Z"/>"#,
            ),
            wallpaper: outline(
                r#"<rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/>"#,
            ),
            // Connectivity is represented by Wi-Fi in the reference UI;
            // the former ellipsis glyph read as a placeholder in both the
            // sidebar and the page header.
            status: decode(include_bytes!(
                "../../../assets/icons/settings/connectivity.svg"
            )),
            islands: decode(include_bytes!("../../../assets/icons/settings/widgets.svg")),
            users: outline(
                r#"<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2m20 0v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/><circle cx="9" cy="7" r="4"/>"#,
            ),
            keyboard: decode(include_bytes!("../../../assets/icons/devices/keyboard.svg")),
            mouse: decode(include_bytes!("../../../assets/icons/devices/mouse.svg")),
            cursor: decode(include_bytes!("../../../assets/icons/settings/cursor.svg")),
            devices: outline(
                r#"<rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 1v3m6-3v3M9 20v3m6-3v3M1 9h3m-3 6h3M20 9h3m-3 6h3"/>"#,
            ),
            windows: outline(
                r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18m-15-3h.01M9 6h.01"/>"#,
            ),
            system: decode(include_bytes!("../../../assets/icons/settings/system.svg")),
            titlebar: decode(include_bytes!("../../../assets/icons/bar-top.svg")),
            workspaces: decode(include_bytes!("../../../assets/icons/briefcase.svg")),
            shortcuts: outline(
                r#"<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 16h10"/>"#,
            ),
            music_note: decode(include_bytes!(
                "../../../assets/icons/settings/music-note.svg"
            )),
            eye: decode(include_bytes!("../../../assets/icons/eye.svg")),
            exclamation: decode(include_bytes!("../../../assets/icons/exclamation.svg")),
            applications: outline(
                r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>"#,
            ),
            privacy: outline(
                r#"<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/><path d="m9 12 2 2 4-4"/>"#,
            ),
            accessibility: outline(
                r#"<circle cx="16" cy="4" r="1"/><path d="m18 19 1-7-6 1m-1 9 3-7-4-4 4-3 4 3 3 1M7 12a5 5 0 1 0 6 7M6 8l4-1"/>"#,
            ),
            about: outline(r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4m0-4h.01"/>"#),
            search: outline(r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>"#),
            headphones: outline(
                r#"<path d="M3 14v-3a9 9 0 0 1 18 0v3"/><rect x="3" y="13" width="4" height="8" rx="2"/><rect x="17" y="13" width="4" height="8" rx="2"/>"#,
            ),
            minimize: outline(r#"<path d="M7 12h10"/>"#),
            maximize: outline(r#"<rect x="7" y="7" width="10" height="10" rx="1"/>"#),
            close: outline(r#"<path d="m7 7 10 10M17 7 7 17"/>"#),
        }
    }
}

/// Outline glyphs use the same optical grid and stroke as the React mock.
pub(crate) fn outline(paths: &str) -> IconSource {
    decode(format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"##).as_bytes())
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
            &icons.islands,
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
            &icons.applications,
            &icons.privacy,
            &icons.accessibility,
            &icons.about,
            &icons.search,
            &icons.headphones,
            &icons.minimize,
            &icons.maximize,
            &icons.close,
        ] {
            assert!(
                matches!(icon, IconSource::Image(_)),
                "expected a rasterized icon, fell back to a symbol instead"
            );
        }
    }
}
