//! PTY lifetime, bounded output delivery and VT state. No GUI work happens on workers.
use anyhow::{Context, Result};
use creamui_widgets::TextSelection;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, SyncSender},
    thread,
    time::{Duration, Instant},
};

pub const SCROLLBACK: usize = 10_000;

pub fn output_text(screen: &vt100::Screen) -> String {
    let mut snapshot = screen.clone();
    snapshot.set_scrollback(usize::MAX);
    let history = snapshot.scrollback();
    let (rows, cols) = snapshot.size();
    let total = history + rows as usize;
    let mut result = String::new();
    let mut base = 0;
    while base < total {
        let top = base.min(history);
        snapshot.set_scrollback(history - top);
        let first = base - top;
        let count = (rows as usize - first).min(total - base);
        for row in first..first + count {
            let wrapped = snapshot.row_wrapped(row as u16);
            let mut line = String::new();
            for col in 0..cols {
                let Some(cell) = snapshot.cell(row as u16, col) else {
                    continue;
                };
                if !cell.is_wide_continuation() {
                    line.push_str(if cell.has_contents() {
                        cell.contents()
                    } else {
                        " "
                    });
                }
            }
            if wrapped {
                result.push_str(&line);
            } else {
                result.push_str(line.trim_end());
                if base + row - first + 1 < total {
                    result.push('\n');
                }
            }
        }
        base += count;
    }
    result
}

pub fn save_output(path: &Path, screen: &vt100::Screen) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid output path"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".cterm-output-{}.tmp", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(output_text(screen).as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub environment: BTreeMap<String, String>,
    pub scrollback: usize,
    pub theme: crate::config::TerminalTheme,
}

#[derive(Default)]
pub struct Palette {
    pub query: String,
    pub cursor: usize,
    pub selection: TextSelection,
}
impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            environment: BTreeMap::new(),
            scrollback: SCROLLBACK,
            theme: crate::config::TerminalTheme::Default,
        }
    }
}

/// Working directory for tests that only need a live PTY, on either platform.
#[cfg(test)]
pub(crate) fn idle_cwd() -> std::path::PathBuf {
    crate::backends::idle_cwd()
}

