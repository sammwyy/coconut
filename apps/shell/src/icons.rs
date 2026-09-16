use creamui_core::BoxedWidget;
use creamui_image::{ImageData, SvgSize};
use creamui_theme::Color;
use creamui_widgets::{Icon, IconImage, IconSource};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Bundled icons are rasterized once, at a size generous enough for any
/// on-screen use, then tinted to whatever `color` a call site asks for —
/// unlike a build-time bake to a fixed color, the same decoded icon adapts
/// if it's later drawn against a different theme or state.
const RASTER_SIZE: u32 = 128;

pub fn pixel_icon(name: &str, size: f32, color: Color) -> BoxedWidget {
    Box::new(Icon::new(icon_source(name), color).size(size))
}

fn icon_source(name: &str) -> IconSource {
    // `IconSource::Image` holds an `Rc`, so this cache — like the rest of
    // this `Rc`-based UI tree — is confined to the render thread that
    // builds widgets, rather than a process-wide `static`.
    thread_local! {
        static CACHE: RefCell<HashMap<String, IconSource>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| {
        if let Some(source) = cache.borrow().get(name) {
            return source.clone();
        }
        let bytes =
            registered_icon_bytes(name).unwrap_or_else(|| panic!("missing bundled icon: {name}"));
        let source = decode(bytes);
        cache.borrow_mut().insert(name.to_owned(), source.clone());
        source
    })
}

fn decode(bytes: &'static [u8]) -> IconSource {
    let image =
        ImageData::from_svg(bytes, SvgSize::Max(RASTER_SIZE)).expect("bundled icon must decode");
    IconSource::Image(IconImage {
        width: image.width(),
        height: image.height(),
        rgba: Rc::from(image.pixels()),
        monochrome: true,
    })
}

include!(concat!(env!("OUT_DIR"), "/icons_registry.rs"));
