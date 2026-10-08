use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Finds Settings next to the running Coconut binary, then on `PATH`.
pub fn settings_program() -> Option<PathBuf> {
    let name = format!("coconut-settings{}", std::env::consts::EXE_SUFFIX);
    let beside_shell = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join(&name)));
    beside_shell.filter(|path| path.is_file()).or_else(|| {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join(&name))
                .find(|path| path.is_file())
        })
    })
}

/// Opens Coconut Settings at an internal route without keeping the shell's
/// process group or standard streams attached to it.
pub fn open_settings(route: impl Into<String>) {
    let Some(program) = settings_program() else {
        eprintln!("coconut: coconut-settings is not installed");
        return;
    };
    let route = route.into();
    std::thread::spawn(move || {
        let mut command = Command::new(program);
        command
            .arg(route)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        detach(&mut command);
        if let Err(error) = command.spawn() {
            eprintln!("coconut: could not start Settings: {error}");
        }
    });
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