/// A process that stays open so tests can drive the VT parser themselves.
#[cfg(test)]
pub(crate) fn idle_command() -> Vec<String> {
    crate::backends::idle_command()
}
pub enum Event {
    Output(u64, Vec<u8>),
    Exited(u64, String),
    Configuration(Result<crate::config::Config, String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellMarker {
    PromptStart,
    PromptEnd,
    CommandOutputStart,
    CommandFinished(Option<i32>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBoundary {
    pub marker: ShellMarker,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct CompletedCommand {
    pub status: Option<i32>,
    pub duration: Duration,
}

#[derive(Default)]
pub struct Callbacks {
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub replies: Vec<u8>,
    pub boundaries: Vec<CommandBoundary>,
}
impl vt100::Callbacks for Callbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title)
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
    }
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        prefix: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        action: char,
    ) {
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        let reply = match (prefix, action, first) {
            (None, 'n', 5) => "\x1b[0n".into(),
            (None | Some(b'?'), 'n', 6) => {
                let (row, col) = screen.cursor_position();
                format!(
                    "\x1b[{}{};{}R",
                    if prefix.is_some() { "?" } else { "" },
                    row + 1,
                    col + 1
                )
            }
            (None, 'c', 0) => "\x1b[?1;2c".into(),
            (Some(b'>'), 'c', 0) => "\x1b[>0;1;0c".into(),
            (None, 't', 18) => {
                let (r, c) = screen.size();
                format!("\x1b[8;{r};{c}t")
            }
            _ => String::new(),
        };
        self.replies.extend_from_slice(reply.as_bytes());
    }
    fn unhandled_osc(&mut self, screen: &mut vt100::Screen, params: &[&[u8]]) {
        if let [b"7", value] = params {
            self.cwd = parse_osc7(value);
        }
        if let Some(marker) = parse_shell_marker(params) {
            let mut history = screen.clone();
            history.set_scrollback(usize::MAX);
            self.boundaries.push(CommandBoundary {
                marker,
                line: history.scrollback() + screen.cursor_position().0 as usize,
            });
        }
    }
}

fn parse_shell_marker(params: &[&[u8]]) -> Option<ShellMarker> {
    let [b"133", marker, rest @ ..] = params else {
        return None;
    };
    Some(match *marker {
        b"A" => ShellMarker::PromptStart,
        b"B" => ShellMarker::PromptEnd,
        b"C" => ShellMarker::CommandOutputStart,
        b"D" => ShellMarker::CommandFinished(
            rest.first()
                .and_then(|status| std::str::from_utf8(status).ok())
                .and_then(|status| status.parse().ok()),
        ),
        _ => return None,
    })
}

fn parse_osc7(value: &[u8]) -> Option<PathBuf> {
    let value = std::str::from_utf8(value).ok()?;
    let location = value.strip_prefix("file://")?;
    let (host, path) = location.split_once('/')?;
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    let mut decoded = Vec::with_capacity(path.len() + 1);
    decoded.push(b'/');
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let path = PathBuf::from(std::str::from_utf8(&decoded).ok()?);
    path.is_absolute().then_some(path)
}

pub struct Session {
    pub id: u64,
    pub cwd: PathBuf,
    pub theme: crate::config::TerminalTheme,
    pub parser: vt100::Parser<Callbacks>,
    pub exited: Option<String>,
    pub search: crate::search::Search,
    pub control_mode: Option<crate::island::Mode>,
    pub pending_paste: Option<String>,
    pub palette: Palette,
    pub manual_title: Option<String>,
    pub command_boundaries: Vec<CommandBoundary>,
    completed_commands: Vec<CompletedCommand>,
    command_started: Option<Instant>,
    master: Box<dyn MasterPty + Send>,
    input: Option<SyncSender<Vec<u8>>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}
impl Session {
    #[cfg(test)]
    pub fn spawn(
        id: u64,
        cwd: &Path,
        command: &[String],
        events: SyncSender<Event>,
    ) -> Result<Self> {
        Self::spawn_with(id, cwd, command, &SessionOptions::default(), events)
    }
    pub fn spawn_with(
        id: u64,
        cwd: &Path,
        command: &[String],
        options: &SessionOptions,
        events: SyncSender<Event>,
    ) -> Result<Self> {
        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let mut cmd = if let Some(program) = command.first() {
            let mut cmd = CommandBuilder::new(program);
            cmd.args(&command[1..]);
            cmd
        } else {
            let shell = crate::config::Config::default()
                .shell_command()
                .map_err(anyhow::Error::msg)?;
            CommandBuilder::new(&shell[0])
        };
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", env!("CARGO_PKG_NAME"));
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        for (key, value) in &options.environment {
            cmd.env(key, value);
        }
        // An inherited size would override the PTY's negotiated dimensions in some shells.
        cmd.env_remove("LINES");
        cmd.env_remove("COLUMNS");
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("couldn't start the terminal process")?;
        let killer = child.clone_killer();
        drop(pair.slave);
        let (input, incoming) = mpsc::sync_channel::<Vec<u8>>(64);
        let (handshake_tx, handshake_rx) = mpsc::channel::<()>();
        let mut handshake_tx = Some(handshake_tx);
        thread::spawn(move || {
            while let Ok(bytes) = incoming.recv() {
                let answered = crate::backends::is_startup_reply(&bytes);
                if writer
                    .write_all(&bytes)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
                if answered {
                    if let Some(done) = handshake_tx.take() {
                        let _ = done.send(());
                    }
                }
            }
        });
        let reply = input.clone();
        let (startup_tx, startup_rx) = mpsc::channel::<bool>();
        let exit_events = events.clone();
        let child = crate::backends::watch_child(
            child,
            Box::new(move |status| {
                let _ = exit_events.send(Event::Exited(id, status));
            }),
        );
        thread::spawn(move || {
            let mut buffer = [0; 16 * 1024];
            let mut startup = Some(Vec::new());
            let mut startup_tx = Some(startup_tx);
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        if let Some(done) = startup_tx.take() {
                            let _ = done.send(false);
                        }
                        break;
                    }
                    Ok(n) => {
                        if let Some(pending) = startup.as_mut() {
                            pending.extend_from_slice(&buffer[..n]);
                            match crate::backends::take_startup(pending) {
                                crate::backends::Startup::NeedMore => continue,
                                crate::backends::Startup::Ready { reply: answer } => {
                                    let answered = answer.is_some();
                                    if let Some(answer) = answer {
                                        let _ = reply.send(answer);
                                    }
                                    let forward = std::mem::take(pending);
                                    startup = None;
                                    if let Some(done) = startup_tx.take() {
                                        let _ = done.send(answered);
                                    }
                                    if forward.is_empty() {
                                        continue;
                                    }
                                    if events.send(Event::Output(id, forward)).is_err() {
                                        return;
                                    }
                                }
                            }
                            continue;
                        }
                        if events
                            .send(Event::Output(id, buffer[..n].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    // Linux PTYs return EIO when the slave closes.
                    Err(_) => {
                        if let Some(done) = startup_tx.take() {
                            let _ = done.send(false);
                        }
                        break;
                    }
                }
            }
            if let Some(mut child) = child {
                let status = match child.wait() {
                    Ok(status) => status.to_string(),
                    Err(error) => format!("Process ended: {error}"),
                };
                let _ = events.send(Event::Exited(id, status));
            }
        });
        if crate::backends::waits_for_pty_startup()
            && startup_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap_or(false)
        {
            let _ = handshake_rx.recv_timeout(std::time::Duration::from_secs(2));
        }
        Ok(Self {
            id,
            cwd: cwd.to_path_buf(),
            theme: options.theme,
            parser: vt100::Parser::new_with_callbacks(
                24,
                80,
                options.scrollback,
                Callbacks::default(),
            ),
            exited: None,
            search: crate::search::Search::default(),
            control_mode: None,
            pending_paste: None,
            palette: Palette::default(),
            manual_title: None,
            command_boundaries: Vec::new(),
            completed_commands: Vec::new(),
            command_started: None,
            master: pair.master,
            input: Some(input),
            killer,
        })
    }
    pub fn process(&mut self, bytes: &[u8]) -> Result<()> {
        self.parser.process(bytes);
        if let Some(cwd) = self.parser.callbacks_mut().cwd.take() {
            self.cwd = cwd;
        }
        let boundaries = std::mem::take(&mut self.parser.callbacks_mut().boundaries);
        for boundary in boundaries {
            self.record_boundary(boundary);
        }
        self.refresh_mode();
        let replies = std::mem::take(&mut self.parser.callbacks_mut().replies);
        if !replies.is_empty() {
            self.write(replies)?;
        }
        Ok(())
    }
    fn record_boundary(&mut self, boundary: CommandBoundary) {
        match boundary.marker {
            ShellMarker::CommandOutputStart => self.command_started = Some(Instant::now()),
            ShellMarker::CommandFinished(status) => {
                if let Some(started) = self.command_started.take() {
                    self.completed_commands.push(CompletedCommand {
                        status,
                        duration: started.elapsed(),
                    });
                    if self.completed_commands.len() > 64 {
                        self.completed_commands.drain(..32);
                    }
                }
            }
            ShellMarker::PromptStart | ShellMarker::PromptEnd => {}
        }
        self.command_boundaries.push(boundary);
        if self.command_boundaries.len() > 2_048 {
            self.command_boundaries.drain(..1_024);
        }
    }
    pub fn take_completed_commands(&mut self) -> Vec<CompletedCommand> {
        std::mem::take(&mut self.completed_commands)
    }
    pub fn navigate_command(&mut self, previous: bool) -> bool {
        let mut snapshot = self.parser.screen().clone();
        snapshot.set_scrollback(usize::MAX);
        let history = snapshot.scrollback();
        let top = history.saturating_sub(self.parser.screen().scrollback());
        let target = if previous {
            self.command_boundaries
                .iter()
                .rev()
                .find(|boundary| boundary.line < top)
        } else {
            self.command_boundaries
                .iter()
                .find(|boundary| boundary.line > top)
        };
        let Some(target) = target else {
            return false;
        };
        self.parser
            .screen_mut()
            .set_scrollback(history.saturating_sub(target.line));
        true
    }
    pub fn write(&self, bytes: Vec<u8>) -> Result<()> {
        if let Some(input) = &self.input {
            input
                .try_send(bytes)
                .context("terminal input is unavailable or busy")?;
        }
        Ok(())
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if self.parser.screen().size() != (rows, cols) {
            self.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            self.parser.screen_mut().set_size(rows, cols);
            self.refresh_mode();
        }
        Ok(())
    }
    pub fn finish(&mut self, status: String) {
        self.exited = Some(status);
        self.input = None;
    }
    pub fn set_mode(&mut self, mode: Option<crate::island::Mode>) {
        self.control_mode = mode;
        match mode {
            Some(crate::island::Mode::Search) => {
                self.refresh_mode();
                self.search.reveal(self.parser.screen_mut());
            }
            Some(crate::island::Mode::PasteConfirm) => {}
            Some(crate::island::Mode::Palette) => self.palette = Palette::default(),
            Some(crate::island::Mode::RenameTab) => {
                self.palette.query = self.manual_title.clone().unwrap_or_else(|| {
                    let title = &self.parser.callbacks().title;
                    if title.is_empty() {
                        self.cwd
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Terminal".into())
                    } else {
                        title.clone()
                    }
                });
                self.palette.cursor = self.palette.query.len();
                self.palette.selection = TextSelection {
                    anchor: 0,
                    focus: self.palette.cursor,
                };
            }
            Some(crate::island::Mode::SaveOutput) => self.palette = Palette::default(),
            None => {
                self.search.clear();
                self.pending_paste = None;
            }
        }
    }
    fn refresh_mode(&mut self) {
        match self.control_mode {
            Some(crate::island::Mode::Search) => self.search.refresh(self.parser.screen()),
            Some(crate::island::Mode::PasteConfirm) => {}
            Some(crate::island::Mode::Palette) => {}
            Some(crate::island::Mode::RenameTab) => {}
            Some(crate::island::Mode::SaveOutput) => {}
            None => {}
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.input = None;
        if self.exited.is_none() {
            crate::backends::hangup(self.master.as_ref());
            let _ = self.killer.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_markers_track_commands_and_navigate_boundaries() {
        let (tx, _rx) = mpsc::sync_channel(128);
        let mut session = Session::spawn(
            1,
            &crate::terminal::idle_cwd(),
            &crate::terminal::idle_command(),
            tx,
        )
        .unwrap();
        session.resize(2, 20).unwrap();
        session
            .process(b"\x1b]133;A\x07$ \x1b]133;B\x07echo done\x1b]133;C\x07done\r\n")
            .unwrap();
        assert!(matches!(
            session.command_boundaries.as_slice(),
            [
                CommandBoundary {
                    marker: ShellMarker::PromptStart,
                    ..
                },
                CommandBoundary {
                    marker: ShellMarker::PromptEnd,
                    ..
                },
                CommandBoundary {
                    marker: ShellMarker::CommandOutputStart,
                    ..
                }
            ]
        ));
        session.command_started = Some(Instant::now() - Duration::from_secs(12));
        session.process(b"\x1b]133;D;0\x07").unwrap();
        let completed = session.take_completed_commands();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, Some(0));
        assert!(completed[0].duration >= Duration::from_secs(12));

        session
            .process(b"\x1b]133;A\x07one\r\ntwo\r\nthree\r\nfour\r\nfive")
            .unwrap();
        assert!(session.navigate_command(true));
        assert!(session.parser.screen().scrollback() > 0);
    }

    #[test]
    fn vt_state_title_queries_and_alternate_screen_survive_chunked_output() {
        let mut parser = vt100::Parser::new_with_callbacks(4, 20, 20, Callbacks::default());
        for chunk in b"\x1b]2;build\x07\x1b[31mhi\x1b[0m\x1b[6n".chunks(2) {
            parser.process(chunk);
        }
        assert_eq!(parser.callbacks().title, "build");
        assert_eq!(parser.callbacks().replies, b"\x1b[1;3R");
        assert_eq!(
            parser.screen().cell(0, 0).unwrap().fgcolor(),
            vt100::Color::Idx(1)
        );
        parser.process(b"\x1b[?1049hother\x1b[?1049l");
        assert_eq!(parser.screen().contents(), "hi");
    }
    #[test]
    fn osc7_updates_the_working_directory_and_rejects_remote_hosts() {
        assert_eq!(
            parse_osc7(b"file://localhost/tmp/a%20b"),
            Some(PathBuf::from("/tmp/a b"))
        );
        assert_eq!(parse_osc7(b"file://remote/tmp"), None);
        assert_eq!(parse_osc7(b"invalid"), None);
    }
    #[test]
    fn output_text_includes_scrollback_and_soft_wrapped_rows() {
        let mut parser = vt100::Parser::new(2, 6, 20);
        parser.process(b"first\r\nsecond\r\nthird");
        assert_eq!(output_text(parser.screen()), "first\nsecond\nthird");
        let mut wrapped = vt100::Parser::new(2, 4, 20);
        wrapped.process(b"abcdefgh");
        assert_eq!(output_text(wrapped.screen()), "abcdefgh");
    }
    #[cfg(unix)]
    #[test]
    fn real_pty_has_correct_size_environment_and_reaps_child() {
        let (tx, rx) = mpsc::sync_channel(128);
        let mut session = Session::spawn(
            7,
            Path::new("/tmp"),
            &[
                "/bin/sh".into(),
                "-c".into(),
                "read line; stty size; printf '\\033[32m%s:%s:%s' \"$TERM\" \"$PWD\" \"$line\""
                    .into(),
            ],
            tx,
        )
        .unwrap();
        session.resize(32, 100).unwrap();
        session.write(b"hello\r".to_vec()).unwrap();
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
                Event::Output(_, bytes) => session.process(&bytes).unwrap(),
                Event::Exited(_, status) => {
                    session.finish(status);
                    break;
                }
                Event::Configuration(_) => unreachable!(),
            }
        }
        let text = session.parser.screen().contents();
        assert!(text.contains("32 100"), "{text}");
        assert!(text.contains("xterm-256color:/tmp:hello"), "{text}");
    }
    #[cfg(unix)]
    #[test]
    fn profile_options_apply_environment_scrollback_and_theme() {
        let (tx, rx) = mpsc::sync_channel(128);
        let mut session = Session::spawn_with(
            8,
            Path::new("/tmp"),
            &[
                "/bin/sh".into(),
                "-c".into(),
                "printf %s \"$CTERM_PROFILE_TEST\"".into(),
            ],
            &SessionOptions {
                environment: BTreeMap::from([("CTERM_PROFILE_TEST".into(), "applied".into())]),
                scrollback: 7,
                theme: crate::config::TerminalTheme::Dracula,
            },
            tx,
        )
        .unwrap();
        assert_eq!(session.cwd, Path::new("/tmp"));
        assert_eq!(session.theme, crate::config::TerminalTheme::Dracula);
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
                Event::Output(_, bytes) => session.process(&bytes).unwrap(),
                Event::Exited(_, status) => {
                    session.finish(status);
                    break;
                }
                Event::Configuration(_) => unreachable!(),
            }
        }
        assert!(session.parser.screen().contents().contains("applied"));
    }
    #[cfg(windows)]
    #[test]
    fn real_pty_runs_cmd() {
        let (tx, rx) = mpsc::sync_channel(128);
        let mut session = Session::spawn(
            7,
            &std::env::temp_dir(),
            &[
                "cmd.exe".into(),
                "/d".into(),
                "/c".into(),
                "echo hello-from-pty".into(),
            ],
            tx,
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut exited = false;
        loop {
            let text = session.parser.screen().contents();
            if exited && text.contains("hello-from-pty") {
                break;
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                panic!("timed out waiting for cmd.exe: {text:?}");
            }
            match rx.recv_timeout(left).unwrap() {
                Event::Output(_, bytes) => session.process(&bytes).unwrap(),
                Event::Exited(_, status) => {
                    session.finish(status);
                    exited = true;
                }
                Event::Configuration(_) => unreachable!(),
            }
        }
    }
}
