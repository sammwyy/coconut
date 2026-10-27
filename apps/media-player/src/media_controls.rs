//! Bridges the player with the desktop's media controls.

use crate::backends::media::{LoopMode, MediaCommand, MediaSnapshot, PlaybackState, Track};
use crate::backends::MediaSession;
use crate::state::{PlayerState, Repeat, Status};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

impl PlayerState {
    /// Registers with the platform once the window exists (Windows binds the
    /// controls to it).
    pub fn start_media_controls(&self) {
        let (sender, receiver) = mpsc::channel();
        let Some(session) = MediaSession::new(sender) else {
            return;
        };
        *self.media_session.borrow_mut() = Some(session);
        self.pump_media_commands(Arc::new(Mutex::new(receiver)));
    }

    fn pump_media_commands(&self, receiver: Arc<Mutex<Receiver<MediaCommand>>>) {
        let state = self.clone();
        let next = receiver.clone();
        self.app.spawn_background(
            move || receiver.lock().ok()?.recv().ok(),
            move |command| {
                if let Some(command) = command {
                    state.handle_media_command(command);
                    state.pump_media_commands(next);
                }
            },
        );
    }

    fn handle_media_command(&self, command: MediaCommand) {
        match command {
            MediaCommand::Play => self.set_playing(true),
            MediaCommand::Pause => self.set_playing(false),
            MediaCommand::PlayPause => self.toggle(),
            MediaCommand::Stop => {
                self.set_playing(false);
                self.seek_to(0.0);
            }
            MediaCommand::Next => {
                self.advance(true);
            }
            MediaCommand::Previous => self.prev(),
            MediaCommand::SeekBy(delta) => self.seek_by(delta),
            MediaCommand::SetPosition(position) => self.seek_to(position),
            MediaCommand::SetVolume(volume) => {
                if self.muted.peek() && volume > 0.0 {
                    self.toggle_mute();
                }
                self.set_volume(volume as f32);
            }
            MediaCommand::SetRate(rate) => self.set_speed(rate as f32),
            MediaCommand::SetShuffle(shuffle) => self.shuffle.set(shuffle),
            MediaCommand::SetLoop(mode) => self.repeat.set(match mode {
                LoopMode::None => Repeat::Off,
                LoopMode::Track => Repeat::One,
                LoopMode::Playlist => Repeat::All,
            }),
            MediaCommand::Raise => {
                crate::app::player::open_window(self);
            }
            MediaCommand::Quit => self.app.exit(),
            MediaCommand::Open(path) => {
                let start = self.queue.peek().len();
                let had_current = self.current.peek().is_some();
                self.enqueue(crate::cli::collect_media(vec![path]));
                if had_current && self.queue.peek().len() > start {
                    self.play_index(start);
                }
            }
        }
        self.publish_media();
        self.poke();
    }

    pub(crate) fn publish_media(&self) {
        let session = self.media_session.borrow();
        let discord = self.discord.borrow();
        if session.is_none() && discord.is_none() {
            return;
        }
        let snapshot = self.media_snapshot();
        let interface = self.config.peek().interface;
        if let Some(session) = session.as_ref() {
            session.update(&snapshot);
        }
        if let Some(discord) = discord.as_ref() {
            discord.update(&snapshot, &interface);
        }
    }

    pub(crate) fn announce_seek(&self, position: f64) {
        let snapshot = MediaSnapshot {
            position,
            ..self.media_snapshot()
        };
        if let Some(session) = self.media_session.borrow().as_ref() {
            session.update(&snapshot);
            session.seeked(position);
        }
        if let Some(discord) = self.discord.borrow().as_ref() {
            discord.update(&snapshot, &self.config.peek().interface);
        }
    }

    fn media_snapshot(&self) -> MediaSnapshot {
        let media = self.media.peek();
        let loaded = self.status.peek() == Status::Playing && media.is_some();
        let queue_len = self.queue.peek().len();
        let current = self.current.peek();
        let repeat = self.repeat.peek();
        let shuffle = self.shuffle.peek();
        MediaSnapshot {
            state: match (loaded, self.playing.peek()) {
                (true, true) => PlaybackState::Playing,
                (true, false) => PlaybackState::Paused,
                _ => PlaybackState::Stopped,
            },
            track: media.filter(|_| loaded).map(|media| Track {
                id: self.track_serial.get(),
                path: media.path,
                title: media.title,
                artist: media.artist,
                album: media.album,
                duration: media.duration,
                video: media.has_video,
                art: self.cover_art.borrow().clone(),
            }),
            position: self.position.peek(),
            volume: if self.muted.peek() {
                0.0
            } else {
                self.volume.peek() as f64
            },
            rate: self.speed.peek() as f64,
            shuffle,
            loop_mode: match repeat {
                Repeat::Off => LoopMode::None,
                Repeat::One => LoopMode::Track,
                Repeat::All => LoopMode::Playlist,
            },
            can_next: current.is_some_and(|index| {
                index + 1 < queue_len || (queue_len > 1 && (shuffle || repeat == Repeat::All))
            }),
            can_previous: current.is_some(),
        }
    }
}
