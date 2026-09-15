use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        None | Some("shell") => coconut_shell::run(),
        Some("settings") => open_settings(),
        Some(other) => {
            eprintln!("coconut: unknown command \"{other}\"");
            eprintln!("usage: coconut [shell|settings]");
            std::process::exit(1);
        }
    }
}

fn open_settings() {
    let program = settings_program();
    match Command::new(&program).status() {
        Ok(status) if status.success() => {}
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("coconut: failed to start {}: {error}", program.display());
            std::process::exit(1);
        }
    }
}

fn settings_program() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent().map(|directory| {
                directory.join(format!("coconut-settings{}", std::env::consts::EXE_SUFFIX))
            })
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("coconut-settings"))
}
