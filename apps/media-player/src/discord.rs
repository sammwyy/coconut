//! Shares the now-playing track with Discord over its local IPC socket.
//! Like the MPRIS/SMTC integrations, snapshots travel through a channel to a
//! worker thread that owns the client, reconnects as needed and clears the
//! presence when the feature is disabled or the player exits.

use crate::backends::media::{MediaSnapshot, PlaybackState, Track};
use crate::config::{DiscordTitle, InterfaceConfig};
use creamui_image::ImageData;
use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The Discord application this presence reports against; its name shows up
/// as "Listening to <name>" / "Watching <name>". Can be overridden with the
/// CPLAYER_DISCORD_CLIENT_ID environment variable.
const CLIENT_ID: &str = "1562874215139508285";

/// Fallback large image and the small badge: Discord cannot receive image
/// bytes over IPC, so these name art assets uploaded to the Discord
/// application (Rich Presence → Art Assets). CPLAYER_DISCORD_ASSET overrides
/// the key.
const ASSET_KEY: &str = "cplayer";

/// Anonymous image host used to hand cover art to Discord as a public URL;
/// Discord's proxy fetches it once and caches it for the presence.
const COVER_HOST: &str = "https://catbox.moe/user/api.php";
/// Longest edge of the JPEG thumbnail uploaded for each cover.
const COVER_SIDE: u32 = 384;
/// Bound on remembered cover URLs so repeated queue changes cannot grow it.
const COVER_CACHE_LIMIT: usize = 128;

/// How long the worker waits before retrying a failed connection.
const RETRY_INTERVAL: Duration = Duration::from_secs(15);
/// Snapshot poll interval; also bounds how often reconnects are attempted.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// What would visibly change on Discord, so `update` only hits the IPC socket
/// when it has to. The playing timestamps are wall-clock seconds, so they
/// stay constant while playback advances normally and only shift on seeks or
/// rate changes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Signature {
    Idle,
    Paused {
        track: u64,
        covers: bool,
        title: DiscordTitle,
    },
    Playing {
        track: u64,
        start: i64,
        end: i64,
        covers: bool,
        title: DiscordTitle,
    },
}

/// A snapshot bundled with the preferences that shape its activity, so the
/// worker can re-publish the same thing after a reconnect.
struct Update {
    snapshot: MediaSnapshot,
    interface: InterfaceConfig,
}

pub struct Presence {
    updates: mpsc::Sender<Update>,
    last: RefCell<Option<Signature>>,
}

impl Presence {
    pub fn new() -> Option<Self> {
        let client_id = std::env::var("CPLAYER_DISCORD_CLIENT_ID")
            .ok()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| CLIENT_ID.to_string());
        if client_id.is_empty() {
            eprintln!("cplayer: Discord presence needs an application id");
            return None;
        }
        let (updates, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord".into())
            .spawn(move || run(receiver, client_id))
            .ok()?;
        Some(Self {
            updates,
            last: RefCell::new(None),
        })
    }

    pub fn update(&self, snapshot: &MediaSnapshot, interface: &InterfaceConfig) {
        let signature = signature(snapshot, interface);
        let mut last = self.last.borrow_mut();
        if *last == Some(signature) {
            return;
        }
        *last = Some(signature);
        let _ = self.updates.send(Update {
            snapshot: snapshot.clone(),
            interface: interface.clone(),
        });
    }
}

fn signature(snapshot: &MediaSnapshot, interface: &InterfaceConfig) -> Signature {
    let Some(track) = snapshot.track.as_ref() else {
        return Signature::Idle;
    };
    if interface.discord_metadata_only && track.artist.is_none() {
        return Signature::Idle;
    }
    let covers = interface.discord_covers;
    let title = interface.discord_title;
    if snapshot.state != PlaybackState::Playing {
        return Signature::Paused {
            track: track.id,
            covers,
            title,
        };
    }
    let (start, end) = timestamps(snapshot).unwrap_or((0, 0));
    Signature::Playing {
        track: track.id,
        start,
        end,
        covers,
        title,
    }
}

