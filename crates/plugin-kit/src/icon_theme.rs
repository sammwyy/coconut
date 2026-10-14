//! CreamUI image adapter for delib's shared Freedesktop resolver.

use creamui_image::{ImageData, SvgSize};
use std::{fs, path::Path};

pub use delib::icons::{installed_icon_themes, xdg_data_directories, IconRequest};

#[derive(Clone)]
pub struct IconResolver(delib::icons::IconResolver);

impl IconResolver {
    pub fn new(theme: impl Into<String>) -> Self {
        Self(delib::icons::IconResolver::new(theme))
    }

    pub fn resolve(&self, request: IconRequest) -> Option<std::path::PathBuf> {
        self.0.resolve(request)
    }

    pub fn load(&self, request: IconRequest) -> Option<ImageData> {
        let size = request.size.saturating_mul(request.scale.max(1));
        self.resolve(request)
            .and_then(|path| load_icon(&path, size))
    }
}

pub fn load_icon(path: &Path, size: u32) -> Option<ImageData> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        ImageData::from_svg(&fs::read(path).ok()?, SvgSize::Max(size)).ok()
    } else {
        ImageData::from_path(path).ok()
    }
}
