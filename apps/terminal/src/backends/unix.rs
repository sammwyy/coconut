//! Linux and other Unix systems. macOS reuses the PTY and fontconfig helpers.

use super::{missing_shell, Backend, Startup};
use portable_pty::{Child, MasterPty};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(not(target_os = "macos"))]
pub struct Unix;

#[cfg(not(target_os = "macos"))]
impl Backend for Unix {
    fn prepare_display() {
        discover_wayland_display();
    }
    fn default_shell() -> &'static str {
        "bash"
    }
    fn fallback_shells() -> &'static [&'static str] {
        &["sh", "bash"]
    }
    fn shell_missing_message(shell: &str) -> String {
        missing_shell(shell, Self::fallback_shells())
    }
    fn executable_candidates(path: &Path) -> Vec<PathBuf> {
        executable_candidates(path)
    }
    fn is_executable_file(metadata: &Metadata) -> bool {
        is_executable_file(metadata)
    }
    fn monospace_candidates(bold: bool) -> Vec<PathBuf> {
        fontconfig_candidates(bold)
    }
    fn font_directories() -> Vec<PathBuf> {
        font_directories()
    }
    fn open_url(url: &str) -> Command {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
    fn notification(title: &str, body: &str) -> Command {
        let mut command = Command::new("notify-send");
        command.args(["--app-name", "cterm", title, body]);
        command
    }
    fn take_startup(pending: &mut Vec<u8>) -> Startup {
        passthrough_startup(pending)
    }
    fn is_startup_reply(_: &[u8]) -> bool {
        false
    }
    fn waits_for_pty_startup() -> bool {
        false
    }
    fn watch_child(
        child: Box<dyn Child + Send + Sync>,
        on_exit: Box<dyn FnOnce(String) + Send>,
    ) -> Option<Box<dyn Child + Send + Sync>> {
        reap_after_eof(child, on_exit)
    }
    fn hangup(master: &dyn MasterPty) {
        signal_hangup(master);
    }
    #[cfg(test)]
    fn idle_cwd() -> PathBuf {
        PathBuf::from("/tmp")
    }
    #[cfg(test)]
    fn idle_command() -> Vec<String> {
        vec!["/bin/cat".into()]
    }
}

pub(super) fn discover_wayland_display() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return;
    }
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    use std::os::unix::fs::FileTypeExt;
    let mut sockets = std::fs::read_dir(runtime)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_socket()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("wayland-"))
        .collect::<Vec<_>>();
    sockets.sort();
    if let Some(socket) = sockets.first() {
        std::env::set_var("WAYLAND_DISPLAY", socket);
    }
}

pub(super) fn executable_candidates(path: &Path) -> Vec<PathBuf> {
    vec![path.to_path_buf()]
}

pub(super) fn is_executable_file(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

pub(super) fn passthrough_startup(pending: &mut Vec<u8>) -> Startup {
    let _ = pending;
    Startup::Ready { reply: None }
}

pub(super) fn reap_after_eof(
    child: Box<dyn Child + Send + Sync>,
    on_exit: Box<dyn FnOnce(String) + Send>,
) -> Option<Box<dyn Child + Send + Sync>> {
    let _ = on_exit;
    Some(child)
}

pub(super) fn signal_hangup(master: &dyn MasterPty) {
    if let Some(group) = master.process_group_leader().filter(|group| *group > 0) {
        // SAFETY: openpty/spawn_command creates a separate session. Signal its
        // foreground job group as a real terminal hangup would.
        unsafe {
            libc::kill(-group, libc::SIGHUP);
        }
    }
}

const PREFERRED_MONO: &[&str] = &[
    "JetBrains Mono",
    "Hack",
    "Cascadia Mono",
    "Fira Mono",
    "Source Code Pro",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "Ubuntu Mono",
    "Inconsolata",
];

const REGULAR_STEMS: &[&str] = &[
    "jetbrainsmono-regular",
    "hack-regular",
    "cascadiamono",
    "firamono-regular",
    "sourcecodepro-regular",
    "dejavusansmono",
    "liberationmono-regular",
    "notosansmono-regular",
    "ubuntumono-r",
    "inconsolata-regular",
];

const BOLD_STEMS: &[&str] = &[
    "jetbrainsmono-bold",
    "hack-bold",
    "cascadiamono",
    "firamono-bold",
    "sourcecodepro-bold",
    "dejavusansmono-bold",
    "liberationmono-bold",
    "notosansmono-bold",
    "ubuntumono-b",
    "inconsolata-bold",
];

pub(super) fn fontconfig_candidates(bold: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let generic = if bold {
        "monospace:weight=bold"
    } else {
        "monospace"
    };
    // KDE's fontconfig alias resolves `monospace` to a tight face. A stock
    // Fedora config often resolves it to Nimbus Mono or a CJK fallback, which
    // is the sparse look. Keep the alias only when it is a real text face.
    let generic_match = fc_match(generic);
    if let Some((path, family, spacing)) = &generic_match {
        if acceptable_generic(family, spacing) {
            push_unique(&mut paths, path.clone());
        }
    }
    for family in PREFERRED_MONO {
        let pattern = if bold {
            format!("{family}:weight=bold")
        } else {
            (*family).to_string()
        };
        if let Some((path, matched, _)) = fc_match(&pattern) {
            if family_matches(&matched, family) {
                push_unique(&mut paths, path);
            }
        }
    }
    let stems = if bold { BOLD_STEMS } else { REGULAR_STEMS };
    for path in scan_font_files(stems) {
        push_unique(&mut paths, path);
    }
    // Last resort: a loose monospace still beats the proportional UI face.
    if let Some((path, _, _)) = generic_match {
        push_unique(&mut paths, path);
    }
    paths
}

/// fontconfig spacing: 100 is monospace, 110 is charcell.
fn acceptable_generic(family: &str, spacing: &str) -> bool {
    let family = family.to_ascii_lowercase();
    if family.contains("cjk") || family.contains("nimbus") || family.contains("courier") {
        return false;
    }
    spacing.is_empty() || spacing == "100" || spacing == "110"
}

fn family_matches(matched: &str, requested: &str) -> bool {
    if matched.to_ascii_lowercase().contains("cjk") {
        return false;
    }
    matched
        .split(',')
        .any(|name| name.trim().eq_ignore_ascii_case(requested))
}

fn fc_match(pattern: &str) -> Option<(PathBuf, String, String)> {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}\t%{family}\t%{spacing}", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let mut fields = text.trim().split('\t');
    let file = fields.next()?.trim();
    if file.is_empty() {
        return None;
    }
    let family = fields.next().unwrap_or("").trim().to_string();
    let spacing = fields.next().unwrap_or("").trim().to_string();
    Some((PathBuf::from(file), family, spacing))
}

fn scan_font_files(stems: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in font_roots() {
        walk_fonts(&root, stems, &mut found, 0);
    }
    let mut ordered = Vec::new();
    for stem in stems {
        if let Some(path) = found.iter().find(|(name, _)| name == stem) {
            ordered.push(path.1.clone());
        }
    }
    ordered
}

pub(super) fn font_directories() -> Vec<PathBuf> {
    font_roots()
}

fn font_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".local/share/fonts"));
        roots.push(home.join(".fonts"));
    }
    roots
}

fn walk_fonts(directory: &Path, stems: &[&str], found: &mut Vec<(String, PathBuf)>, depth: u8) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir()) || path.is_dir();
        if is_dir {
            walk_fonts(&path, stems, found, depth + 1);
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let stem = stem.to_ascii_lowercase();
        if stems.contains(&stem.as_str()) {
            found.push((stem, path));
        }
    }
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}
