//! Windows System Media Transport Controls: the media flyout, lock screen
//! and hardware media keys.

use super::media::{LoopMode, MediaCommand, MediaSnapshot, PlaybackState, MAX_RATE, MIN_RATE};
use std::cell::{Cell, RefCell};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::core::{Result, HSTRING};
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Media::{
    AutoRepeatModeChangeRequestedEventArgs, MediaPlaybackAutoRepeatMode, MediaPlaybackStatus,
    MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
    PlaybackRateChangeRequestedEventArgs, ShuffleEnabledChangeRequestedEventArgs,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::{
    DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference,
};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GetWindow, IsWindowVisible, GW_OWNER,
};

/// Seconds skipped by the fast-forward and rewind buttons.
const SKIP_SECONDS: f64 = 10.0;
/// The flyout extrapolates the position itself; refresh it now and then.
const TIMELINE_INTERVAL: Duration = Duration::from_secs(5);

pub struct MediaSession {
    controls: SystemMediaTransportControls,
    last: RefCell<Option<MediaSnapshot>>,
    timeline_sent: Cell<Option<Instant>>,
}

impl MediaSession {
    pub fn new(commands: mpsc::Sender<MediaCommand>) -> Option<Self> {
        let window = main_window()?;
        Self::create(window, commands)
            .map_err(|error| eprintln!("cplayer: media controls are unavailable: {error}"))
            .ok()
    }

    fn create(window: HWND, commands: mpsc::Sender<MediaCommand>) -> Result<Self> {
        let interop = windows::core::factory::<
            SystemMediaTransportControls,
            ISystemMediaTransportControlsInterop,
        >()?;
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(window)? };
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsStopEnabled(true)?;
        controls.SetIsFastForwardEnabled(true)?;
        controls.SetIsRewindEnabled(true)?;
        controls.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;

