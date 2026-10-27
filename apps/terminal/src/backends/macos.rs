//! macOS. PTY hangup and the executable bit match Unix; fonts and the shell do not.

use super::unix::{
    self, fontconfig_candidates, is_executable_file as unix_executable, passthrough_startup,
    reap_after_eof, signal_hangup,
};
use super::{missing_shell, Backend, Startup};
use portable_pty::{Child, MasterPty};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct MacOs;

impl Backend for MacOs {
    fn prepare_display() {}
    fn default_shell() -> &'static str {
        "zsh"
    }
    fn fallback_shells() -> &'static [&'static str] {
        &["zsh", "bash", "sh"]
    }
    fn shell_missing_message(shell: &str) -> String {
        missing_shell(shell, Self::fallback_shells())
    }
    fn executable_candidates(path: &Path) -> Vec<PathBuf> {
        unix::executable_candidates(path)
    }
    fn is_executable_file(metadata: &Metadata) -> bool {
        unix_executable(metadata)
    }
    fn monospace_candidates(bold: bool) -> Vec<PathBuf> {
        let mut paths = bundled_fonts(bold);
        for path in fontconfig_candidates(bold) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        paths
    }
    fn font_directories() -> Vec<PathBuf> {
        let mut directories = vec![
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            directories.push(PathBuf::from(home).join("Library/Fonts"));
        }
        for directory in unix::font_directories() {
            if !directories.contains(&directory) {
                directories.push(directory);
            }
        }
        directories
    }
    fn open_url(url: &str) -> Command {
        let mut command = Command::new("open");
        command.arg(url);
        command
    }
    fn notification(title: &str, body: &str) -> Command {
        let escape = |value: &str| value.replace('\\', "\\\\").replace('"', "\\\"");
        let mut command = Command::new("osascript");
        command.args([
            "-e",
            &format!(
                "display notification \"{}\" with title \"{}\"",
                escape(body),
                escape(title)
            ),
        ]);
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

fn bundled_fonts(bold: bool) -> Vec<PathBuf> {
    let names: &[&str] = if bold {
        &[
            "/System/Library/Fonts/Menlo.ttc",
            "/System/Library/Fonts/Supplemental/Courier New Bold.ttf",
            "/Library/Fonts/Courier New Bold.ttf",
        ]
    } else {
        &[
            "/System/Library/Fonts/Menlo.ttc",
            "/System/Library/Fonts/Monaco.ttf",
            "/System/Library/Fonts/Supplemental/Courier New.ttf",
        ]
    };
    names
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .collect()
}
