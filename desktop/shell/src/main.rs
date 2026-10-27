use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        None | Some("shell") => coconut_shell::run(),
        Some("settings") => open_settings(),
        Some("bg") => open_background(),
        Some("terminal") => open_terminal(),
        Some(other) => {
            eprintln!("coconut: unknown command \"{other}\"");
            eprintln!("usage: coconut [shell|settings [route]|bg|terminal]");
            std::process::exit(1);
        }
    }
}

fn open_background() {
    open_sibling("coconut-bg");
}

fn open_settings() {
    open_sibling("coconut-settings");
}

fn open_terminal() {
    open_sibling("cterm");
}

fn open_sibling(name: &str) {
    let program = sibling_program(name);
    match Command::new(&program)
        .args(std::env::args_os().skip(2))
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("coconut: failed to start {}: {error}", program.display());
            std::process::exit(1);
        }
    }
}

fn sibling_program(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|directory| directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}
