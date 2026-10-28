use crate::config::{Accent, Config, RepeatMode};
use crate::engine::{self, Engine, EngineEvent, MediaInfo, SubtitleSource, SubtitleStyle};
use crate::icons::Icons;
use creamui_image::ImageData;
use creamui_reactive::Signal;
use creamui_render::{AppHandle, WindowHandle};
use creamui_theme::Color;
use creamui_widgets::ScrollController;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

/// Seconds the playback chrome takes to slide in or out.
const CHROME_TRANSITION: f32 = 0.24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, PartialEq)]
pub enum Status {
    Empty,
    Loading,
    Playing,
    Error(String),
}

#[derive(Clone)]
pub struct QueueItem {
    pub path: PathBuf,
    pub name: String,
}

fn insert_next(queue: &mut Vec<QueueItem>, current: Option<usize>, items: Vec<QueueItem>) {
    let index = current.map_or(queue.len(), |index| (index + 1).min(queue.len()));
    queue.splice(index..index, items);
}

pub const SPEEDS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

#[derive(Clone)]
pub struct PlayerState {
    pub engine: Rc<Engine>,
    pub app: AppHandle,
    pub icons: Icons,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    pub(crate) window_pending: Rc<Cell<bool>>,

    pub queue: Signal<Vec<QueueItem>>,
    pub current: Signal<Option<usize>>,
    pub media: Signal<Option<MediaInfo>>,
    pub status: Signal<Status>,
    pub playing: Signal<bool>,
    pub position: Signal<f64>,
    pub duration: Signal<f64>,
    /// Preview timestamp while the user is dragging the seek bar.
    pub scrub: Signal<Option<f64>>,
    pub volume: Signal<f32>,
    pub muted: Signal<bool>,
    pub speed: Signal<f32>,
    pub repeat: Signal<Repeat>,
    pub shuffle: Signal<bool>,
    pub queue_open: Signal<bool>,
    pub subtitles_open: Signal<bool>,
    pub audio_open: Signal<bool>,
    pub subtitle: Signal<Option<SubtitleSource>>,
    pub subtitle_style: Signal<SubtitleStyle>,
    pub controls_visible: Signal<bool>,
    pub frame: Signal<Option<ImageData>>,
    pub cover: Signal<Option<ImageData>>,
    /// Small, pre-blurred copy of the cover art used as the audio backdrop.
    pub backdrop: Signal<Option<ImageData>>,
    /// Accent picked from the cover art or the video, for `Accent::Auto`.
    media_accent: Rc<Cell<Option<Color>>>,
    video_frames: Rc<Cell<u32>>,
    /// Encoded cover art of the current item, shared with media controls.
    pub(crate) cover_art: Rc<RefCell<Option<std::sync::Arc<[u8]>>>>,
    /// Increments for every item that starts, so media controls see repeats.
    pub(crate) track_serial: Rc<Cell<u64>>,
    pub(crate) media_session: Rc<RefCell<Option<crate::backends::MediaSession>>>,
    /// Discord rich presence, enabled by `interface.discord_presence`.
    pub(crate) discord: Rc<RefCell<Option<crate::discord::Presence>>>,
    pub queue_scroll: ScrollController,
    pub subtitles_scroll: ScrollController,
    pub audio_scroll: ScrollController,
    pub config: Signal<Config>,
    /// Chrome slide progress: 0.0 fully hidden, 1.0 fully shown.
    pub chrome_progress: Signal<f32>,
    chrome_animating: Rc<Cell<bool>>,
    chrome_last: Rc<Cell<Instant>>,
    config_stamp: Rc<Cell<Option<SystemTime>>>,
    ticks: Rc<Cell<u32>>,

    last_move: Rc<Cell<Instant>>,
    last_pos: Rc<Cell<(f32, f32)>>,
    view_size: Rc<Cell<(f32, f32)>>,
    dragging_subtitle: Rc<Cell<bool>>,
    /// Monotonic token used to discard async `ffprobe` results from superseded track loads.
    pub load_token: Rc<AtomicU64>,
    pending_restart: Rc<Cell<Option<u64>>>,
    force_play: Rc<Cell<bool>>,
    pending_seek: Rc<Cell<Option<f64>>>,
    last_video_frame: Rc<Cell<Instant>>,
    pub rng: Rc<Cell<u64>>,
}

