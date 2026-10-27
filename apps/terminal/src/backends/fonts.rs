//! Installed font families, and the face the terminal draws with.
//!
//! The settings list is every family name found in a system font file.
//! The default is the first monospace candidate whose glyphs fit one cell,
//! so a desktop that already aliases `monospace` to a tight face (KDE does)
//! keeps that face, and Windows still lands on Cascadia Mono or Consolas.

use super::select_backend;
use creamui_fonts::{cached_layout, FontFace, FontWeight, LayoutSettings};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use swash::{StringId, Style};

const FAMILY: &str = "Cterm Mono";
const SAMPLE_PX: f32 = 14.0;

struct Family {
    name: String,
    regular: PathBuf,
    regular_weight: u16,
    regular_italic: bool,
    bold: Option<PathBuf>,
    bold_weight: u16,
}

pub fn install(requested: &str) {
    let name = match family_named(requested) {
        Some(family) => family.name.clone(),
        None => default_family(),
    };
    let already = INSTALLED.with(|installed| installed.borrow().eq_ignore_ascii_case(&name));
    if already {
        return;
    }
    let Some(family) = family_named(&name) else {
        return;
    };
    if creamui_fonts::register_file(FAMILY, FontWeight::Regular, &family.regular).is_err() {
        return;
    }
    let bold = family.bold.as_ref().unwrap_or(&family.regular);
    if creamui_fonts::register_file(FAMILY, FontWeight::Bold, bold).is_err() {
        let _ = creamui_fonts::register_file(FAMILY, FontWeight::Bold, &family.regular);
    }
    INSTALLED.with(|installed| *installed.borrow_mut() = name);
}

thread_local! {
    static INSTALLED: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn default_family() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            for path in select_backend!(monospace_candidates(false)) {
                if mono_metrics(&path).is_none() {
                    continue;
                }
                if let Some(name) = family_of_path(&path) {
                    return name;
                }
            }
            catalog()
                .first()
                .map(|family| family.name.clone())
                .unwrap_or_else(|| "monospace".to_string())
        })
        .clone()
}

/// Family names for the settings dropdown. `selected` stays visible when that
/// face is no longer installed, so the saved value is not silently replaced.
pub fn font_choices(selected: &str) -> Vec<String> {
    let mut names: Vec<String> = catalog().iter().map(|family| family.name.clone()).collect();
    let selected = selected.trim();
    if !selected.is_empty() && !names.iter().any(|name| name.eq_ignore_ascii_case(selected)) {
        names.insert(0, selected.to_string());
    }
    names
}

fn family_named(name: &str) -> Option<&'static Family> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    catalog()
        .iter()
        .find(|family| family.name.eq_ignore_ascii_case(name))
}

fn family_of_path(path: &Path) -> Option<String> {
    catalog()
        .iter()
        .find(|family| {
            same_file(&family.regular, path)
                || family
                    .bold
                    .as_ref()
                    .is_some_and(|bold| same_file(bold, path))
        })
        .map(|family| family.name.clone())
        .or_else(|| read_face(path).map(|(name, _, _)| name))
}

fn catalog() -> &'static [Family] {
    static CATALOG: OnceLock<Vec<Family>> = OnceLock::new();
    CATALOG.get_or_init(scan_families)
}

fn scan_families() -> Vec<Family> {
    let mut found: Vec<Family> = Vec::new();
    for directory in select_backend!(font_directories()) {
        walk_faces(&directory, &mut found, 0);
    }
    found.sort_by_key(|family| family.name.to_ascii_lowercase());
    found
}

