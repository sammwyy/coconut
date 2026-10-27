mod audio;
mod decode;
mod mpv;
pub(crate) mod probe;

use crate::config::PlaybackBackend;
pub use audio::{AudioClock, AudioOutput};
pub use decode::{DecodeSession, EngineEvent, VideoFrame};
use mpv::{MpvPlayback, MpvSession};
pub use probe::MediaInfo;
pub use probe::SubtitleSource;
pub use probe::SubtitleStyle;

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use decode::{AUDIO_CHANNELS, AUDIO_RATE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvailableBackends {
    pub mpv: bool,
    pub ffmpeg: bool,
}

impl AvailableBackends {
    pub fn detect() -> Self {
        let installed = |program| {
            crate::backends::background_command(program)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        if !installed("ffprobe") {
            return Self::default();
        }
        Self {
            mpv: mpv::available(),
            ffmpeg: installed("ffmpeg"),
        }
    }

    pub fn choices(self) -> Vec<PlaybackBackend> {
        let mut choices = Vec::new();
        if self.mpv {
            choices.push(PlaybackBackend::Mpv);
        }
        if self.ffmpeg {
            choices.push(PlaybackBackend::Ffmpeg);
        }
        choices
    }

    pub fn effective(self, preferred: PlaybackBackend) -> Option<PlaybackBackend> {
        match preferred {
            PlaybackBackend::Mpv if self.mpv => Some(PlaybackBackend::Mpv),
            PlaybackBackend::Ffmpeg if self.ffmpeg => Some(PlaybackBackend::Ffmpeg),
            _ => self.choices().into_iter().next(),
        }
    }
}

/// Extracts the first attached picture stream from `path` as PNG bytes.
pub fn cover_art(path: &std::path::Path) -> Option<Vec<u8>> {
    let output = crate::backends::background_command("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-an",
            "-vcodec",
            "png",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-",
        ])
        .output()
        .ok()?;
    if output.status.success() && !output.stdout.is_empty() {
        Some(output.stdout)
    } else {
        None
    }
}

struct Inner {
    session: Option<PlaybackSession>,
    backend: PlaybackBackend,
    subtitle: Option<SubtitleSource>,
    subtitle_style: SubtitleStyle,
    clock: AudioClock,
    /// Media timestamp (in seconds) at which the current decode session started.
    base: f64,
    speed: f32,
    volume: f32,
    paused: bool,
    has_audio: bool,
    has_video: bool,
    path: PathBuf,
    out_w: u32,
    out_h: u32,
    /// Wall-clock fallback state for streams without an audio track.
    resume: Instant,
    frozen: f64,
    exited: bool,
    exit_code: i32,
    exit_tail: String,
    /// Monotonic counter incremented on each session spawn to discard events
    /// from superseded decode processes.
    generation: u64,
    active_generation: u64,
}

enum PlaybackSession {
    Ffmpeg(DecodeSession),
    Mpv(MpvSession),
}

pub struct RestartRequest {
    info: MediaInfo,
    backend: PlaybackBackend,
    subtitle: Option<SubtitleSource>,
    subtitle_style: SubtitleStyle,
    pos: f64,
    speed: f32,
    generation: u64,
    latest: Arc<AtomicU64>,
    audio: bool,
    paused: bool,
    volume: f32,
    events_tx: Sender<EngineEvent>,
    video_tx: SyncSender<(u64, VideoFrame)>,
}

pub struct PreparedSession {
    session: PlaybackSession,
    info: MediaInfo,
    pos: f64,
    speed: f32,
    generation: u64,
}

impl RestartRequest {
    pub fn spawn(self) -> Result<Option<PreparedSession>, String> {
        if self.latest.load(Ordering::Acquire) != self.generation {
            return Ok(None);
        }
        let (out_w, out_h) = self.info.output_size();
        if self.info.has_video && (out_w == 0 || out_h == 0) {
            return Err("video stream has no usable dimensions".into());
        }
        let override_backend = match std::env::var("CPLAYER_BACKEND").as_deref() {
            Ok("mpv") => Some(PlaybackBackend::Mpv),
            Ok("ffmpeg") => Some(PlaybackBackend::Ffmpeg),
            _ => None,
        };
        let preference = override_backend.unwrap_or(self.backend);
        let open_mpv = || {
            MpvSession::open(
                &self.info.path,
                (out_w, out_h),
                MpvPlayback {
                    pos: self.pos,
                    speed: self.speed,
                    paused: self.paused,
                    volume: self.volume,
                },
                self.subtitle.clone(),
                self.subtitle_style,
                self.info.audio_stream,
                self.events_tx.clone(),
                self.video_tx.clone(),
                self.generation,
            )
        };
        let mpv = (preference != PlaybackBackend::Ffmpeg)
            .then(&open_mpv)
            .flatten();
        if override_backend == Some(PlaybackBackend::Mpv) && mpv.is_none() {
            return Err("libmpv unavailable or could not initialize its renderer".into());
        }
        let session = if let Some(mpv) = mpv {
            PlaybackSession::Mpv(mpv)
        } else {
            match DecodeSession::spawn(
                &self.info.path,
                self.pos,
                self.speed,
                self.info.video_stream,
                self.info.audio_stream.filter(|_| self.audio),
                out_w,
                out_h,
                self.subtitle.as_ref(),
                self.subtitle_style,
                self.events_tx.clone(),
                self.video_tx.clone(),
                self.generation,
            ) {
                Ok(session) => PlaybackSession::Ffmpeg(session),
                Err(error)
                    if override_backend.is_none() && preference == PlaybackBackend::Ffmpeg =>
                {
                    PlaybackSession::Mpv(open_mpv().ok_or_else(|| {
                        format!("could not start ffmpeg: {error}; libmpv unavailable")
                    })?)
                }
                Err(error) => return Err(format!("could not start ffmpeg: {error}")),
            }
        };
        let prepared = PreparedSession {
            session,
            info: self.info,
            pos: self.pos,
            speed: self.speed,
            generation: self.generation,
        };
        if self.latest.load(Ordering::Acquire) != self.generation {
            return Ok(None);
        }
        Ok(Some(prepared))
    }
}

pub struct Engine {
    events_tx: Sender<EngineEvent>,
    events_rx: Arc<Mutex<Receiver<EngineEvent>>>,
    video_tx: SyncSender<(u64, VideoFrame)>,
    video_rx: Arc<Mutex<Receiver<(u64, VideoFrame)>>>,
    audio: Option<AudioOutput>,
    latest: Arc<AtomicU64>,
    inner: Mutex<Inner>,
}

impl Engine {
    pub fn new() -> Self {
        let (events_tx, events_rx) = channel();
        let (video_tx, video_rx) = std::sync::mpsc::sync_channel(2);
        Engine {
            events_tx,
            events_rx: Arc::new(Mutex::new(events_rx)),
            video_tx,
            video_rx: Arc::new(Mutex::new(video_rx)),
            audio: AudioOutput::open(),
            latest: Arc::new(AtomicU64::new(0)),
            inner: Mutex::new(Inner {
                session: None,
                backend: PlaybackBackend::Auto,
                subtitle: None,
                subtitle_style: SubtitleStyle::default(),
                clock: AudioClock::default(),
                base: 0.0,
                speed: 1.0,
                volume: 1.0,
                paused: false,
                has_audio: false,
                has_video: false,
                path: PathBuf::new(),
                out_w: 0,
                out_h: 0,
                resume: Instant::now(),
                frozen: 0.0,
                exited: false,
                exit_code: 0,
                exit_tail: String::new(),
                generation: 0,
                active_generation: 0,
            }),
        }
    }

    pub fn events(&self) -> Arc<Mutex<Receiver<EngineEvent>>> {
        self.events_rx.clone()
    }

    pub fn video(&self) -> Arc<Mutex<Receiver<(u64, VideoFrame)>>> {
        self.video_rx.clone()
    }

    pub fn request_session(&self, info: MediaInfo, pos: f64, speed: f32) -> RestartRequest {
        let mut inner = self.inner.lock().unwrap();
        inner.generation += 1;
        self.latest.store(inner.generation, Ordering::Release);
        RestartRequest {
            backend: inner.backend,
            subtitle: inner.subtitle.clone(),
            subtitle_style: inner.subtitle_style,
            audio: info.has_audio && self.audio.is_some(),
            paused: inner.paused,
            volume: inner.volume,
            info,
            pos,
            speed,
            generation: inner.generation,
            latest: self.latest.clone(),
            events_tx: self.events_tx.clone(),
            video_tx: self.video_tx.clone(),
        }
    }

    /// Returns false if a newer request superseded this subprocess.
    pub fn activate(&self, prepared: PreparedSession, initial_paused: Option<bool>) -> bool {
        let PreparedSession {
            mut session,
            info,
            pos,
            speed,
            generation,
        } = prepared;
        let (out_w, out_h) = info.output_size();
        let mut inner = self.inner.lock().unwrap();
        if inner.generation != generation {
            return false;
        }
        let paused = initial_paused.unwrap_or(inner.paused);
        inner.session = None;
        let mpv = matches!(session, PlaybackSession::Mpv(_));
        match &mut session {
            PlaybackSession::Ffmpeg(session) => {
                if let Some(player) = self.audio.as_ref() {
                    if let Some(rx) = session.audio_rx.take() {
                        inner.clock = player.play_stream(rx);
                        if !paused {
                            player.player.play();
                        }
                    } else {
                        player.player.stop();
                        inner.clock = AudioClock::default();
                    }
                }
                session.activate(paused);
            }
            PlaybackSession::Mpv(session) => {
                if let Some(player) = &self.audio {
                    player.player.stop();
                }
                inner.clock = AudioClock::default();
                session.pause(paused);
                session.volume(inner.volume);
            }
        }

        inner.session = Some(session);
        inner.active_generation = generation;
        inner.base = pos;
        inner.speed = speed;
        inner.paused = paused;
        inner.has_audio = info.has_audio && (mpv || self.audio.is_some());
        inner.has_video = info.has_video;
        inner.path = info.path;
        inner.out_w = out_w;
        inner.out_h = out_h;
        inner.resume = Instant::now();
        inner.frozen = 0.0;
        inner.exited = false;
        inner.exit_code = 0;
        inner.exit_tail.clear();
        true
    }

    pub fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.generation += 1;
        inner.active_generation = inner.generation;
        self.latest.store(inner.generation, Ordering::Release);
        inner.session = None;
        inner.subtitle = None;
        inner.subtitle_style = SubtitleStyle::default();
        inner.exited = false;
        if let Some(player) = &self.audio {
            player.player.stop();
        }
    }

    pub fn generation(&self) -> u64 {
        self.inner.lock().unwrap().generation
    }

    pub fn video_generation(&self) -> u64 {
        self.inner.lock().unwrap().active_generation
    }

    pub fn set_paused(&self, paused: bool) {
        let mut inner = self.inner.lock().unwrap();
        if inner.paused == paused {
            return;
        }
        inner.paused = paused;
        if let Some(session) = &inner.session {
            match session {
                PlaybackSession::Mpv(session) => session.pause(paused),
                PlaybackSession::Ffmpeg(session) => {
                    if paused {
                        if let Some(player) = &self.audio {
                            player.player.pause();
                        }
                        session.suspend();
                    } else {
                        session.resume();
                        if let Some(player) = &self.audio {
                            player.player.play();
                        }
                    }
                }
            }
        }
        if paused {
            inner.frozen += inner.resume.elapsed().as_secs_f64();
        } else {
            inner.resume = Instant::now();
        }
    }

    pub fn position(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        if inner.session.is_none() {
            return inner.base;
        }
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            return session.position();
        }
        let running = if inner.paused {
            0.0
        } else {
            inner.resume.elapsed().as_secs_f64()
        };
        let wall = inner.frozen + running;
        let elapsed = if inner.has_audio {
            let samples = inner.clock.played.load(Ordering::Relaxed) as f64;
            let audio = samples / (AUDIO_CHANNELS as f64 * AUDIO_RATE as f64);
            if inner.has_video {
                audio.max(wall)
            } else {
                audio
            }
        } else {
            wall
        };
        inner.base + elapsed * inner.speed as f64
    }

    pub fn ended(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if inner.session.is_none() {
            return false;
        }
        if matches!(inner.session.as_ref(), Some(PlaybackSession::Mpv(_))) {
            inner.exited && inner.exit_code == 0
        } else if inner.has_audio {
            inner.clock.drained.load(Ordering::Relaxed) && (!inner.exited || inner.exit_code == 0)
        } else {
            inner.exited && inner.exit_code == 0
        }
    }

    pub fn mark_exited(&self, code: i32, tail: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.exited = true;
        inner.exit_code = code;
        inner.exit_tail = tail;
    }

    pub fn failed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.exited && inner.exit_code != 0
    }

    pub fn exit_tail(&self) -> String {
        self.inner.lock().unwrap().exit_tail.clone()
    }

    /// Sets the rate used by the next session without spawning one.
    pub fn prime_speed(&self, speed: f32) {
        self.inner.lock().unwrap().speed = speed;
    }

    pub fn set_backend(&self, backend: PlaybackBackend) {
        self.inner.lock().unwrap().backend = backend;
    }

    pub fn set_subtitle(&self, subtitle: Option<SubtitleSource>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.subtitle = subtitle.clone();
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.subtitle(subtitle);
            true
        } else {
            false
        }
    }

    pub fn set_subtitle_style(&self, style: SubtitleStyle) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.subtitle_style = style;
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.subtitle_style(style);
            true
        } else {
            false
        }
    }

    pub fn set_audio_live(&self, index: usize) -> bool {
        let inner = self.inner.lock().unwrap();
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.audio(index);
            true
        } else {
            false
        }
    }

    pub fn seek_live(&self, pos: f64) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.seek(pos);
            inner.exited = false;
            inner.base = pos;
            true
        } else {
            false
        }
    }

    pub fn set_speed_live(&self, speed: f32) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.speed(speed);
            inner.speed = speed;
            true
        } else {
            false
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.volume = volume.max(0.0);
        if let Some(PlaybackSession::Mpv(session)) = &inner.session {
            session.volume(inner.volume);
        }
        if let Some(player) = &self.audio {
            player.player.set_volume(volume.max(0.0));
        }
    }

    pub fn has_audio_output(&self) -> bool {
        self.audio.is_some()
            || matches!(
                self.inner.lock().unwrap().session.as_ref(),
                Some(PlaybackSession::Mpv(_))
            )
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.session = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_choices_only_contain_available_engines() {
        let both = AvailableBackends {
            mpv: true,
            ffmpeg: true,
        };
        assert_eq!(
            both.choices(),
            [PlaybackBackend::Mpv, PlaybackBackend::Ffmpeg]
        );
        assert_eq!(
            both.effective(PlaybackBackend::Auto),
            Some(PlaybackBackend::Mpv)
        );
        assert_eq!(
            both.effective(PlaybackBackend::Ffmpeg),
            Some(PlaybackBackend::Ffmpeg)
        );
        let only_ffmpeg = AvailableBackends {
            mpv: false,
            ffmpeg: true,
        };
        assert_eq!(only_ffmpeg.choices(), [PlaybackBackend::Ffmpeg]);
        assert_eq!(
            only_ffmpeg.effective(PlaybackBackend::Mpv),
            Some(PlaybackBackend::Ffmpeg)
        );
        assert!(AvailableBackends::default().choices().is_empty());
        assert_eq!(
            AvailableBackends::default().effective(PlaybackBackend::Auto),
            None
        );
    }

    #[test]
    fn superseded_request_never_starts_decoder() {
        let (events_tx, _) = channel();
        let (video_tx, _) = std::sync::mpsc::sync_channel(1);
        let request = RestartRequest {
            info: MediaInfo {
                path: PathBuf::from("/nonexistent/cplayer-test"),
                title: String::new(),
                artist: None,
                album: None,
                duration: 0.0,
                has_video: false,
                has_audio: true,
                video_stream: None,
                audio_stream: Some(0),
                audio_tracks: vec![],
                subtitles: vec![],
                attached_picture: false,
                width: 0,
                height: 0,
            },
            backend: PlaybackBackend::Auto,
            subtitle: None,
            subtitle_style: SubtitleStyle::default(),
            pos: 0.0,
            speed: 1.0,
            volume: 1.0,
            paused: false,
            generation: 1,
            latest: Arc::new(AtomicU64::new(2)),
            audio: true,
            events_tx,
            video_tx,
        };
        assert!(request.spawn().unwrap().is_none());
    }

    #[test]
    fn live_seek_and_rate_do_not_replace_mpv_session() {
        let Some(path) = std::env::var_os("CPLAYER_MPV_TEST_FILE") else {
            return;
        };
        let engine = Engine::new();
        let info = probe::probe(std::path::Path::new(&path)).unwrap();
        let prepared = engine
            .request_session(info.clone(), 0.0, 1.0)
            .spawn()
            .unwrap()
            .unwrap();
        assert!(matches!(prepared.session, PlaybackSession::Mpv(_)));
        assert!(engine.activate(prepared, Some(false)));
        let generation = engine.generation();
        assert!(engine.seek_live(0.4));
        assert!(engine.set_speed_live(1.5));
        assert_eq!(engine.generation(), generation);
        assert_eq!(engine.position(), 0.4);
        engine.set_backend(PlaybackBackend::Ffmpeg);
        let prepared = engine
            .request_session(info, engine.position(), 1.5)
            .spawn()
            .unwrap()
            .unwrap();
        assert!(matches!(prepared.session, PlaybackSession::Ffmpeg(_)));
        assert!(engine.activate(prepared, None));
        assert!(!engine.seek_live(0.2));
        engine.stop();
    }

    #[test]
    fn ffmpeg_fallback_decodes_when_fixture_is_provided() {
        let Some(path) = std::env::var_os("CPLAYER_FFMPEG_TEST_FILE") else {
            return;
        };
        let engine = Engine::new();
        engine.set_backend(PlaybackBackend::Ffmpeg);
        let info = probe::probe(std::path::Path::new(&path)).unwrap();
        let prepared = engine
            .request_session(info.clone(), 0.0, 1.0)
            .spawn()
            .unwrap()
            .unwrap();
        assert!(matches!(prepared.session, PlaybackSession::Ffmpeg(_)));
        assert!(engine.activate(prepared, Some(false)));
        let (generation, frame) = engine
            .video()
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(generation, engine.generation());
        assert_eq!((frame.width, frame.height), (320, 180));
        engine.set_paused(true);
        let paused = engine.position();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!((engine.position() - paused).abs() < 0.01);
        engine.set_paused(false);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(engine.position() > paused + 0.1);
        let active = engine.video_generation();
        let prepared = engine
            .request_session(info, engine.position(), 1.0)
            .spawn()
            .unwrap()
            .unwrap();
        assert_eq!(engine.video_generation(), active);
        assert!(engine.activate(prepared, None));
        assert_ne!(engine.video_generation(), active);
        assert!(!engine.set_audio_live(1));
        assert!(!engine.seek_live(0.2));
        engine.stop();
    }
}
