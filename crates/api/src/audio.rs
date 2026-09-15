use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playback {
    pub app_icon: Option<PathBuf>,
    pub title: String,
    pub artist: String,
    pub status: String,
    pub art_url: Option<PathBuf>,
    pub position: String,
    pub length: String,
}

pub trait AudioIntegration {
    fn playback(&self) -> Option<Playback>;
    fn toggle_playback(&self);

    fn seek(&self, _position: f64) {}
    fn previous(&self) {}
    fn next(&self) {}
}

pub struct Fallback;

impl AudioIntegration for Fallback {
    fn playback(&self) -> Option<Playback> {
        None
    }
    fn toggle_playback(&self) {}
}