fn walk_faces(directory: &Path, found: &mut Vec<Family>, depth: u8) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir()) || path.is_dir();
        if is_dir {
            walk_faces(&path, found, depth + 1);
            continue;
        }
        if !is_font_file(&path) {
            continue;
        }
        let Some((name, weight, italic)) = read_face(&path) else {
            continue;
        };
        let family = if let Some(family) = found
            .iter_mut()
            .find(|family| family.name.eq_ignore_ascii_case(&name))
        {
            family
        } else {
            found.push(Family {
                name,
                regular: path.clone(),
                regular_weight: weight,
                regular_italic: italic,
                bold: None,
                bold_weight: 0,
            });
            found.last_mut().expect("the family was just inserted")
        };
        let regular_score = |weight: u16, italic: bool| {
            (weight as i32 - 400).unsigned_abs() + if italic { 10_000 } else { 0 }
        };
        if regular_score(weight, italic)
            < regular_score(family.regular_weight, family.regular_italic)
        {
            family.regular = path.clone();
            family.regular_weight = weight;
            family.regular_italic = italic;
        }
        let closer_bold = family.bold.is_none()
            || (weight as i32 - 700).unsigned_abs()
                < (family.bold_weight as i32 - 700).unsigned_abs();
        if !italic && weight >= 550 && closer_bold {
            family.bold = Some(path);
            family.bold_weight = weight;
        }
    }
}

fn read_face(path: &Path) -> Option<(String, u16, bool)> {
    let face = FontFace::from_path(path).ok()?;
    let font = face.font_ref();
    let name = family_name(&font)?;
    let attributes = font.attributes();
    let italic = !matches!(attributes.style(), Style::Normal);
    Some((name, attributes.weight().0, italic))
}

fn family_name(font: &swash::FontRef<'_>) -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for localized in font.localized_strings() {
        let Some(rank) = name_rank(localized.id(), localized.language()) else {
            continue;
        };
        if !localized.is_decodable() {
            continue;
        }
        let text: String = localized.chars().collect();
        let text = text.trim();
        if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
            continue;
        }
        if best.as_ref().is_none_or(|(score, _)| rank > *score) {
            best = Some((rank, text.to_string()));
        }
    }
    best.map(|(_, name)| name)
}

/// Typographic family outranks the legacy family name, and an English record
/// outranks a translation. Platform-unicode records have an empty language.
fn name_rank(id: StringId, language: &str) -> Option<u8> {
    let kind = match id {
        StringId::TypographicFamily => 2,
        StringId::Family => 1,
        _ => return None,
    };
    let english = language.is_empty() || language.starts_with("en");
    Some(kind * 2 + u8::from(english))
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
}

fn same_file(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

/// Advance of "M" when the face is monospace and its line box is not huge.
fn mono_metrics(path: &Path) -> Option<f32> {
    let face = FontFace::from_path(path).ok()?;
    let m = advance(&face, "M")?;
    let i = advance(&face, "i")?;
    if m < 1.0 {
        return None;
    }
    let ratio = i / m;
    let line = face.line_metrics(SAMPLE_PX).line_height / SAMPLE_PX;
    let em = m / SAMPLE_PX;
    (ratio >= 0.92 && em <= 0.72 && line <= 1.5).then_some(m)
}

fn advance(face: &FontFace, text: &str) -> Option<f32> {
    let shaped = cached_layout(face, text, SAMPLE_PX, &LayoutSettings::default());
    let advance = shaped.chars.first()?.advance;
    (advance > 0.0).then_some(advance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_terminal_face_fits_one_cell_and_is_listed() {
        let choices = font_choices("");
        let default = default_family();
        assert!(
            choices.iter().any(|name| name == &default),
            "{default} is not among the installed families"
        );
        install(&default);
        let face = creamui_fonts::resolve(FAMILY, FontWeight::Regular);
        let m = advance(&face, "M").expect("missing M");
        let i = advance(&face, "i").expect("missing i");
        assert!(
            m > 0.0 && i / m >= 0.92 && m / SAMPLE_PX <= 0.72,
            "face is sparse or proportional: i={i} M={m} family={default}"
        );
        let line = face.line_metrics(SAMPLE_PX);
        let height = (line.ascent + line.descent).max(SAMPLE_PX * 1.15);
        assert!(
            height / SAMPLE_PX <= 1.5,
            "line box is too tall: ascent={} descent={} height={height}",
            line.ascent,
            line.descent
        );
    }
}
