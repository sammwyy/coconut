use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
    let output = PathBuf::from(env::var("OUT_DIR").expect("build output directory must exist"));
    let mut icons = Vec::new();
    collect_icons(&source_dir, &mut icons);
    icons.sort();
    let mut names = HashSet::new();
    let mut registry = String::from(
        "fn registered_icon_bytes(name: &str) -> Option<&'static [u8]> {\n    match name {\n",
    );
    for source in icons {
        println!("cargo:rerun-if-changed={}", source.display());
        let name = source
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("icon name must be valid UTF-8");
        assert!(names.insert(name.to_owned()), "duplicate icon name: {name}");
        // Absolute so `include_bytes!` resolves regardless of where the
        // generated file (under `OUT_DIR`) ends up; forward slashes so the
        // path is a valid string literal on Windows too.
        let path = source.to_string_lossy().replace('\\', "/");
        registry.push_str(&format!(
            "        \"{name}\" => Some(include_bytes!(\"{path}\")),\n"
        ));
    }
    registry.push_str("        _ => None,\n    }\n}\n");
    fs::write(output.join("icons_registry.rs"), registry)
        .expect("icon registry must be written into build output");
}

/// Icons are organized into per-category subfolders, so this walks
/// `assets/icons` recursively; every icon is still registered by its bare
/// file stem, wherever it lives in that tree.
fn collect_icons(dir: &Path, icons: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir)
        .expect("icon source directory must be readable")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            collect_icons(&path, icons);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("svg") {
            println!("cargo:rerun-if-changed={}", path.display());
            icons.push(path);
        }
    }
}
