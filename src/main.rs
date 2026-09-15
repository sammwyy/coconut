fn main() {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        None | Some("shell") => coconut_shell::run(),
        Some("settings") => coconut_settings_panel::run(),
        Some(other) => {
            eprintln!("coconut: unknown command \"{other}\"");
            eprintln!("usage: coconut [shell|settings]");
            std::process::exit(1);
        }
    }
}
