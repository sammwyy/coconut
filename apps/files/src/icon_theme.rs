//! Image-decoding adapter around the shared desktop icon resolver.

use creamui_image::{ImageData, SvgSize};
use delib::icons::{system_icon_theme_with_override, IconRequest, IconResolver};
use std::{fs, path::Path};

#[derive(Clone)]
pub(crate) struct IconTheme(IconResolver);

impl IconTheme {
    pub(crate) fn system() -> Self {
        Self(IconResolver::new(system_icon_theme_with_override(Some(
            "CFILES_ICON_THEME",
        ))))
    }

    pub(crate) fn load(&self, names: &[&str]) -> Option<ImageData> {
        self.0
            .resolve(IconRequest {
                names: names.iter().map(|name| (*name).into()).collect(),
                size: 64,
                scale: 1,
            })
            .and_then(|path| load_image(&path))
    }
}

fn load_image(path: &Path) -> Option<ImageData> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        ImageData::from_svg(&fs::read(path).ok()?, SvgSize::Max(128)).ok()
    } else {
        ImageData::from_path(path).ok()
    }
}
