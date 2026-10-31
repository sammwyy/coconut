use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Icons {
    glyphs: HashMap<&'static str, IconSource>,
}

impl Icons {
    pub fn load() -> Self {
        let mut glyphs = HashMap::new();
        for (name, bytes) in [
            (
                "play",
                include_bytes!("../../../assets/glyphs/player/player_play.svg") as &[u8],
            ),
            (
                "pause",
                include_bytes!("../../../assets/glyphs/player/player_pause.svg") as &[u8],
            ),
            (
                "next",
                include_bytes!("../../../assets/glyphs/player/player_next.svg") as &[u8],
            ),
            (
                "prev",
                include_bytes!("../../../assets/glyphs/player/player_previous.svg") as &[u8],
            ),
            (
                "shuffle",
                include_bytes!("../../../assets/glyphs/media/media-shuffle.svg") as &[u8],
            ),
            (
                "repeat",
                include_bytes!("../../../assets/glyphs/media/media-repeat.svg") as &[u8],
            ),
            (
                "repeat-one",
                include_bytes!("../../../assets/glyphs/media/media-repeat-one.svg") as &[u8],
            ),
            (
                "volume",
                include_bytes!("../../../assets/glyphs/media/media-volume.svg") as &[u8],
            ),
            (
                "mute",
                include_bytes!("../../../assets/glyphs/media/media-mute.svg") as &[u8],
            ),
            (
                "speed",
                include_bytes!("../../../assets/glyphs/media/media-speed.svg") as &[u8],
            ),
            (
                "queue",
                include_bytes!("../../../assets/glyphs/media/media-queue.svg") as &[u8],
            ),
            (
                "subtitles",
                include_bytes!("../../../assets/glyphs/media/media-subtitles.svg") as &[u8],
            ),
            (
                "audio-track",
                include_bytes!("../../../assets/glyphs/media/media-audio-track.svg") as &[u8],
            ),
            (
                "open",
                include_bytes!("../../../assets/glyphs/media/media-open.svg") as &[u8],
            ),
            (
                "plus",
                include_bytes!("../../../assets/glyphs/media/media-plus.svg") as &[u8],
            ),
            (
                "close",
                include_bytes!("../../../assets/glyphs/actions/close.svg") as &[u8],
            ),
            (
                "minimize",
                include_bytes!("../../../assets/glyphs/media/media-minimize.svg") as &[u8],
            ),
            (
                "maximize",
                include_bytes!("../../../assets/glyphs/media/media-maximize.svg") as &[u8],
            ),
            (
                "music",
                include_bytes!("../../../assets/glyphs/settings/music-note.svg") as &[u8],
            ),
            (
                "film",
                include_bytes!("../../../assets/glyphs/media/media-film.svg") as &[u8],
            ),
            (
                "warning",
                include_bytes!("../../../assets/glyphs/status/exclamation.svg") as &[u8],
            ),
            (
                "app",
                include_bytes!("../../../assets/icons/apps/app-music.svg") as &[u8],
            ),
            (
                "fullscreen",
                include_bytes!("../../../assets/glyphs/media/media-fullscreen.svg") as &[u8],
            ),
            (
                "settings",
                include_bytes!("../../../assets/glyphs/media/media-settings.svg") as &[u8],
            ),
            (
                "sliders",
                include_bytes!("../../../assets/glyphs/media/media-sliders.svg") as &[u8],
            ),
            (
                "palette",
                include_bytes!("../../../assets/glyphs/media/media-palette.svg") as &[u8],
            ),
            (
                "keyboard",
                include_bytes!("../../../assets/glyphs/devices/keyboard.svg") as &[u8],
            ),
            (
                "info",
                include_bytes!("../../../assets/glyphs/media/media-info.svg") as &[u8],
            ),
            (
                "timer",
                include_bytes!("../../../assets/glyphs/media/media-timer.svg") as &[u8],
            ),
            (
                "clock",
                include_bytes!("../../../assets/glyphs/media/media-clock.svg") as &[u8],
            ),
            (
                "panel",
                include_bytes!("../../../assets/glyphs/media/media-panel.svg") as &[u8],
            ),
            (
                "sparkle",
                include_bytes!("../../../assets/glyphs/media/media-sparkle.svg") as &[u8],
            ),
            (
                "image",
                include_bytes!("../../../assets/glyphs/media/media-image.svg") as &[u8],
            ),
            (
                "forward",
                include_bytes!("../../../assets/glyphs/media/media-forward.svg") as &[u8],
            ),
            (
                "text",
                include_bytes!("../../../assets/glyphs/media/media-text.svg") as &[u8],
            ),
            (
                "chat",
                include_bytes!("../../../assets/glyphs/media/media-chat.svg") as &[u8],
            ),
            (
                "shape",
                include_bytes!("../../../assets/glyphs/media/media-shape.svg") as &[u8],
            ),
            (
                "refresh",
                include_bytes!("../../../assets/glyphs/media/media-refresh.svg") as &[u8],
            ),
            (
                "check",
                include_bytes!("../../../assets/glyphs/actions/check.svg") as &[u8],
            ),
        ] {
            glyphs.insert(name, decode(bytes, name != "app"));
        }
        Self { glyphs }
    }

    pub fn get(&self, name: &str) -> IconSource {
        self.glyphs
            .get(name)
            .unwrap_or(&self.glyphs["music"])
            .clone()
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
        assert!(matches!(icons.get("app"), IconSource::Image(_)));
    }
}
