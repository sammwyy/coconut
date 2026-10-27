use coconut_media_player::cli::{self, Launch};

fn main() -> Result<(), String> {
    match cli::parse(std::env::args_os().skip(1))? {
        Launch::Play(paths) => coconut_media_player::run(paths),
        Launch::Settings => coconut_media_player::app::show_settings(),
        Launch::Quit => coconut_media_player::app::quit()?,
        Launch::Help => println!("{}", cli::HELP),
        Launch::Version => println!("Media Player {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
