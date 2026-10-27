use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const HELP: &str = "\
Media Player — native video and music player

Usage: cplayer [FILE|DIRECTORY ...]
       cplayer --player [FILE|DIRECTORY ...]
       cplayer --settings
       cplayer --quit
       cplayer --help | --version

Directories are scanned for playable files. With no arguments the player
opens on its welcome screen; files can be added from there or with 'o'.

Keyboard: space play/pause · ← → seek 5s (shift 30s) · ↑ ↓ volume ·
f maximize · m mute · [ ] speed · n p next/prev · l repeat · s shuffle ·
o open · , settings · q close window · --quit stop player";

pub enum Launch {
    Play(Vec<PathBuf>),
    Settings,
    Quit,
    Help,
    Version,
}

pub fn is_media(path: &Path) -> bool {
    crate::MEDIA_EXTENSIONS.iter().any(|ext| {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    })
}

fn expand(path: PathBuf, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut children: Vec<PathBuf> = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_media(p))
            .collect();
        children.sort();
        out.extend(children);
    } else {
        out.push(path);
    }
}

pub fn collect_media(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        expand(path, &mut out);
    }
    out.into_iter()
        .filter(|p| p.is_file() && is_media(p))
        .collect()
}

pub fn parse(args: impl Iterator<Item = OsString>) -> Result<Launch, String> {
    let mut paths = Vec::new();
    let mut bare = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if !bare && text.starts_with('-') {
            match text.as_ref() {
                "-h" | "--help" => return Ok(Launch::Help),
                "-V" | "--version" => return Ok(Launch::Version),
                "--settings" => return Ok(Launch::Settings),
                "--player" => {}
                "--quit" => return Ok(Launch::Quit),
                "--" => bare = true,
                other => return Err(format!("unknown option '{other}' (try --help)")),
            }
        } else {
            expand(PathBuf::from(&*arg), &mut paths);
        }
    }
    Ok(Launch::Play(paths))
}

pub fn is_video(path: &Path) -> bool {
    const VIDEO: &[&str] = &[
        "mp4", "m4v", "mkv", "webm", "avi", "mov", "ts", "mts", "m2ts", "mpg", "mpeg", "wmv",
        "flv", "3gp", "ogv",
    ];
    VIDEO.iter().any(|ext| {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    })
}

pub fn kind_label(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_uppercase();
    let kind = if is_video(path) { "Video" } else { "Audio" };
    if ext.is_empty() {
        kind.into()
    } else {
        format!("{kind} · {ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_modes_have_distinct_flags() {
        assert!(
            matches!(parse([OsString::from("--player")].into_iter()), Ok(Launch::Play(paths)) if paths.is_empty())
        );
        assert!(matches!(
            parse([OsString::from("--settings")].into_iter()),
            Ok(Launch::Settings)
        ));
        assert!(matches!(
            parse([OsString::from("--quit")].into_iter()),
            Ok(Launch::Quit)
        ));
    }
}
