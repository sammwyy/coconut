use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Spawns `command` detached from Coconut's own process group/session so
/// the launched app keeps running even if the shell restarts or is closed.
pub fn spawn_detached(command: &mut Command) -> io::Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(command);
    command.spawn()
}

/// Builds the command that runs `program` inside a terminal emulator. Honors
/// `$TERMINAL` first, then the `cterm` binary installed next to the running
/// process or on `PATH`, then the `x-terminal-emulator` alternative.
pub fn terminal_command(program: &str, args: &[String]) -> Command {
    let terminal = std::env::var_os("TERMINAL")
        .map(PathBuf::from)
        .or_else(cterm)
        .unwrap_or_else(|| PathBuf::from("x-terminal-emulator"));
    let mut command = Command::new(terminal);
    command.arg("-e").arg(program).args(args);
    command
}

/// `cterm` sits next to the running executable in installed and `cargo run`
/// layouts alike; fall back to `PATH` for a separately installed binary.
fn cterm() -> Option<PathBuf> {
    let name = format!("cterm{}", std::env::consts::EXE_SUFFIX);
    if let Some(sibling) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)))
        .filter(|path| path.is_file())
    {
        return Some(sibling);
    }
    std::env::var_os("PATH")
        .iter()
        .flat_map(|paths| std::env::split_paths(paths))
        .map(|dir| dir.join(&name))
        .find(|path| path.is_file())
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
