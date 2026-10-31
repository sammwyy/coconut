use coconut_integration_clipboard::{config::Config, daemon};

#[tokio::main]
async fn main() {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("Usage: coconut-clipboard\nSession clipboard service. Configuration: $XDG_CONFIG_HOME/coconut/clipboard.toml");
        return;
    }
    if std::env::args().len() > 1 {
        eprintln!("clipboard: unexpected arguments; use --help");
        std::process::exit(2);
    }
    let result = match Config::load() {
        Ok(config) => daemon::run(config).await,
        Err(error) => Err(error.into()),
    };
    if let Err(error) = result {
        eprintln!("clipboard: {error}");
        std::process::exit(1);
    }
}
