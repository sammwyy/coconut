fn main() {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        None | Some("shell") => creamshell_shell::run(),
        Some("settings") => creamshell_settings_panel::run(),
        Some(other) => {
            eprintln!("creamshell: unknown command \"{other}\"");
            eprintln!("usage: creamshell [shell|settings]");
            std::process::exit(1);
        }
    }
}