/// Wall-clock activity window in Unix seconds. Discord counts down from
/// `end` on its own, so this only needs recomputing after seeks and rate
/// changes.
fn timestamps(snapshot: &MediaSnapshot) -> Option<(i64, i64)> {
    let track = snapshot.track.as_ref()?;
    if snapshot.state != PlaybackState::Playing || track.duration <= 0.0 {
        return None;
    }
    let rate = snapshot.rate.max(0.1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    let start = now - snapshot.position / rate;
    let end = start + track.duration / rate;
    Some((start.round() as i64, end.round() as i64))
}

/// The presence headline, composed from the tagged fields per the title
/// mode. Untagged tracks never reach it: they always show the bare filename.
fn headline(track: &Track, mode: DiscordTitle) -> String {
    let artist = track.artist.as_deref().unwrap_or_default();
    match mode {
        DiscordTitle::Title => track.title.clone(),
        DiscordTitle::Artist => artist.to_string(),
        DiscordTitle::ArtistTitle => format!("{artist} - {}", track.title),
        DiscordTitle::TitleArtist => format!("{} - {artist}", track.title),
    }
}

/// The second presence line, paired with whatever the title mode left out.
fn state_line(snapshot: &MediaSnapshot, mode: DiscordTitle) -> Option<String> {
    let track = snapshot.track.as_ref()?;
    let paused = snapshot.state != PlaybackState::Playing;
    let line = match mode {
        DiscordTitle::Title => track.artist.as_ref().map(|artist| format!("by {artist}")),
        DiscordTitle::Artist => Some(track.title.clone()),
        DiscordTitle::ArtistTitle | DiscordTitle::TitleArtist => track.album.clone(),
    };
    match (paused, line) {
        (false, line) => line,
        (true, Some(line)) => Some(format!("Paused — {line}")),
        (true, None) => Some("Paused".to_string()),
    }
}

fn asset_key() -> String {
    std::env::var("CPLAYER_DISCORD_ASSET")
        .ok()
        .filter(|key| !key.is_empty())
        .unwrap_or_else(|| ASSET_KEY.to_string())
}

fn activity(update: &Update, cover: Option<String>) -> Option<Activity<'static>> {
    let snapshot = &update.snapshot;
    let track = snapshot.track.as_ref()?;
    let tagged = track.artist.is_some();
    if update.interface.discord_metadata_only && !tagged {
        return None;
    }
    let mode = update.interface.discord_title;
    let details = if tagged {
        headline(track, mode)
    } else {
        track
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| track.title.clone())
    };
    let mut activity = Activity::new()
        .details(details)
        .activity_type(if track.video {
            ActivityType::Watching
        } else {
            ActivityType::Listening
        })
        .status_display_type(StatusDisplayType::Details);
    if let Some(state) = state_line(snapshot, mode) {
        activity = activity.state(state);
    }
    if let Some((start, end)) = timestamps(snapshot) {
        activity = activity.timestamps(Timestamps::new().start(start).end(end));
    }
    activity = activity.assets(
        Assets::new()
            .large_image(cover.unwrap_or_else(asset_key))
            .large_text(track.album.clone().unwrap_or_else(|| track.title.clone())),
    );
    Some(activity)
}

/// Decodes the embedded cover into a bounded JPEG thumbnail for upload.
fn encode_cover(art: &[u8]) -> Option<Vec<u8>> {
    let data = ImageData::from_bytes(art).ok()?;
    let (width, height) = (data.width(), data.height());
    if width == 0 || height == 0 {
        return None;
    }
    let pixels = data.pixels();
    let mut rgb = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::new(width, height);
    for (dst, src) in rgb.pixels_mut().zip(pixels.chunks_exact(4)) {
        *dst = image::Rgb([src[0], src[1], src[2]]);
    }
    let thumb = image::imageops::thumbnail(&rgb, COVER_SIDE, COVER_SIDE);
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82)
        .encode_image(&thumb)
        .ok()?;
    Some(out.into_inner())
}

