//! Windows. ConPTY needs a cursor-position reply before it will run the child,
//! and it does not close the output pipe when that child exits.

use super::{missing_shell, Backend, Startup};
use portable_pty::{Child, MasterPty};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

pub struct Windows;

impl Backend for Windows {
    fn prepare_display() {}
    fn default_shell() -> &'static str {
        "powershell"
    }
    fn fallback_shells() -> &'static [&'static str] {
        &["powershell", "pwsh", "cmd"]
    }
    fn shell_missing_message(shell: &str) -> String {
        missing_shell(shell, Self::fallback_shells())
    }
    fn executable_candidates(path: &Path) -> Vec<PathBuf> {
        if path.extension().is_some() {
            return vec![path.to_path_buf()];
        }
        // CreateProcess requires an extension. Search the same suffixes as cmd.
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        pathext
            .split(';')
            .map(str::trim)
            .filter(|ext| !ext.is_empty())
            .map(|ext| {
                let mut file = path.as_os_str().to_owned();
                file.push(".");
                file.push(ext.trim_start_matches('.'));
                PathBuf::from(file)
            })
            .collect()
    }
    fn is_executable_file(metadata: &Metadata) -> bool {
        metadata.is_file()
    }
    fn monospace_candidates(bold: bool) -> Vec<PathBuf> {
        let names: &[&str] = if bold {
            &[
                "consolab.ttf",
                "CascadiaMono.ttf",
                "CascadiaCode.ttf",
                "lucon.ttf",
            ]
        } else {
            &[
                "CascadiaMono.ttf",
                "consola.ttf",
                "CascadiaCode.ttf",
                "lucon.ttf",
            ]
        };
        let mut found = Vec::new();
        for name in names {
            for directory in font_dirs() {
                let path = directory.join(name);
                if path.is_file() && !found.contains(&path) {
                    found.push(path);
                }
            }
        }
        found
    }
    fn font_directories() -> Vec<PathBuf> {
        font_dirs()
    }
    fn open_url(url: &str) -> Command {
        let mut command = Command::new("cmd");
        // The first quoted argument of `start` is a window title.
        command.args(["/C", "start", "", url]);
        command
    }
    fn notification(title: &str, body: &str) -> Command {
        let mut command = Command::new("msg");
        command.args(["*", &format!("{title}: {body}")]);
        command
    }
    fn take_startup(pending: &mut Vec<u8>) -> Startup {
        if let Some(pos) = pending.windows(4).position(|window| window == b"\x1b[6n") {
            let rest = pending.split_off(pos + 4);
            pending.truncate(pos);
            pending.extend_from_slice(&rest);
            return Startup::Ready {
                reply: Some(b"\x1b[1;1R".to_vec()),
            };
        }
        if pending.len() >= 64 {
            Startup::Ready { reply: None }
        } else {
            Startup::NeedMore
        }
    }
    fn is_startup_reply(bytes: &[u8]) -> bool {
        bytes == b"\x1b[1;1R"
    }
    fn waits_for_pty_startup() -> bool {
        true
    }
    fn watch_child(
        mut child: Box<dyn Child + Send + Sync>,
        on_exit: Box<dyn FnOnce(String) + Send>,
    ) -> Option<Box<dyn Child + Send + Sync>> {
        thread::spawn(move || {
            let status = match child.wait() {
                Ok(status) => status.to_string(),
                Err(error) => format!("Process ended: {error}"),
            };
            on_exit(status);
        });
        None
    }
    fn hangup(_master: &dyn MasterPty) {}
    #[cfg(test)]
    fn idle_cwd() -> PathBuf {
        std::env::temp_dir()
    }
    #[cfg(test)]
    fn idle_command() -> Vec<String> {
        vec!["cmd.exe".into(), "/d".into(), "/k".into()]
    }
}

fn font_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        directories.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        directories.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    directories
}
