use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Icons {
    glyphs: HashMap<&'static str, IconSource>,
}

impl Icons {
    pub fn load() -> Self {
        let paths: &[(&'static str, &str)] = &[
            ("play", r#"<path d="M8 5.5v13l11-6.5Z"/>"#),
            ("pause", r#"<path d="M8 5v14M16 5v14" stroke-width="2.4"/>"#),
            (
                "next",
                r#"<path d="M6 5.5v13l9-6.5Z"/><path d="M17 5v14" stroke-width="2.2"/>"#,
            ),
            (
                "prev",
                r#"<path d="M18 5.5v13l-9-6.5Z"/><path d="M7 5v14" stroke-width="2.2"/>"#,
            ),
            (
                "shuffle",
                r#"<path d="M16 4h4v4M20 4l-7.5 7.5M16 20h4v-4M20 20l-7.5-7.5M4 8h3l2.5 2.5M4 16h3l2.5-2.5"/>"#,
            ),
            (
                "repeat",
                r#"<path d="M17 3l3 3-3 3M7 21l-3-3 3-3"/><path d="M20 6H8a4 4 0 0 0-4 4v2M4 18h12a4 4 0 0 0 4-4v-2"/>"#,
            ),
            (
                "repeat-one",
                r#"<path d="M17 3l3 3-3 3M7 21l-3-3 3-3"/><path d="M20 6H8a4 4 0 0 0-4 4v2M4 18h12a4 4 0 0 0 4-4v-2"/><path d="M11.5 9.5v5M11.5 9.5 10 11" stroke-width="1.3"/>"#,
            ),
            (
                "volume",
                r#"<path d="M4 9v6h4l6 4.5v-15L8 9H4Z"/><path d="M16.5 9.5a4 4 0 0 1 0 5M19 7a7.5 7.5 0 0 1 0 10"/>"#,
            ),
            (
                "mute",
                r#"<path d="M4 9v6h4l6 4.5v-15L8 9H4Z"/><path d="m17 9.5 5 5m0-5-5 5"/>"#,
            ),
            (
                "speed",
                r#"<circle cx="12" cy="13" r="8"/><path d="M12 13l3.5-3.5"/><path d="M12 5V3" stroke-width="1.2"/>"#,
            ),
            (
                "queue",
                r#"<path d="M4 6h16M4 11h16M4 16h9"/><circle cx="18.5" cy="17.5" r="2.6"/><path d="M18.5 15v-1.5" stroke-width="1.2"/>"#,
            ),
            (
                "subtitles",
                r#"<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M5 13h6M5 16h4M14 13h5M13 16h6"/>"#,
            ),
            (
                "audio-track",
                r#"<path d="M4 9v6h4l6 4.5v-15L8 9H4Z"/><path d="M17 7a7 7 0 0 1 0 10M20 4a11 11 0 0 1 0 16"/>"#,
            ),
            (
                "open",
                r#"<path d="M3 8V6a2 2 0 0 1 2-2h4l3 3h7a2 2 0 0 1 2 2v2"/><path d="M3 8h18l-2 11a2 2 0 0 1-2 1.6H7A2 2 0 0 1 5 19L3 8Z"/>"#,
            ),
            ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
            ("close", r#"<path d="m6 6 12 12M6 18 18 6"/>"#),
            ("minimize", r#"<path d="M5 12h14"/>"#),
            (
                "maximize",
                r#"<rect x="5" y="5" width="14" height="14" rx="2"/>"#,
            ),
            (
                "music",
                r#"<path d="M9.5 18.5V6l11-2.2V17"/><ellipse cx="6.8" cy="18.5" rx="2.8" ry="2.5"/><ellipse cx="17.6" cy="17" rx="2.8" ry="2.5"/>"#,
            ),
            (
                "film",
                r#"<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M8 4.5v15M16 4.5v15M3 9h5M3 15h5M16 9h5M16 15h5" stroke-width="1.3"/>"#,
            ),
            (
                "warning",
                r#"<path d="M12 4 2.5 20h19L12 4Z"/><path d="M12 10v4.5M12 17.4v.1" stroke-width="1.9"/>"#,
            ),
            (
                "app",
                r#"<circle cx="12" cy="12" r="9.5"/><path d="M9.7 7.8v8.4l7.2-4.2Z"/>"#,
            ),
            (
                "fullscreen",
                r#"<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>"#,
            ),
            (
                "settings",
                r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z"/><circle cx="12" cy="12" r="3"/>"#,
            ),
            (
                "sliders",
                r#"<path d="M4 7h9M19 7h1M4 17h3M13 17h7"/><circle cx="16" cy="7" r="2.5"/><circle cx="10" cy="17" r="2.5"/>"#,
            ),
            (
                "palette",
                r#"<path d="M12 3a9 9 0 1 0 0 18c1.1 0 1.8-.8 1.8-1.8 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 0-1 .8-1.8 1.8-1.8H17a4 4 0 0 0 4-4c0-4.4-4-8-9-8Z"/><circle cx="7.5" cy="11" r="1.1"/><circle cx="10" cy="7" r="1.1"/><circle cx="14.5" cy="7" r="1.1"/>"#,
            ),
            (
                "keyboard",
                r#"<rect x="2.5" y="6" width="19" height="12" rx="2.5"/><path d="M6.5 10h.01M10 10h.01M14 10h.01M17.5 10h.01M8 14h8" stroke-width="1.8"/>"#,
            ),
            (
                "info",
                r#"<circle cx="12" cy="12" r="9.5"/><path d="M12 11v5.5M12 7.6v.1" stroke-width="1.9"/>"#,
            ),
            (
                "timer",
                r#"<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2M10 2.5h4"/>"#,
            ),
            (
                "clock",
                r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
            ),
            (
                "panel",
                r#"<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M7 15h10"/>"#,
            ),
            (
                "sparkle",
                r#"<path d="M11 3.5l1.8 5.2L18 10.5l-5.2 1.8L11 17.5l-1.8-5.2L4 10.5l5.2-1.8Z"/><path d="M19 15.5v5M16.5 18h5" stroke-width="1.3"/>"#,
            ),
            (
                "image",
                r#"<rect x="3" y="4" width="18" height="16" rx="2.5"/><circle cx="9" cy="9.5" r="1.8"/><path d="m21 15-5-5L5 20"/>"#,
            ),
            (
                "forward",
                r#"<path d="M4 6.5v11l7-5.5Z"/><path d="M13 6.5v11l7-5.5Z"/>"#,
            ),
            ("text", r#"<path d="M5 6h14M5 11h14M5 16h8"/>"#),
            (
                "chat",
                r#"<path d="M21 14a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2Z"/>"#,
            ),
            (
                "shape",
                r#"<rect x="3.5" y="3.5" width="10" height="10" rx="2"/><circle cx="15.5" cy="15.5" r="5"/>"#,
            ),
            (
                "refresh",
                r#"<path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7"/>"#,
            ),
            (
                "check",
                r#"<path d="m5 12.5 4.5 4.5L19 7.5" stroke-width="2.2"/>"#,
            ),
        ];
        let mut glyphs = HashMap::new();
        for (name, path) in paths {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{path}</svg>"##
            );
            glyphs.insert(*name, decode(&svg));
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

fn decode(svg: &str) -> IconSource {
    let image = ImageData::from_svg(svg.as_bytes(), SvgSize::Max(128)).expect("bundled SVG");
    IconSource::Image(IconImage {
        image: image.image().clone(),
        monochrome: true,
    })
}
