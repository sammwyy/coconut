//! Moved verbatim from `apps/shell/src/process.rs` (Phase 4b of the
//! dock/island/panel refactor): `app_drawer`'s launched-app spawning is the
//! only consumer of this helper left after the migration, and plugin crates
//! can't depend on the `apps/shell` binary crate to reuse it from there.

use std::io;
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
