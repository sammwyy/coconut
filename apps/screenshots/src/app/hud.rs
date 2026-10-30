use creamui_core::{Point, Rect, Size};
use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource};

pub const ITEM: f32 = 30.0;
const GAP: f32 = 4.0;
const SEP: f32 = 10.0;
const PAD: f32 = 8.0;
const BAR_HEIGHT: f32 = ITEM + 2.0 * PAD;
const MARGIN: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Brush,
    Blur,
    Eraser,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HudId {
    Tool(Tool),
    Color(usize),
    Radius(usize),
    Copy,
    Save,
    SaveAs,
    Close,
}

pub const PALETTE: [[u8; 3]; 6] = [
    [255, 77, 77],
    [255, 169, 77],
    [255, 212, 59],
    [81, 207, 102],
    [51, 154, 240],
    [250, 250, 250],
];

pub const RADII: [f32; 3] = [3.0, 6.0, 12.0];

fn items() -> Vec<HudId> {
    let mut items = vec![
        HudId::Tool(Tool::Select),
        HudId::Tool(Tool::Brush),
        HudId::Tool(Tool::Blur),
        HudId::Tool(Tool::Eraser),
    ];
    items.extend((0..PALETTE.len()).map(HudId::Color));
    items.extend((0..RADII.len()).map(HudId::Radius));
    items.extend([HudId::Copy, HudId::Save, HudId::SaveAs, HudId::Close]);
    items
}

fn separators(item: HudId) -> bool {
    matches!(item, HudId::Color(0) | HudId::Radius(0) | HudId::Copy)
}

/// Lays the floating toolbar out next to the selection: below it when room
/// allows, above otherwise, and inside near the bottom-right corner as the
/// last resort. Without a selection it docks to the bottom-center edge.
pub fn layout(selection: Option<Rect>, viewport: Size) -> (Rect, Vec<(HudId, Rect)>) {
    let items = items();
    let width = items.len() as f32 * (ITEM + GAP) - GAP + 3.0 * SEP + 2.0 * PAD;
    let height = BAR_HEIGHT;
    let (x, y) = match selection {
        Some(selection) => {
            let mut x = selection.x + selection.width / 2.0 - width / 2.0;
            x = x.clamp(MARGIN, (viewport.width - width - MARGIN).max(MARGIN));
            let below = selection.y + selection.height + MARGIN;
            let above = selection.y - MARGIN - height;
            let y = if below + height <= viewport.height - 4.0 {
                below
            } else if above >= 4.0 {
                above
            } else {
                (viewport.height - height - MARGIN).max(MARGIN)
            };
            (x, y)
        }
        None => (
            ((viewport.width - width) / 2.0).max(MARGIN),
            (viewport.height - height - 18.0).max(MARGIN),
        ),
    };
    let mut cursor = x + PAD;
    let mut rects = Vec::with_capacity(items.len());
    for item in items {
        if separators(item) {
            cursor += SEP - GAP;
        }
        rects.push((
            item,
            Rect {
                x: cursor,
                y: y + PAD,
                width: ITEM,
                height: ITEM,
            },
        ));
        cursor += ITEM + GAP;
    }
    (
        Rect {
            x,
            y,
            width,
            height,
        },
        rects,
    )
}

pub fn hit(items: &[(HudId, Rect)], point: Point) -> Option<HudId> {
    items
        .iter()
        .find(|(_, rect)| rect.contains(point))
        .map(|(id, _)| *id)
}

pub struct HudIcons {
    pub select: IconSource,
    pub brush: IconSource,
    pub blur: IconSource,
    pub eraser: IconSource,
    pub copy: IconSource,
    pub save: IconSource,
    pub save_as: IconSource,
    pub close: IconSource,
}

impl HudIcons {
    pub fn load() -> Self {
        Self {
            select: outline(r#"<path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.5L3 3z"/>"#),
            brush: outline(
                r#"<path d="m9.06 11.9 8.07-8.06a2.85 2.85 0 1 1 4.03 4.03l-8.06 8.08"/><path d="M7.07 14.94c-1.66 0-3 1.35-3 3.02 0 1.33-2.5 1.52-2 2.02 1.08 1.1 2.49 2.02 4 2.02 2.2 0 4-1.8 4-4.04a3.01 3.01 0 0 0-3-3.02z"/>"#,
            ),
            blur: outline(
                r#"<path d="M7 16.3c2.2 0 4-1.83 4-4.05 0-1.16-.57-2.26-1.71-3.19S7.29 6.75 7 5.3c-.29 1.45-1.14 2.84-2.29 3.76S3 11.1 3 12.25c0 2.22 1.8 4.05 4 4.05z"/><path d="M12.56 6.6A10.97 10.97 0 0 0 14 3.02c.5 2.5 2 4.9 4 6.5s3 3.5 3 5.5a6.98 6.98 0 0 1-11.91 4.97"/>"#,
            ),
            eraser: outline(
                r#"<path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21"/><path d="M22 21H7"/><path d="m5 11 9 9"/>"#,
            ),
            copy: outline(
                r#"<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#,
            ),
            save: outline(
                r#"<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/>"#,
            ),
            save_as: outline(
                r#"<path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7l-2-2H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2z"/><path d="M12 10v6m0 0-2.5-2.5M12 16l2.5-2.5"/>"#,
            ),
            close: outline(r#"<path d="m7 7 10 10M17 7 7 17"/>"#),
        }
    }
}

fn outline(paths: &str) -> IconSource {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>"##
    );
    match ImageData::from_svg(svg.as_bytes(), SvgSize::Max(64)) {
        Ok(image) => IconSource::Image(IconImage {
            image: image.image().clone(),
            monochrome: true,
        }),
        Err(error) => {
            eprintln!("coconut-screenshots: failed to decode a bundled icon: {error}");
            IconSource::Symbol(creamui_widgets::Symbol::Check)
        }
    }
}
