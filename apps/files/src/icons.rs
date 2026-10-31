use crate::icon_theme::IconTheme;
use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Clone)]
pub(crate) struct Icons {
    fallback: HashMap<&'static str, IconSource>,
    theme: IconTheme,
    themed: Rc<RefCell<HashMap<String, IconSource>>>,
}

impl Icons {
    pub fn load() -> Self {
        let mut icons = HashMap::new();
        for (name, bytes) in [
            (
                "home",
                include_bytes!("../../../assets/glyphs/files/files-home.svg") as &[u8],
            ),
            (
                "folder",
                include_bytes!("../../../assets/glyphs/files/desktop-folder.svg") as &[u8],
            ),
            (
                "file",
                include_bytes!("../../../assets/glyphs/files/desktop-file.svg") as &[u8],
            ),
            (
                "desktop",
                include_bytes!("../../../assets/glyphs/devices/monitor.svg") as &[u8],
            ),
            (
                "download",
                include_bytes!("../../../assets/glyphs/files/files-download.svg") as &[u8],
            ),
            (
                "image",
                include_bytes!("../../../assets/glyphs/files/files-image.svg") as &[u8],
            ),
            (
                "video",
                include_bytes!("../../../assets/glyphs/files/files-video.svg") as &[u8],
            ),
            (
                "music",
                include_bytes!("../../../assets/glyphs/settings/music-note.svg") as &[u8],
            ),
            (
                "link",
                include_bytes!("../../../assets/glyphs/files/files-link.svg") as &[u8],
            ),
            (
                "drive",
                include_bytes!("../../../assets/glyphs/files/files-drive.svg") as &[u8],
            ),
            (
                "pin",
                include_bytes!("../../../assets/glyphs/files/files-pin.svg") as &[u8],
            ),
            (
                "back",
                include_bytes!("../../../assets/glyphs/files/files-back.svg") as &[u8],
            ),
            (
                "forward",
                include_bytes!("../../../assets/glyphs/files/files-forward.svg") as &[u8],
            ),
            (
                "up",
                include_bytes!("../../../assets/glyphs/files/files-up.svg") as &[u8],
            ),
            (
                "chevron-up",
                include_bytes!("../../../assets/glyphs/navigation/chevron-up.svg") as &[u8],
            ),
            (
                "chevron-down",
                include_bytes!("../../../assets/glyphs/navigation/chevron-down.svg") as &[u8],
            ),
            (
                "chevron-right",
                include_bytes!("../../../assets/glyphs/files/files-chevron-right.svg") as &[u8],
            ),
            (
                "search",
                include_bytes!("../../../assets/glyphs/files/files-search.svg") as &[u8],
            ),
            (
                "grid",
                include_bytes!("../../../assets/glyphs/files/files-grid.svg") as &[u8],
            ),
            (
                "list",
                include_bytes!("../../../assets/glyphs/files/files-list.svg") as &[u8],
            ),
            (
                "columns",
                include_bytes!("../../../assets/glyphs/files/files-columns.svg") as &[u8],
            ),
            (
                "plus",
                include_bytes!("../../../assets/glyphs/files/files-plus.svg") as &[u8],
            ),
            (
                "close",
                include_bytes!("../../../assets/glyphs/actions/close.svg") as &[u8],
            ),
            (
                "minimize",
                include_bytes!("../../../assets/glyphs/files/files-minimize.svg") as &[u8],
            ),
            (
                "maximize",
                include_bytes!("../../../assets/glyphs/files/files-maximize.svg") as &[u8],
            ),
            (
                "clock",
                include_bytes!("../../../assets/glyphs/files/files-clock.svg") as &[u8],
            ),
            (
                "refresh",
                include_bytes!("../../../assets/glyphs/files/files-refresh.svg") as &[u8],
            ),
            (
                "settings",
                include_bytes!("../../../assets/glyphs/settings/system.svg") as &[u8],
            ),
            (
                "folder-large",
                include_bytes!("../../../assets/glyphs/files/folder-large.svg") as &[u8],
            ),
            (
                "app",
                include_bytes!("../../../assets/icons/apps/app-files.svg") as &[u8],
            ),
        ] {
            icons.insert(name, decode(bytes, name != "folder-large" && name != "app"));
        }
        Self {
            fallback: icons,
            theme: IconTheme::system(),
            themed: Rc::new(RefCell::new(HashMap::new())),
        }
    }
    pub fn get(&self, name: &str) -> IconSource {
        if let Some(icon) = self.themed.borrow().get(name).cloned() {
            return icon;
        }
        let icon = themed_names(name)
            .and_then(|names| self.theme.load(names))
            .map(|image| {
                IconSource::Image(IconImage {
                    image: image.image().clone(),
                    monochrome: false,
                })
            })
            .unwrap_or_else(|| {
                self.fallback
                    .get(name)
                    .unwrap_or(&self.fallback["file"])
                    .clone()
            });
        self.themed.borrow_mut().insert(name.into(), icon.clone());
        icon
    }
}

/// Navigation glyphs remain bundled; places and file categories deliberately
/// use semantic Freedesktop names so the desktop's icon pack supplies them.
fn themed_names(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "home" => Some(&["user-home", "go-home"]),
        "folder" | "folder-large" => Some(&["folder"]),
        "file" => Some(&["text-x-generic", "unknown"]),
        "desktop" => Some(&["user-desktop", "desktop"]),
        "download" => Some(&["folder-download", "folder"]),
        "image" => Some(&["image-x-generic", "image"]),
        "video" => Some(&["video-x-generic", "video"]),
        "music" => Some(&["audio-x-generic", "audio"]),
        "drive" => Some(&["drive-harddisk", "drive"]),
        _ => None,
    }
}

fn decode(svg: &[u8], monochrome: bool) -> IconSource {
    let image = ImageData::from_svg(svg, SvgSize::Max(128)).expect("bundled SVG");
    IconSource::Image(IconImage {
        image: image.image().clone(),
        monochrome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_glyphs_decode() {
        let icons = Icons::load();
        assert!(matches!(
            icons.fallback["folder-large"],
            IconSource::Image(_)
        ));
        assert!(matches!(icons.fallback["app"], IconSource::Image(_)));
    }
}