/// Hands the cover JPEG to an anonymous host through curl and returns the
/// public URL Discord should fetch. Failures fall back to the static asset.
fn upload_cover(art: &[u8]) -> Option<String> {
    let jpeg = encode_cover(art)?;
    let mut child = crate::backends::background_command("curl")
        .args([
            "-fsS",
            "--max-time",
            "15",
            "-F",
            "reqtype=fileupload",
            "-F",
            "fileToUpload=@-;type=image/jpeg;filename=cover.jpg",
            COVER_HOST,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(&jpeg).ok()?;
    let output = child.wait_with_output().ok()?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && url.starts_with("https://")).then_some(url)
}

/// Resolved cover URLs per file, including failures so a bad upload is not
/// retried on every pause/resume.
type CoverCache = HashMap<PathBuf, Option<String>>;

fn publish(
    client: &mut DiscordIpcClient,
    update: &Update,
    covers: &mut CoverCache,
) -> Result<(), String> {
    let cover = update.snapshot.track.as_ref().and_then(|track| {
        let shown = track.artist.is_some() || !update.interface.discord_metadata_only;
        if !shown || !update.interface.discord_covers {
            return None;
        }
        track.art.as_ref().and_then(|art| {
            if covers.len() >= COVER_CACHE_LIMIT {
                covers.clear();
            }
            covers
                .entry(track.path.clone())
                .or_insert_with(|| upload_cover(art))
                .clone()
        })
    });
    match activity(update, cover) {
        Some(activity) => client.set_activity(activity),
        None => client.clear_activity(),
    }
    .map_err(|error| error.to_string())
}

fn connect(client_id: &str) -> Result<DiscordIpcClient, String> {
    let mut client = DiscordIpcClient::new(client_id);
    client.connect().map_err(|error| error.to_string())?;
    Ok(client)
}

fn run(updates: mpsc::Receiver<Update>, client_id: String) {
    let mut client = None;
    let mut next_attempt = Instant::now();
    let mut reported = false;
    let mut latest = None;
    let mut covers = HashMap::new();
    loop {
        if client.is_none() && next_attempt <= Instant::now() {
            match connect(&client_id) {
                Ok(mut connected) => {
                    reported = false;
                    match latest
                        .as_ref()
                        .map(|update| publish(&mut connected, update, &mut covers))
                    {
                        Some(Err(error)) => {
                            eprintln!("cplayer: Discord IPC failed: {error}");
                            next_attempt = Instant::now() + RETRY_INTERVAL;
                        }
                        _ => client = Some(connected),
                    }
                }
                Err(error) => {
                    if !reported {
                        eprintln!("cplayer: Discord IPC unavailable: {error}");
                        reported = true;
                    }
                    next_attempt = Instant::now() + RETRY_INTERVAL;
                }
            }
        }
        match updates.recv_timeout(POLL_INTERVAL) {
            Ok(update) => {
                let failed = match client
                    .as_mut()
                    .map(|client| publish(client, &update, &mut covers))
                {
                    Some(Err(error)) => {
                        eprintln!("cplayer: Discord IPC failed: {error}");
                        true
                    }
                    _ => false,
                };
                if failed {
                    client = None;
                    next_attempt = Instant::now() + RETRY_INTERVAL;
                }
                latest = Some(update);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some(mut client) = client.take() {
        let _ = client.clear_activity();
        let _ = client.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::media::{LoopMode, Track};
    use std::path::PathBuf;

    fn interface() -> InterfaceConfig {
        InterfaceConfig::default()
    }

    fn snapshot(state: PlaybackState, position: f64, video: bool) -> MediaSnapshot {
        MediaSnapshot {
            state,
            track: Some(Track {
                id: 7,
                path: PathBuf::from("media/Title.mp3"),
                title: "Title".into(),
                artist: Some("Artist".into()),
                album: Some("Album".into()),
                duration: 200.0,
                video,
                art: None,
            }),
            position,
            volume: 1.0,
            rate: 1.0,
            shuffle: false,
            loop_mode: LoopMode::None,
            can_next: true,
            can_previous: true,
        }
    }

    fn update(snapshot: &MediaSnapshot, interface: &InterfaceConfig) -> Update {
        Update {
            snapshot: snapshot.clone(),
            interface: interface.clone(),
        }
    }

    #[test]
    fn no_track_means_no_activity() {
        let mut snapshot = snapshot(PlaybackState::Stopped, 0.0, false);
        snapshot.track = None;
        assert!(activity(&update(&snapshot, &interface()), None).is_none());
        assert_eq!(signature(&snapshot, &interface()), Signature::Idle);
    }

    #[test]
    fn playing_describes_track_and_progress() {
        let snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        let activity = activity(&update(&snapshot, &interface()), None).unwrap();
        let value = serde_json::to_value(&activity).unwrap();
        assert_eq!(value["details"], "Artist - Title");
        assert_eq!(value["state"], "Album");
        assert_eq!(value["type"], 2); // Listening
        assert_eq!(value["status_display_type"], 2);
        assert_eq!(value["assets"]["large_image"], ASSET_KEY);
        assert_eq!(value["assets"]["large_text"], "Album");
        assert!(value["assets"].get("small_image").is_none());
        let window = value["timestamps"].as_object().unwrap();
        // The window spans the track's whole length around the current time.
        assert_eq!(
            window["end"].as_i64().unwrap() - window["start"].as_i64().unwrap(),
            200
        );
    }

    #[test]
    fn each_title_mode_moves_the_remaining_field_to_the_second_line() {
        let snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        let cases = [
            (DiscordTitle::Title, "Title", Some("by Artist")),
            (DiscordTitle::Artist, "Artist", Some("Title")),
            (DiscordTitle::ArtistTitle, "Artist - Title", Some("Album")),
            (DiscordTitle::TitleArtist, "Title - Artist", Some("Album")),
        ];
        for (mode, details, state) in cases {
            let mut interface = interface();
            interface.discord_title = mode;
            let value =
                serde_json::to_value(activity(&update(&snapshot, &interface), None).unwrap())
                    .unwrap();
            assert_eq!(value["details"], details, "{mode:?}");
            assert_eq!(
                value["state"].as_str().map(str::to_string),
                state.map(str::to_string),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn uploaded_cover_beats_the_static_asset() {
        let snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        let activity = activity(
            &update(&snapshot, &interface()),
            Some("https://files.catbox.moe/abc.jpg".into()),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&activity).unwrap()["assets"]["large_image"],
            "https://files.catbox.moe/abc.jpg"
        );
    }

    #[test]
    fn video_uses_watching_type() {
        let snapshot = snapshot(PlaybackState::Playing, 60.0, true);
        let activity = activity(&update(&snapshot, &interface()), None).unwrap();
        assert_eq!(serde_json::to_value(&activity).unwrap()["type"], 3);
    }

    #[test]
    fn untagged_files_show_the_filename_without_extension() {
        let mut interface = interface();
        interface.discord_metadata_only = false;
        let mut snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        let track = snapshot.track.as_mut().unwrap();
        track.artist = None;
        track.album = None;
        track.path = PathBuf::from("music/Some Song.opus");
        let value =
            serde_json::to_value(activity(&update(&snapshot, &interface), None).unwrap()).unwrap();
        assert_eq!(value["details"], "Some Song");
        assert!(value.get("state").is_none());
    }

    #[test]
    fn metadata_only_hides_untagged_files() {
        let mut snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        snapshot.track.as_mut().unwrap().artist = None;
        assert!(activity(&update(&snapshot, &interface()), None).is_none());
        assert_eq!(signature(&snapshot, &interface()), Signature::Idle);
        let mut lax = interface();
        lax.discord_metadata_only = false;
        assert!(activity(&update(&snapshot, &lax), None).is_some());
        assert_ne!(signature(&snapshot, &lax), Signature::Idle);
    }

    #[test]
    fn paused_drops_the_time_bar_and_says_so() {
        let snapshot = snapshot(PlaybackState::Paused, 60.0, false);
        let value = serde_json::to_value(activity(&update(&snapshot, &interface()), None).unwrap())
            .unwrap();
        assert_eq!(value["state"], "Paused — Album");
        assert!(value.get("timestamps").is_none());
    }

    #[test]
    fn signature_changes_on_pause_and_seek_only() {
        let playing = snapshot(PlaybackState::Playing, 60.0, false);
        assert_ne!(
            signature(&playing, &interface()),
            signature(&snapshot(PlaybackState::Paused, 60.0, false), &interface())
        );
        // A seek shifts the wall-clock window, normal ticking does not.
        assert_ne!(
            signature(&playing, &interface()),
            signature(&snapshot(PlaybackState::Playing, 90.0, false), &interface())
        );
    }

    #[test]
    fn cover_toggle_changes_the_signature() {
        let playing = snapshot(PlaybackState::Playing, 60.0, false);
        let mut interface = interface();
        interface.discord_covers = false;
        assert_ne!(
            signature(&playing, &interface),
            signature(&playing, &InterfaceConfig::default())
        );
    }

    #[test]
    fn unknown_duration_hides_the_time_bar() {
        let mut snapshot = snapshot(PlaybackState::Playing, 60.0, false);
        snapshot.track.as_mut().unwrap().duration = 0.0;
        assert!(timestamps(&snapshot).is_none());
    }
}
