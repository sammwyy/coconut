//! Platform-neutral description of the now-playing state shared with the
//! desktop's media controls (MPRIS on Linux, SMTC on Windows).

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    None,
    Track,
    Playlist,
}

#[derive(Clone, Debug)]
pub struct Track {
    /// Changes whenever a new item starts, even when the same file repeats.
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Seconds, `0.0` when unknown.
    pub duration: f64,
    pub video: bool,
    /// Encoded cover art (PNG).
    pub art: Option<Arc<[u8]>>,
}

impl PartialEq for Track {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.duration == other.duration
            && self.video == other.video
            && match (&self.art, &other.art) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaSnapshot {
    pub state: PlaybackState,
    pub track: Option<Track>,
    /// Seconds.
    pub position: f64,
    pub volume: f64,
    pub rate: f64,
    pub shuffle: bool,
    pub loop_mode: LoopMode,
    pub can_next: bool,
    pub can_previous: bool,
}

impl MediaSnapshot {
    /// Equal apart from the playback position, which advances every tick
    /// and is read on demand rather than announced.
    pub fn same_besides_position(&self, other: &Self) -> bool {
        Self {
            position: other.position,
            ..self.clone()
        } == *other
    }
}

/// A request from the desktop's media controls.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// Relative seek in seconds.
    SeekBy(f64),
    /// Absolute position in seconds.
    SetPosition(f64),
    SetVolume(f64),
    SetRate(f64),
    SetShuffle(bool),
    SetLoop(LoopMode),
    Raise,
    Quit,
    Open(PathBuf),
}

pub const MIN_RATE: f64 = 0.5;
pub const MAX_RATE: f64 = 2.0;
