mod mpris;

use crate::platform::Playback;

pub trait AudioIntegration {
    fn playback(&self) -> Option<Playback>;
    fn toggle_playback(&self);
}

pub struct Fallback;
pub use mpris::Mpris;

impl AudioIntegration for Fallback {
    fn playback(&self) -> Option<Playback> {
        None
    }
    fn toggle_playback(&self) {}
}
