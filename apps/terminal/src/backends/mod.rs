//! Platform backends. `select_backend!` binds every call to the crate compiled
//! for this target, so the rest of the program does not branch on the OS.

mod fonts;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use portable_pty::{Child, MasterPty};
use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// What to do with the bytes read before the child has produced a normal frame.
pub enum Startup {
    /// Keep the bytes. The query may be split across reads.
    #[cfg_attr(
        not(windows),
        expect(dead_code, reason = "only ConPTY needs a split startup query")
    )]
    NeedMore,
    /// `pending` is the output to deliver. `reply` is written to the PTY first.
    Ready { reply: Option<Vec<u8>> },
}

pub trait Backend {
    fn prepare_display();
    fn default_shell() -> &'static str;
    fn fallback_shells() -> &'static [&'static str];
    fn shell_missing_message(shell: &str) -> String {
        missing_shell(shell, Self::fallback_shells())
    }
    fn executable_candidates(path: &Path) -> Vec<PathBuf>;
    fn is_executable_file(metadata: &Metadata) -> bool;
    /// Ordered font files. The first one with real monospace metrics is used.
    fn monospace_candidates(bold: bool) -> Vec<PathBuf>;
    /// Directories that hold installed font files. Subdirectories are included.
    fn font_directories() -> Vec<PathBuf>;
    fn open_url(url: &str) -> std::process::Command;
    fn notification(title: &str, body: &str) -> std::process::Command;
    fn take_startup(pending: &mut Vec<u8>) -> Startup;
    fn is_startup_reply(bytes: &[u8]) -> bool;
    /// ConPTY must be answered before the first resize, or the UI thread blocks.
    fn waits_for_pty_startup() -> bool;
    /// `None` means a background thread owns the child and reports the exit.
    fn watch_child(
        child: Box<dyn Child + Send + Sync>,
        on_exit: Box<dyn FnOnce(String) + Send>,
    ) -> Option<Box<dyn Child + Send + Sync>>;
    fn hangup(master: &dyn MasterPty);
    #[cfg(test)]
    fn idle_cwd() -> PathBuf;
    #[cfg(test)]
    fn idle_command() -> Vec<String>;
}

/// Compile-time dispatch. Inactive platforms are not type-checked.
macro_rules! select_backend {
    ($method:ident ($($arg:expr),* $(,)?)) => {{
        #[cfg(windows)]
        {
            <$crate::backends::windows::Windows as $crate::backends::Backend>::$method($($arg),*)
        }
        #[cfg(target_os = "macos")]
        {
            <$crate::backends::macos::MacOs as $crate::backends::Backend>::$method($($arg),*)
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            <$crate::backends::unix::Unix as $crate::backends::Backend>::$method($($arg),*)
        }
    }};
}
pub(crate) use select_backend;

pub fn missing_shell(shell: &str, fallbacks: &[&str]) -> String {
    match fallbacks {
        [] => format!("Couldn't find {shell:?} in PATH"),
        [only] => format!("Couldn't find {shell:?} or {only} in PATH"),
        [head @ .., last] => {
            format!(
                "Couldn't find {shell:?}, {} or {last} in PATH",
                head.join(", ")
            )
        }
    }
}

pub fn prepare_display() {
    select_backend!(prepare_display());
}
pub fn default_shell() -> &'static str {
    select_backend!(default_shell())
}
pub fn fallback_shells() -> &'static [&'static str] {
    select_backend!(fallback_shells())
}
pub fn shell_missing_message(shell: &str) -> String {
    select_backend!(shell_missing_message(shell))
}
pub fn executable_candidates(path: &Path) -> Vec<PathBuf> {
    select_backend!(executable_candidates(path))
}
pub fn is_executable_file(metadata: &Metadata) -> bool {
    select_backend!(is_executable_file(metadata))
}
pub fn open_url(url: &str) -> std::process::Command {
    select_backend!(open_url(url))
}
pub fn notify(title: &str, body: &str) {
    if let Ok(mut child) = select_backend!(notification(title, body)).spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}
pub fn install_terminal_font(family: &str) {
    fonts::install(family);
}
pub fn default_font() -> String {
    fonts::default_family()
}
pub fn font_choices(selected: &str) -> Vec<String> {
    fonts::font_choices(selected)
}
pub fn take_startup(pending: &mut Vec<u8>) -> Startup {
    select_backend!(take_startup(pending))
}
pub fn is_startup_reply(bytes: &[u8]) -> bool {
    select_backend!(is_startup_reply(bytes))
}
pub fn waits_for_pty_startup() -> bool {
    select_backend!(waits_for_pty_startup())
}
pub fn watch_child(
    child: Box<dyn Child + Send + Sync>,
    on_exit: Box<dyn FnOnce(String) + Send>,
) -> Option<Box<dyn Child + Send + Sync>> {
    select_backend!(watch_child(child, on_exit))
}
pub fn hangup(master: &dyn MasterPty) {
    select_backend!(hangup(master));
}
#[cfg(test)]
pub(crate) fn idle_cwd() -> PathBuf {
    select_backend!(idle_cwd())
}
#[cfg(test)]
pub(crate) fn idle_command() -> Vec<String> {
    select_backend!(idle_command())
}
