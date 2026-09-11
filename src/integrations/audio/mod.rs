#[cfg(not(target_os = "windows"))]
mod mpris;
#[cfg(target_os = "windows")]
mod windows;

use crate::platform::Playback;

pub trait AudioIntegration {
    fn playback(&self) -> Option<Playback>;
    fn toggle_playback(&self);
}

pub struct Fallback;
#[cfg(not(target_os = "windows"))]
pub use mpris::Mpris;
#[cfg(target_os = "windows")]
pub use windows::WindowsMedia;

impl AudioIntegration for Fallback {
    fn playback(&self) -> Option<Playback> {
        None
    }
    fn toggle_playback(&self) {}
}
