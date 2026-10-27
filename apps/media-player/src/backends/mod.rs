use std::io::Read;
use std::process::{Child, Command};

macro_rules! register_os_backend {
    ($(#[cfg($meta:meta)] $module:ident),+ $(,)?) => {
        $(
            #[cfg($meta)]
            mod $module;
            #[cfg($meta)]
            pub use $module::*;
        )+
    };
}

register_os_backend! {
    #[cfg(unix)] unix,
    #[cfg(windows)] windows,
}

pub mod media;
#[cfg(target_os = "linux")]
mod mpris;
#[cfg(windows)]
mod smtc;

#[cfg(target_os = "linux")]
pub use mpris::MediaSession;
#[cfg(windows)]
pub use smtc::MediaSession;

/// Platforms without a media control integration accept updates silently.
#[cfg(not(any(target_os = "linux", windows)))]
pub struct MediaSession;

#[cfg(not(any(target_os = "linux", windows)))]
impl MediaSession {
    pub fn new(_: std::sync::mpsc::Sender<media::MediaCommand>) -> Option<Self> {
        None
    }
    pub fn update(&self, _: &media::MediaSnapshot) {}
    pub fn seeked(&self, _: f64) {}
}

pub struct SpawnedDecoder {
    pub child: Child,
    pub control: ProcessControl,
    pub video_reader: Option<Box<dyn Read + Send>>,
    pub audio_reader: Option<Box<dyn Read + Send>>,
}

/// Creates a `Command` configured for background media subprocesses
/// (e.g. suppressing console window creation on Windows).
pub fn background_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    configure_command(&mut cmd);
    cmd
}