impl PlayerState {
    pub fn new(app: AppHandle) -> Self {
        let config = Config::load();
        let playback = config.playback.clone();
        let engine = Engine::new();
        engine.set_volume(playback.volume.clamp(0.0, 1.0));
        engine.prime_speed(playback.speed);
        engine.set_backend(playback.backend);
        Self {
            engine: Rc::new(engine),
            app,
            icons: Icons::load(),
            window: Rc::new(RefCell::new(None)),
            window_pending: Rc::new(Cell::new(false)),
            queue: Signal::new(Vec::new()),
            current: Signal::new(None),
            media: Signal::new(None),
            status: Signal::new(Status::Empty),
            playing: Signal::new(false),
            position: Signal::new(0.0),
            duration: Signal::new(0.0),
            scrub: Signal::new(None),
            volume: Signal::new(playback.volume.clamp(0.0, 1.0)),
            muted: Signal::new(false),
            speed: Signal::new(playback.speed),
            repeat: Signal::new(match playback.repeat {
                RepeatMode::Off => Repeat::Off,
                RepeatMode::All => Repeat::All,
                RepeatMode::One => Repeat::One,
            }),
            shuffle: Signal::new(playback.shuffle),
            queue_open: Signal::new(false),
            subtitles_open: Signal::new(false),
            audio_open: Signal::new(false),
            subtitle: Signal::new(None),
            subtitle_style: Signal::new(SubtitleStyle::default()),
            frame: Signal::new(None),
            cover: Signal::new(None),
            backdrop: Signal::new(None),
            media_accent: Rc::new(Cell::new(None)),
            video_frames: Rc::new(Cell::new(0)),
            cover_art: Rc::new(RefCell::new(None)),
            track_serial: Rc::new(Cell::new(0)),
            media_session: Rc::new(RefCell::new(None)),
            discord: Rc::new(RefCell::new(
                config
                    .interface
                    .discord_presence
                    .then(crate::discord::Presence::new)
                    .flatten(),
            )),
            queue_scroll: ScrollController::new(0.0),
            subtitles_scroll: ScrollController::new(0.0),
            audio_scroll: ScrollController::new(0.0),
            config: Signal::new(config),
            chrome_progress: Signal::new(1.0),
            chrome_animating: Rc::new(Cell::new(false)),
            chrome_last: Rc::new(Cell::new(Instant::now())),
            config_stamp: Rc::new(Cell::new(Config::modified())),
            ticks: Rc::new(Cell::new(0)),
            controls_visible: Signal::new(true),
            last_move: Rc::new(Cell::new(Instant::now())),
            last_pos: Rc::new(Cell::new((0.0, 0.0))),
            view_size: Rc::new(Cell::new((960.0, 640.0))),
            dragging_subtitle: Rc::new(Cell::new(false)),
            load_token: Rc::new(AtomicU64::new(0)),
            pending_restart: Rc::new(Cell::new(None)),
            force_play: Rc::new(Cell::new(false)),
            pending_seek: Rc::new(Cell::new(None)),
            last_video_frame: Rc::new(Cell::new(Instant::now() - Duration::from_secs(1))),
            rng: Rc::new(Cell::new(
                SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as u64)
                    .unwrap_or(0x9e3779b9),
            )),
        }
    }

    pub fn poke(&self) {
        self.last_move.set(Instant::now());
        self.set_chrome(true);
    }

    pub(crate) fn media_accent(&self) -> Option<Color> {
        self.media_accent.get()
    }

    /// Ignores pointer deltas below 2 px because some Wayland compositors
    /// re-emit pointer motion events on every surface commit.
    pub fn poke_pointer(&self, pos: (f32, f32)) {
        let last = self.last_pos.get();
        if (pos.0 - last.0).abs() < 2.0 && (pos.1 - last.1).abs() < 2.0 {
            return;
        }
        self.last_pos.set(pos);
        if self.dragging_subtitle.get() {
            self.move_subtitle_pointer(pos);
        }
        self.last_move.set(Instant::now());
        self.set_chrome(true);
    }

    pub fn set_view_size(&self, width: f32, height: f32) {
        self.view_size.set((width, height));
    }

    pub fn start_subtitle_drag(&self) {
        self.dragging_subtitle.set(true);
    }

    pub fn move_subtitle_pointer(&self, pos: (f32, f32)) {
        if !self.dragging_subtitle.get() {
            return;
        }
        let (width, height) = self.view_size.get();
        let mut style = self.subtitle_style.peek();
        style.x = (pos.0 / width.max(1.0)).clamp(0.05, 0.95);
        style.y = (pos.1 / height.max(1.0)).clamp(0.08, 0.92);
        style.customized = true;
        self.subtitle_style.set_if_changed(style);
    }

    pub fn finish_subtitle_drag(&self) {
        if self.dragging_subtitle.replace(false) {
            self.commit_subtitle_style();
        }
    }

    fn set_chrome(&self, visible: bool) {
        if self.controls_visible.peek() == visible {
            return;
        }
        self.controls_visible.set(visible);
        if !self.config.peek().interface.animations {
            self.chrome_progress.set(if visible { 1.0 } else { 0.0 });
            return;
        }
        if !self.chrome_animating.replace(true) {
            self.chrome_last.set(Instant::now());
            self.step_chrome();
        }
    }

    /// Advances the chrome slide at roughly 60 fps until it reaches its target.
    fn step_chrome(&self) {
        let state = self.clone();
        self.app.spawn_background(
            || std::thread::sleep(Duration::from_millis(16)),
            move |_| {
                let now = Instant::now();
                let dt = now
                    .duration_since(state.chrome_last.replace(now))
                    .as_secs_f32();
                let target = if state.controls_visible.peek() {
                    1.0
                } else {
                    0.0
                };
                let current = state.chrome_progress.peek();
                let step = dt / CHROME_TRANSITION;
                let next = if target > current {
                    (current + step).min(target)
                } else {
                    (current - step).max(target)
                };
                state.chrome_progress.set(next);
                if next == target {
                    state.chrome_animating.set(false);
                } else {
                    state.step_chrome();
                }
            },
        );
    }

    /// Picks up preferences saved by the settings window.
    fn reload_config(&self) {
        let stamp = Config::modified();
        if stamp == self.config_stamp.get() {
            return;
        }
        self.config_stamp.set(stamp);
        let config = Config::load();
        let accent_changed = config.appearance.accent != self.config.peek().appearance.accent;
        let backend_changed = config.playback.backend != self.config.peek().playback.backend;
        let discord = config.interface.discord_presence;
        let discord_changed = discord != self.config.peek().interface.discord_presence;
        self.engine.set_backend(config.playback.backend);
        self.config.set(config);
        if discord_changed {
            *self.discord.borrow_mut() = if discord {
                crate::discord::Presence::new()
            } else {
                None
            };
        }
        if accent_changed {
            self.apply_theme();
        }
        if backend_changed {
            if let Some(media) = self.media.peek() {
                self.restart(media, self.current_position(), self.speed.peek());
            }
        }
    }

    fn apply_theme(&self) {
        if let Some(window) = self
            .window
            .borrow()
            .as_ref()
            .filter(|window| window.is_open())
        {
            window.set_theme(crate::app::theme_for(self.media_accent.get()));
        }
    }

    fn set_media_accent(&self, accent: Option<Color>) {
        if self.media_accent.replace(accent) != accent
            && self.config.peek().appearance.accent == Accent::Auto
        {
            self.apply_theme();
        }
    }

    /// Video frames early in a file are often black or a title card, so a
    /// few later frames get a chance until one yields a usable color.
    fn sample_video_accent(&self, pixels: &[u8]) {
        let frames = self.video_frames.get() + 1;
        self.video_frames.set(frames);
        if self.media_accent.get().is_none() && matches!(frames, 24 | 72 | 150 | 300) {
            if let Some(accent) = crate::artwork::dominant_color(pixels) {
                self.set_media_accent(Some(accent));
            }
        }
    }

    fn chrome_visible(&self) -> bool {
        match self.media.get() {
            Some(media) if media.has_video => {}
            _ => return true,
        }
        if !self.playing.get()
            || self.scrub.peek().is_some()
            || self.queue_open.get()
            || self.subtitles_open.get()
            || self.audio_open.get()
        {
            return true;
        }
        let delay = self.config.peek().interface.autohide_delay.max(1);
        self.last_move.get().elapsed() < Duration::from_secs(delay as u64)
    }

    fn next_load_token(&self) -> u64 {
        self.load_token.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn enqueue(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let mut queue = self.queue.get();
        let start = queue.len();
        for path in paths {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            queue.push(QueueItem { path, name });
        }
        self.queue.set(queue);
        if self.current.get().is_none() {
            self.play_index(start);
        }
    }

    pub fn play_now(&self, paths: Vec<PathBuf>) {
        let paths = crate::cli::collect_media(paths);
        if paths.is_empty() {
            return;
        }
        self.queue
            .set(paths.into_iter().map(Self::queue_item).collect());
        self.play_index(0);
        self.force_play.set(true);
    }

    fn queue_item(path: PathBuf) -> QueueItem {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown")
            .to_string();
        QueueItem { path, name }
    }

    pub fn enqueue_next(&self, paths: Vec<PathBuf>) {
        let paths = crate::cli::collect_media(paths);
        if paths.is_empty() {
            return;
        }
        let mut queue = self.queue.get();
        insert_next(
            &mut queue,
            self.current.get(),
            paths.into_iter().map(Self::queue_item).collect(),
        );
        self.queue.set(queue);
    }

    pub fn drop_files(&self, paths: Vec<PathBuf>) {
        let playable = crate::cli::collect_media(paths);
        if playable.is_empty() {
            self.status
                .set(Status::Error("no playable files in the drop".into()));
            return;
        }
        self.enqueue(playable);
        self.poke();
    }

    pub fn play_index(&self, index: usize) {
        let path = match self.queue.get().get(index) {
            Some(item) => item.path.clone(),
            None => return,
        };
        self.stop_session();
        self.current.set(Some(index));
        self.status.set(Status::Loading);
        let token = self.next_load_token();
        let state = self.clone();
        self.app.spawn_background(
            move || {
                engine::probe::probe(&path).map(|info| {
                    let cover = if info.attached_picture {
                        engine::cover_art(&path).and_then(|bytes| crate::artwork::decode(&bytes))
                    } else {
                        None
                    };
                    (info, cover)
                })
            },
            move |result| {
                if state.load_token.load(Ordering::Relaxed) != token {
                    return;
                }
                match result {
                    Ok((info, cover)) => {
                        let request =
                            state
                                .engine
                                .request_session(info.clone(), 0.0, state.speed.peek());
                        let generation = state.engine.generation();
                        let ready = state.clone();
                        state.app.spawn_background(
                            move || request.spawn(),
                            move |result| {
                                if ready.load_token.load(Ordering::Relaxed) != token
                                    || ready.engine.generation() != generation
                                {
                                    return;
                                }
                                match result {
                                    Ok(Some(session)) => ready.start_media(info, cover, session),
                                    Ok(None) => {}
                                    Err(error) => {
                                        eprintln!("cplayer: session spawn failed: {error}");
                                        ready.status.set(Status::Error(error));
                                        ready.playing.set(false);
                                    }
                                }
                            },
                        );
                    }
                    Err(error) => {
                        state.status.set(Status::Error(error));
                        state.playing.set(false);
                    }
                }
            },
        );
    }

    fn start_media(
        &self,
        info: MediaInfo,
        cover: Option<crate::artwork::Artwork>,
        session: engine::PreparedSession,
    ) {
        self.pending_restart.set(None);
        self.pending_seek.set(None);
        let art = cover.map(|art| art.into_images());
        self.set_media_accent(art.as_ref().and_then(|art| art.accent));
        *self.cover_art.borrow_mut() = art.as_ref().map(|art| art.encoded.clone());
        self.track_serial.set(self.track_serial.get() + 1);
        let (cover, backdrop) = art.map(|art| (art.cover, art.backdrop)).unwrap_or_default();
        self.cover.set(cover);
        self.backdrop.set(backdrop);
        let autoplay = self.force_play.replace(false) || self.config.peek().playback.autoplay;
        self.engine.activate(session, Some(!autoplay));
        self.duration.set(info.duration);
        self.position.set(0.0);
        self.media.set(Some(info));
        self.status.set(Status::Playing);
        self.playing.set(autoplay);
    }

    fn stop_session(&self) {
        self.next_load_token();
        self.pending_restart.set(None);
        self.force_play.set(false);
        self.pending_seek.set(None);
        self.engine.stop();
        self.subtitle.set(None);
        self.subtitle_style.set(SubtitleStyle::default());
        self.dragging_subtitle.set(false);
        self.subtitles_open.set(false);
        self.audio_open.set(false);
        self.frame.set(None);
        self.cover.set(None);
        self.backdrop.set(None);
        *self.cover_art.borrow_mut() = None;
        self.video_frames.set(0);
        self.position.set(0.0);
        self.duration.set(0.0);
        self.scrub.set(None);
    }

    pub fn toggle(&self) {
        self.set_playing(!self.playing.get());
    }

    pub fn set_playing(&self, playing: bool) {
        if self.media.get().is_none() {
            return;
        }
        if playing && self.engine.ended() {
            self.seek_to(0.0);
        }
        self.playing.set(playing);
        self.engine.set_paused(!playing);
    }

    pub fn shown_position(&self) -> f64 {
        if let Some(scrub) = self.scrub.get() {
            scrub
        } else {
            self.position.get()
        }
    }

    pub fn seek_to(&self, pos: f64) {
        let Some(media) = self.media.get() else {
            return;
        };
        let pos = pos.clamp(
            0.0,
            if self.duration.get() > 0.0 {
                self.duration.get()
            } else {
                f64::MAX
            },
        );
        self.position.set(pos);
        if self.engine.seek_live(pos) {
            self.announce_seek(pos);
            return;
        }
        self.pending_seek.set(Some(pos));
        self.restart(media, pos, self.speed.peek());
        self.announce_seek(pos);
    }

    pub fn seek_by(&self, delta: f64) {
        let pos = (self.current_position() + delta).max(0.0);
        self.seek_to(pos);
    }

    fn current_position(&self) -> f64 {
        if self.pending_restart.get().is_some() {
            self.pending_seek
                .get()
                .unwrap_or_else(|| self.engine.position())
        } else {
            self.engine.position()
        }
    }

    fn restart(&self, media: MediaInfo, pos: f64, speed: f32) {
        let request = self.engine.request_session(media, pos, speed);
        let generation = self.engine.generation();
        self.pending_restart.set(Some(generation));
        let state = self.clone();
        self.app.spawn_background(
            move || request.spawn(),
            move |result| {
                if state.pending_restart.get() != Some(generation)
                    || state.engine.generation() != generation
                {
                    return;
                }
                state.pending_restart.set(None);
                state.pending_seek.set(None);
                match result {
                    Ok(Some(session)) => {
                        state.engine.activate(session, None);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        state.engine.stop();
                        state.playing.set(false);
                        state.status.set(Status::Error(error));
                    }
                }
            },
        );
    }

    pub fn select_subtitle(&self, subtitle: Option<SubtitleSource>) {
        if self.subtitle.peek() == subtitle {
            self.subtitles_open.set(false);
            return;
        }
        self.subtitle.set(subtitle.clone());
        self.subtitles_open.set(false);
        if !self.engine.set_subtitle(subtitle) {
            if let Some(media) = self.media.peek() {
                self.restart(media, self.current_position(), self.speed.peek());
            }
        }
    }

    pub fn select_audio(&self, index: usize) {
        let Some(mut media) = self.media.peek() else {
            return;
        };
        self.audio_open.set(false);
        if media.audio_stream == Some(index)
            || !media.audio_tracks.iter().any(|track| track.index == index)
        {
            return;
        }
        media.audio_stream = Some(index);
        self.media.set(Some(media.clone()));
        if !self.engine.set_audio_live(index) {
            self.restart(media, self.current_position(), self.speed.peek());
        }
    }

    pub fn set_subtitle_position(&self, x: f32, y: f32) {
        let mut style = self.subtitle_style.peek();
        style.x = x;
        style.y = y;
        style.customized = true;
        self.subtitle_style.set(style);
        self.commit_subtitle_style();
    }

    pub fn adjust_subtitle_outline(&self, delta: f32) {
        let mut style = self.subtitle_style.peek();
        style.outline = (style.outline + delta).clamp(0.0, 8.0);
        style.customized = true;
        self.subtitle_style.set(style);
        self.commit_subtitle_style();
    }

    pub fn reset_subtitle_style(&self) {
        self.subtitle_style.set(SubtitleStyle::default());
        self.commit_subtitle_style();
    }

    fn commit_subtitle_style(&self) {
        if !self.engine.set_subtitle_style(self.subtitle_style.peek())
            && self.subtitle.peek().is_some()
        {
            if let Some(media) = self.media.peek() {
                self.restart(media, self.current_position(), self.speed.peek());
            }
        }
    }

    pub fn begin_scrub(&self, fraction: f64) {
        self.scrub.set(Some(fraction * self.duration.get()));
    }

    pub fn update_scrub(&self, fraction: f64) {
        if self.scrub.peek().is_some() {
            self.scrub.set(Some(fraction * self.duration.get()));
        }
    }

    pub fn end_scrub(&self) {
        if let Some(pos) = self.scrub.get() {
            self.scrub.set(None);
            self.seek_to(pos);
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.volume.set(volume);
        if !self.muted.peek() {
            self.engine.set_volume(volume);
        }
    }

    pub fn toggle_mute(&self) {
        let muted = !self.muted.get();
        self.muted.set(muted);
        self.engine
            .set_volume(if muted { 0.0 } else { self.volume.get() });
    }

    pub fn cycle_speed(&self, forward: bool) {
        let speeds = SPEEDS;
        let current = self.speed.get();
        let index = speeds
            .iter()
            .position(|s| (*s - current).abs() < 0.001)
            .unwrap_or(2);
        let next = if forward {
            speeds.get(index + 1).copied().unwrap_or(speeds[0])
        } else {
            index
                .checked_sub(1)
                .map(|i| speeds[i])
                .unwrap_or(*speeds.last().unwrap())
        };
        self.set_speed(next);
    }

    pub fn set_speed(&self, speed: f32) {
        let speed = speed.clamp(SPEEDS[0], SPEEDS[SPEEDS.len() - 1]);
        self.speed.set(speed);
        if let Some(media) = self.media.get() {
            if !self.engine.set_speed_live(speed) {
                self.restart(media, self.current_position(), speed);
            }
        } else {
            self.engine.prime_speed(speed);
        }
    }

    pub fn cycle_repeat(&self) {
        let next = match self.repeat.get() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
        self.repeat.set(next);
    }

    pub fn toggle_shuffle(&self) {
        self.shuffle.set(!self.shuffle.get());
    }

    pub fn advance(&self, forward: bool) -> bool {
        let queue = self.queue.get();
        let Some(index) = self.current.get() else {
            return false;
        };
        let next = if self.shuffle.get() && queue.len() > 1 {
            let mut s = self.rng.get();
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            self.rng.set(s);
            let step = 1 + (s as usize % (queue.len() - 1));
            (index + step) % queue.len()
        } else if forward {
            index + 1
        } else if index == 0 {
            return false;
        } else {
            index - 1
        };
        if next >= queue.len() {
            return false;
        }
        self.play_index(next);
        true
    }

    pub fn prev(&self) {
        if self.current_position() > 3.0 || !self.advance(false) {
            self.seek_to(0.0);
        }
    }

    fn on_track_end(&self) {
        match self.repeat.get() {
            Repeat::One => self.seek_to(0.0),
            Repeat::All => {
                if !self.advance(true) {
                    self.play_index(0);
                }
            }
            Repeat::Off => {
                if !self.advance(true) {
                    self.playing.set(false);
                    self.engine.set_paused(true);
                }
            }
        }
    }

    pub fn remove_at(&self, index: usize) {
        let mut queue = self.queue.get();
        if index >= queue.len() {
            return;
        }
        queue.remove(index);
        self.queue.set(queue.clone());
        match self.current.get() {
            Some(current) if current == index => {
                if queue.is_empty() {
                    self.current.set(None);
                    self.stop_session();
                    self.media.set(None);
                    self.set_media_accent(None);
                    self.status.set(Status::Empty);
                    self.playing.set(false);
                } else {
                    self.play_index(index.min(queue.len() - 1));
                }
            }
            Some(current) if current > index => self.current.set(Some(current - 1)),
            _ => {}
        }
    }

    pub fn start_pump(&self) {
        let rx = self.engine.events();
        let state = self.clone();
        self.app.spawn_background(
            move || rx.lock().unwrap().recv().ok(),
            move |event| {
                match event {
                    Some(EngineEvent::Exited(gen, code, tail))
                        if gen == state.engine.generation() =>
                    {
                        state.engine.mark_exited(code, tail);
                    }
                    Some(_) => {}
                    None => return,
                }
                state.start_pump();
            },
        );
    }

    pub fn start_video_pump(&self) {
        let rx = self.engine.video();
        let state = self.clone();
        self.app.spawn_background(
            move || {
                let rx = rx.lock().unwrap();
                let mut frame = rx.recv().ok()?;
                while let Ok(newest) = rx.try_recv() {
                    frame = newest;
                }
                Some(frame)
            },
            move |frame| {
                if let Some((gen, f)) = frame {
                    if gen == state.engine.video_generation()
                        && state.window.borrow().as_ref().is_some_and(|w| w.is_open())
                        && state.last_video_frame.get().elapsed() >= Duration::from_millis(40)
                    {
                        state.last_video_frame.set(Instant::now());
                        state.sample_video_accent(&f.pixels);
                        if let Ok(image) =
                            ImageData::from_premultiplied_rgba(f.width, f.height, f.pixels)
                        {
                            state.frame.set(Some(image));
                        }
                    }
                    state.start_video_pump();
                }
            },
        );
    }

    pub fn start_ticker(&self) {
        let state = self.clone();
        self.app.spawn_background(
            || {
                std::thread::sleep(Duration::from_millis(100));
            },
            move |_| {
                state.tick();
                state.start_ticker();
            },
        );
    }

    fn tick(&self) {
        let ticks = self.ticks.get().wrapping_add(1);
        self.ticks.set(ticks);
        if ticks.is_multiple_of(10) {
            self.reload_config();
        }

        self.set_chrome(self.chrome_visible());
        self.publish_media();
        if self.status.get() != Status::Playing {
            return;
        }
        if self.scrub.peek().is_none() {
            let pos = self.current_position();
            let pos = if self.duration.get() > 0.0 {
                pos.min(self.duration.get())
            } else {
                pos
            };
            self.position.set_if_changed(pos);
        }
        if self.pending_restart.get().is_some() {
            return;
        }
        if self.engine.ended() {
            self.on_track_end();
        } else if self.engine.failed() {
            let tail = self.engine.exit_tail();
            self.status.set(Status::Error(if tail.is_empty() {
                "playback failed".into()
            } else {
                tail
            }));
            self.playing.set(false);
        }
    }

    pub fn is_maximized(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|handle| handle.decorations().maximized)
    }

    pub fn toggle_maximize(&self) {
        if let Some(handle) = self.window.borrow().as_ref() {
            handle.set_maximized(!handle.decorations().maximized);
        }
    }

    pub fn open_dialog(&self) {
        self.pick_media(true);
    }

    pub fn add_dialog(&self) {
        self.pick_media(false);
    }

    fn pick_media(&self, play_now: bool) {
        let state = self.clone();
        self.app.spawn_background(
            || {
                rfd::FileDialog::new()
                    .set_title("Open media")
                    .add_filter("Media", crate::MEDIA_EXTENSIONS)
                    .add_filter("All files", &["*"])
                    .pick_files()
            },
            move |picked| {
                if let Some(paths) = picked {
                    if play_now {
                        state.play_now(paths);
                    } else {
                        state.enqueue_next(paths);
                    }
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_next_inserts_after_current_without_replacing_other_items() {
        let item = |name: &str| QueueItem {
            name: name.to_string(),
            path: PathBuf::from(name),
        };
        let mut queue = vec![item("playing"), item("later")];
        insert_next(&mut queue, Some(0), vec![item("first"), item("second")]);
        assert_eq!(
            queue
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["playing", "first", "second", "later"]
        );
        insert_next(&mut queue, None, vec![item("at end")]);
        assert_eq!(queue.last().unwrap().name, "at end");
    }
}