        let send = commands.clone();
        controls.ButtonPressed(&TypedEventHandler::new(
            move |_, args: &Option<SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(args) = args {
                    let command = match args.Button()? {
                        SystemMediaTransportControlsButton::Play => MediaCommand::Play,
                        SystemMediaTransportControlsButton::Pause => MediaCommand::Pause,
                        SystemMediaTransportControlsButton::Stop => MediaCommand::Stop,
                        SystemMediaTransportControlsButton::Next => MediaCommand::Next,
                        SystemMediaTransportControlsButton::Previous => MediaCommand::Previous,
                        SystemMediaTransportControlsButton::FastForward => {
                            MediaCommand::SeekBy(SKIP_SECONDS)
                        }
                        SystemMediaTransportControlsButton::Rewind => {
                            MediaCommand::SeekBy(-SKIP_SECONDS)
                        }
                        _ => return Ok(()),
                    };
                    let _ = send.send(command);
                }
                Ok(())
            },
        ))?;
        let send = commands.clone();
        controls.PlaybackPositionChangeRequested(&TypedEventHandler::new(
            move |_, args: &Option<PlaybackPositionChangeRequestedEventArgs>| {
                if let Some(args) = args {
                    let position = args.RequestedPlaybackPosition()?.Duration as f64 / 1e7;
                    let _ = send.send(MediaCommand::SetPosition(position.max(0.0)));
                }
                Ok(())
            },
        ))?;
        let send = commands.clone();
        controls.ShuffleEnabledChangeRequested(&TypedEventHandler::new(
            move |_, args: &Option<ShuffleEnabledChangeRequestedEventArgs>| {
                if let Some(args) = args {
                    let _ = send.send(MediaCommand::SetShuffle(args.RequestedShuffleEnabled()?));
                }
                Ok(())
            },
        ))?;
        let send = commands.clone();
        controls.AutoRepeatModeChangeRequested(&TypedEventHandler::new(
            move |_, args: &Option<AutoRepeatModeChangeRequestedEventArgs>| {
                if let Some(args) = args {
                    let mode = match args.RequestedAutoRepeatMode()? {
                        MediaPlaybackAutoRepeatMode::Track => LoopMode::Track,
                        MediaPlaybackAutoRepeatMode::List => LoopMode::Playlist,
                        _ => LoopMode::None,
                    };
                    let _ = send.send(MediaCommand::SetLoop(mode));
                }
                Ok(())
            },
        ))?;
        let send = commands;
        controls.PlaybackRateChangeRequested(&TypedEventHandler::new(
            move |_, args: &Option<PlaybackRateChangeRequestedEventArgs>| {
                if let Some(args) = args {
                    let rate = args.RequestedPlaybackRate()?.clamp(MIN_RATE, MAX_RATE);
                    let _ = send.send(MediaCommand::SetRate(rate));
                }
                Ok(())
            },
        ))?;
        Ok(Self {
            controls,
            last: RefCell::new(None),
            timeline_sent: Cell::new(None),
        })
    }

    pub fn update(&self, snapshot: &MediaSnapshot) {
        if let Err(error) = self.apply(snapshot) {
            eprintln!("cplayer: couldn't update media controls: {error}");
        }
    }

    pub fn seeked(&self, _position: f64) {
        self.timeline_sent.set(None);
    }

    fn apply(&self, snapshot: &MediaSnapshot) -> Result<()> {
        let mut last = self.last.borrow_mut();
        let previous = last.replace(snapshot.clone());
        let unchanged = previous
            .as_ref()
            .is_some_and(|previous| previous.same_besides_position(snapshot));
        let track_changed = previous.as_ref().map(|p| &p.track) != Some(&snapshot.track);
        if !unchanged {
            let controls = &self.controls;
            controls.SetPlaybackStatus(match (snapshot.state, &snapshot.track) {
                (_, None) => MediaPlaybackStatus::Closed,
                (PlaybackState::Playing, _) => MediaPlaybackStatus::Playing,
                (PlaybackState::Paused, _) => MediaPlaybackStatus::Paused,
                (PlaybackState::Stopped, _) => MediaPlaybackStatus::Stopped,
            })?;
            controls.SetIsNextEnabled(snapshot.can_next)?;
            controls.SetIsPreviousEnabled(snapshot.can_previous)?;
            controls.SetShuffleEnabled(snapshot.shuffle)?;
            controls.SetAutoRepeatMode(match snapshot.loop_mode {
                LoopMode::None => MediaPlaybackAutoRepeatMode::None,
                LoopMode::Track => MediaPlaybackAutoRepeatMode::Track,
                LoopMode::Playlist => MediaPlaybackAutoRepeatMode::List,
            })?;
            controls.SetPlaybackRate(snapshot.rate)?;
            self.timeline_sent.set(None);
        }
        if track_changed {
            self.show_track(snapshot)?;
        }
        let due = self
            .timeline_sent
            .get()
            .map_or(true, |sent| sent.elapsed() >= TIMELINE_INTERVAL);
        if due {
            self.send_timeline(snapshot)?;
        }
        Ok(())
    }

    fn show_track(&self, snapshot: &MediaSnapshot) -> Result<()> {
        let updater = self.controls.DisplayUpdater()?;
        updater.ClearAll()?;
        if let Some(track) = &snapshot.track {
            if track.video {
                updater.SetType(MediaPlaybackType::Video)?;
                let properties = updater.VideoProperties()?;
                properties.SetTitle(&HSTRING::from(track.title.as_str()))?;
                if let Some(artist) = &track.artist {
                    properties.SetSubtitle(&HSTRING::from(artist.as_str()))?;
                }
            } else {
                updater.SetType(MediaPlaybackType::Music)?;
                let properties = updater.MusicProperties()?;
                properties.SetTitle(&HSTRING::from(track.title.as_str()))?;
                if let Some(artist) = &track.artist {
                    properties.SetArtist(&HSTRING::from(artist.replace(';', ", ")))?;
                }
                if let Some(album) = &track.album {
                    properties.SetAlbumTitle(&HSTRING::from(album.as_str()))?;
                }
            }
            if let Some(art) = &track.art {
                updater.SetThumbnail(&thumbnail(art)?)?;
            }
        }
        updater.Update()
    }

    fn send_timeline(&self, snapshot: &MediaSnapshot) -> Result<()> {
        let duration = snapshot.track.as_ref().map_or(0.0, |track| track.duration);
        let span = |seconds: f64| TimeSpan {
            Duration: (seconds.max(0.0) * 1e7) as i64,
        };
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(span(0.0))?;
        timeline.SetMinSeekTime(span(0.0))?;
        timeline.SetEndTime(span(duration))?;
        timeline.SetMaxSeekTime(span(duration))?;
        timeline.SetPosition(span(snapshot.position.min(duration.max(snapshot.position))))?;
        self.controls.UpdateTimelineProperties(&timeline)?;
        self.timeline_sent.set(Some(Instant::now()));
        Ok(())
    }
}

fn thumbnail(bytes: &[u8]) -> Result<RandomAccessStreamReference> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.get()?;
    writer.FlushAsync()?.get()?;
    writer.DetachStream()?;
    RandomAccessStreamReference::CreateFromStream(&stream)
}

/// The player's top-level window, created on this (the UI) thread.
fn main_window() -> Option<HWND> {
    unsafe extern "system" fn visit(window: HWND, found: LPARAM) -> BOOL {
        if IsWindowVisible(window).as_bool() && GetWindow(window, GW_OWNER).0 == 0 {
            *(found.0 as *mut HWND) = window;
            return false.into();
        }
        true.into()
    }
    let mut found = HWND(0);
    unsafe {
        let _ = EnumThreadWindows(
            GetCurrentThreadId(),
            Some(visit),
            LPARAM(&mut found as *mut HWND as isize),
        );
    }
    (found.0 != 0).then_some(found)
}
