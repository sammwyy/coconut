//! Standalone File Explorer application.

use coconut_files::{
    cli::{self, Launch},
    show_window, WindowRequest,
};

fn main() -> Result<(), String> {
    match cli::parse(std::env::args_os().skip(1))? {
        Launch::Manager(path) => {
            show_window(WindowRequest::manager(path));
        }
        Launch::Settings => coconut_files::show_settings(),
        Launch::Portal => coconut_files::portal::service::run()?,
        Launch::Help => println!("{}", cli::HELP),
        Launch::Version => println!("File Explorer {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
