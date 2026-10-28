//! MPRIS 2 over the D-Bus session bus, so desktop shells, media keys and
//! tools such as `playerctl` can show and drive the player.

use super::media::{LoopMode, MediaCommand, MediaSnapshot, PlaybackState, MAX_RATE, MIN_RATE};
use dbus::arg::{self, PropMap, RefArg, Variant};
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::blocking::LocalConnection;
use dbus::channel::{MatchingReceiver, Sender as _};
use dbus::message::MatchRule;
use dbus::Message;
use std::cell::RefCell;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS_NAME: &str = "org.mpris.MediaPlayer2.cplayer";
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
const MIME_TYPES: &[&str] = &[
    "audio/mpeg",
    "audio/flac",
    "audio/ogg",
    "audio/opus",
    "audio/mp4",
    "audio/aac",
    "audio/x-wav",
    "video/mp4",
    "video/x-matroska",
    "video/webm",
    "video/quicktime",
    "video/x-msvideo",
];

enum Update {
    Snapshot(MediaSnapshot),
    Seeked(f64),
}

/// The service may anticipate a command's effect; resending periodically
/// corrects it if the player did not follow.
const RESEND_INTERVAL: Duration = Duration::from_secs(1);

pub struct MediaSession {
    updates: mpsc::Sender<Update>,
    last: RefCell<Option<(MediaSnapshot, Instant)>>,
}

impl MediaSession {
    pub fn new(commands: mpsc::Sender<MediaCommand>) -> Option<Self> {
        let (updates, receiver) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        std::thread::Builder::new()
            .name("mpris".into())
            .spawn(move || match Service::connect(commands) {
                Ok(service) => {
                    let _ = ready.send(true);
                    service.run(receiver);
                }
                Err(error) => {
                    eprintln!("cplayer: MPRIS is unavailable: {error}");
                    let _ = ready.send(false);
                }
            })
            .ok()?;
        started.recv().ok()?.then(|| Self {
            updates,
            last: RefCell::new(None),
        })
    }

    pub fn update(&self, snapshot: &MediaSnapshot) {
        let mut last = self.last.borrow_mut();
        if last
            .as_ref()
            .is_some_and(|(sent, at)| sent == snapshot && at.elapsed() < RESEND_INTERVAL)
        {
            return;
        }
        *last = Some((snapshot.clone(), Instant::now()));
        let _ = self.updates.send(Update::Snapshot(snapshot.clone()));
    }

    pub fn seeked(&self, position: f64) {
        let _ = self.updates.send(Update::Seeked(position));
    }
}

struct Published {
    snapshot: MediaSnapshot,
    /// Cover art written to disk for `mpris:artUrl`, keyed by track id.
    art: Option<(u64, PathBuf)>,
    commands: mpsc::Sender<MediaCommand>,
}

impl Drop for Published {
    fn drop(&mut self) {
        if let Some((_, path)) = self.art.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct Service {
    connection: LocalConnection,
    published: Rc<RefCell<Published>>,
}

impl Service {
    fn connect(commands: mpsc::Sender<MediaCommand>) -> Result<Self, dbus::Error> {
        prune_stale_art();
        let connection = LocalConnection::new_session()?;
        if connection.request_name(BUS_NAME, false, false, true)? != RequestNameReply::PrimaryOwner
        {
            let instance = format!("{BUS_NAME}.instance{}", std::process::id());
            connection.request_name(instance, false, false, true)?;
        }
        let published = Rc::new(RefCell::new(Published {
            snapshot: MediaSnapshot {
                state: PlaybackState::Stopped,
                track: None,
                position: 0.0,
                volume: 1.0,
                rate: 1.0,
                shuffle: false,
                loop_mode: LoopMode::None,
                can_next: false,
                can_previous: false,
            },
            art: None,
            commands,
        }));
        let handler = published.clone();
        connection.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |message, connection| {
                let mut published = handler.borrow_mut();
                let status = published.snapshot.state;
                if let Some(reply) = handle(&message, &mut published) {
                    if !message.get_no_reply() {
                        let _ = connection.send(reply);
                    }
                }
                if published.snapshot.state != status {
                    announce(connection, &published, &["PlaybackStatus"]);
                }
                true
            }),
        );
        Ok(Self {
            connection,
            published,
        })
    }

    fn run(self, updates: mpsc::Receiver<Update>) {
        loop {
            if let Err(error) = self.connection.process(Duration::from_millis(50)) {
                eprintln!("cplayer: MPRIS connection failed: {error}");
                return;
            }
            loop {
                match updates.try_recv() {
                    Ok(Update::Snapshot(snapshot)) => self.publish(snapshot),
                    Ok(Update::Seeked(position)) => {
                        if let Ok(signal) = Message::new_signal(OBJECT_PATH, PLAYER, "Seeked") {
                            let _ = self.connection.send(signal.append1(micros(position)));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
        }
    }

    fn publish(&self, snapshot: MediaSnapshot) {
        let mut published = self.published.borrow_mut();
        refresh_art(&mut published, &snapshot);
        let old = std::mem::replace(&mut published.snapshot, snapshot);
        let new = &published.snapshot;
        let track_changed = old.track != new.track;
        let changed: Vec<&str> = [
            ("PlaybackStatus", old.state != new.state),
            ("LoopStatus", old.loop_mode != new.loop_mode),
            ("Rate", old.rate != new.rate),
            ("Shuffle", old.shuffle != new.shuffle),
            ("Volume", old.volume != new.volume),
            ("CanGoNext", old.can_next != new.can_next),
            ("CanGoPrevious", old.can_previous != new.can_previous),
            ("Metadata", track_changed),
            ("CanPlay", track_changed),
            ("CanPause", track_changed),
            ("CanSeek", track_changed),
        ]
        .into_iter()
        .filter_map(|(name, changed)| changed.then_some(name))
        .collect();
        announce(&self.connection, &published, &changed);
    }
}

fn announce(connection: &LocalConnection, published: &Published, names: &[&str]) {
    if names.is_empty() {
        return;
    }
    let properties: PropMap = names
        .iter()
        .filter_map(|name| Some((name.to_string(), player_property(published, name)?)))
        .collect();
    if let Ok(signal) = Message::new_signal(OBJECT_PATH, PROPERTIES, "PropertiesChanged") {
        let _ = connection.send(signal.append3(PLAYER, properties, Vec::<String>::new()));
    }
}

/// Reflects a transport command immediately, so a client that reads the
/// status right after calling a method sees its effect before the player's
/// next update arrives.
fn anticipate(snapshot: &mut MediaSnapshot, command: &MediaCommand) {
    if snapshot.track.is_none() {
        return;
    }
    snapshot.state = match (command, snapshot.state) {
        (MediaCommand::Play, _)
        | (MediaCommand::PlayPause, PlaybackState::Paused | PlaybackState::Stopped) => {
            PlaybackState::Playing
        }
        (MediaCommand::Pause | MediaCommand::PlayPause | MediaCommand::Stop, _) => {
            PlaybackState::Paused
        }
        (_, state) => state,
    };
}

fn refresh_art(published: &mut Published, snapshot: &MediaSnapshot) {
    let wanted = snapshot
        .track
        .as_ref()
        .and_then(|track| Some((track.id, track.art.clone()?)));
    if published.art.as_ref().map(|(id, _)| *id) == wanted.as_ref().map(|(id, _)| *id) {
        return;
    }
    if let Some((_, path)) = published.art.take() {
        let _ = std::fs::remove_file(path);
    }
    if let Some((id, bytes)) = wanted {
        let path = art_dir().join(format!("{}-{id}.png", std::process::id()));
        match std::fs::create_dir_all(art_dir()).and_then(|()| std::fs::write(&path, &*bytes)) {
            Ok(()) => published.art = Some((id, path)),
            Err(error) => eprintln!("cplayer: couldn't write cover art for MPRIS: {error}"),
        }
    }
}

/// Covers live in the per-session runtime directory, which the system clears
/// at logout; files left by players that did not exit cleanly are pruned at
/// startup.
fn art_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("cplayer-covers")
}

fn prune_stale_art() {
    let Ok(entries) = std::fs::read_dir(art_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid = name
            .to_str()
            .and_then(|name| name.split('-').next()?.parse::<u32>().ok());
        if pid.is_some_and(|pid| !Path::new(&format!("/proc/{pid}")).exists()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn boxed<T: RefArg + 'static>(value: T) -> Variant<Box<dyn RefArg>> {
    Variant(Box::new(value))
}

fn micros(seconds: f64) -> i64 {
    (seconds.max(0.0) * 1_000_000.0) as i64
}

fn track_path(id: u64) -> dbus::Path<'static> {
    dbus::Path::new(format!("/org/cplayer/track/{id}")).expect("valid object path")
}

fn root_property(name: &str) -> Option<Variant<Box<dyn RefArg>>> {
    Some(match name {
        "CanQuit" | "CanRaise" => boxed(true),
        "HasTrackList" => boxed(false),
        "Identity" => boxed("Media Player".to_string()),
        "DesktopEntry" => boxed("cplayer".to_string()),
        "SupportedUriSchemes" => boxed(vec!["file".to_string()]),
        "SupportedMimeTypes" => boxed(
            MIME_TYPES
                .iter()
                .map(|mime| mime.to_string())
                .collect::<Vec<_>>(),
        ),
        _ => return None,
    })
}

fn player_property(published: &Published, name: &str) -> Option<Variant<Box<dyn RefArg>>> {
    let snapshot = &published.snapshot;
    let loaded = snapshot.track.is_some();
    Some(match name {
        "PlaybackStatus" => boxed(
            match snapshot.state {
                PlaybackState::Playing => "Playing",
                PlaybackState::Paused => "Paused",
                PlaybackState::Stopped => "Stopped",
            }
            .to_string(),
        ),
        "LoopStatus" => boxed(
            match snapshot.loop_mode {
                LoopMode::None => "None",
                LoopMode::Track => "Track",
                LoopMode::Playlist => "Playlist",
            }
            .to_string(),
        ),
        "Rate" => boxed(snapshot.rate),
        "MinimumRate" => boxed(MIN_RATE),
        "MaximumRate" => boxed(MAX_RATE),
        "Shuffle" => boxed(snapshot.shuffle),
        "Volume" => boxed(snapshot.volume),
        "Position" => boxed(micros(snapshot.position)),
        "Metadata" => boxed(metadata(published)),
        "CanGoNext" => boxed(snapshot.can_next),
        "CanGoPrevious" => boxed(snapshot.can_previous),
        "CanPlay" | "CanPause" => boxed(loaded),
        "CanSeek" => boxed(snapshot.track.as_ref().is_some_and(|t| t.duration > 0.0)),
        "CanControl" => boxed(true),
        _ => return None,
    })
}

fn metadata(published: &Published) -> PropMap {
    let mut map = PropMap::new();
    let Some(track) = &published.snapshot.track else {
        map.insert(
            "mpris:trackid".into(),
            boxed(dbus::Path::new(NO_TRACK).expect("valid object path")),
        );
        return map;
    };
    map.insert("mpris:trackid".into(), boxed(track_path(track.id)));
    if track.duration > 0.0 {
        map.insert("mpris:length".into(), boxed(micros(track.duration)));
    }
    map.insert("xesam:title".into(), boxed(track.title.clone()));
    map.insert("xesam:url".into(), boxed(file_uri(&track.path)));
    if let Some(artist) = &track.artist {
        let artists: Vec<String> = artist
            .split(';')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
        map.insert("xesam:artist".into(), boxed(artists));
    }
    if let Some(album) = &track.album {
        map.insert("xesam:album".into(), boxed(album.clone()));
    }
    if let Some((_, path)) = &published.art {
        map.insert("mpris:artUrl".into(), boxed(file_uri(path)));
    }
    map
}

#[allow(clippy::type_complexity)]
fn properties(published: &Published, interface: &str) -> Option<PropMap> {
    let (names, lookup): (&[&str], &dyn Fn(&str) -> Option<Variant<Box<dyn RefArg>>>) =
        match interface {
            ROOT => (
                &[
                    "CanQuit",
                    "CanRaise",
                    "HasTrackList",
                    "Identity",
                    "DesktopEntry",
                    "SupportedUriSchemes",
                    "SupportedMimeTypes",
                ],
                &root_property,
            ),
            PLAYER => (
                &[
                    "PlaybackStatus",
                    "LoopStatus",
                    "Rate",
                    "MinimumRate",
                    "MaximumRate",
                    "Shuffle",
                    "Volume",
                    "Position",
                    "Metadata",
                    "CanGoNext",
                    "CanGoPrevious",
                    "CanPlay",
                    "CanPause",
                    "CanSeek",
                    "CanControl",
                ],
                &|name| player_property(published, name),
            ),
            _ => return None,
        };
    Some(
        names
            .iter()
            .filter_map(|name| Some((name.to_string(), lookup(name)?)))
            .collect(),
    )
}

fn error(message: &Message, name: &str, text: &str) -> Message {
    let text = CString::new(text).unwrap_or_default();
    message.error(&name.into(), &text)
}

fn dispatch(
    message: &Message,
    published: &mut Published,
    command: MediaCommand,
) -> Option<Message> {
    anticipate(&mut published.snapshot, &command);
    let _ = published.commands.send(command);
    Some(message.method_return())
}

fn handle(message: &Message, published: &mut Published) -> Option<Message> {
    let interface = message.interface();
    let member = message.member()?;
    let interface = interface.as_deref().unwrap_or("");

    match (interface, &*member) {
        ("org.freedesktop.DBus.Introspectable", "Introspect") => {
            Some(message.method_return().append1(INTROSPECTION))
        }
        ("org.freedesktop.DBus.Peer", "Ping") => Some(message.method_return()),
        (PROPERTIES, "Get") => {
            let (target, name): (&str, &str) = message.read2().ok()?;
            let value = match target {
                ROOT => root_property(name),
                PLAYER => player_property(published, name),
                _ => None,
            };
            Some(match value {
                Some(value) => message.method_return().append1(value),
                None => error(
                    message,
                    "org.freedesktop.DBus.Error.UnknownProperty",
                    "Unknown property",
                ),
            })
        }
        (PROPERTIES, "GetAll") => {
            let target: &str = message.read1().ok()?;
            Some(match properties(published, target) {
                Some(map) => message.method_return().append1(map),
                None => error(
                    message,
                    "org.freedesktop.DBus.Error.UnknownInterface",
                    "Unknown interface",
                ),
            })
        }
        (PROPERTIES, "Set") => {
            let (target, name, value): (&str, &str, Variant<Box<dyn RefArg>>) =
                message.read3().ok()?;
            let value = &*value.0;
            let command = match (target, name) {
                (PLAYER, "Volume") => value
                    .as_f64()
                    .map(|v| MediaCommand::SetVolume(v.clamp(0.0, 1.0))),
                (PLAYER, "Rate") => value
                    .as_f64()
                    .filter(|rate| *rate > 0.0)
                    .map(|rate| MediaCommand::SetRate(rate.clamp(MIN_RATE, MAX_RATE))),
                (PLAYER, "Shuffle") => {
                    arg::cast::<bool>(value).map(|v| MediaCommand::SetShuffle(*v))
                }
                (PLAYER, "LoopStatus") => value.as_str().and_then(|status| {
                    Some(MediaCommand::SetLoop(match status {
                        "None" => LoopMode::None,
                        "Track" => LoopMode::Track,
                        "Playlist" => LoopMode::Playlist,
                        _ => return None,
                    }))
                }),
                _ => None,
            };
            match command {
                Some(command) => dispatch(message, published, command),
                None => Some(error(
                    message,
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "Property is read-only or the value is invalid",
                )),
            }
        }
        (ROOT, "Raise") => dispatch(message, published, MediaCommand::Raise),
        (ROOT, "Quit") => dispatch(message, published, MediaCommand::Quit),
        (PLAYER, "Play") => dispatch(message, published, MediaCommand::Play),
        (PLAYER, "Pause") => dispatch(message, published, MediaCommand::Pause),
        (PLAYER, "PlayPause") => dispatch(message, published, MediaCommand::PlayPause),
        (PLAYER, "Stop") => dispatch(message, published, MediaCommand::Stop),
        (PLAYER, "Next") => dispatch(message, published, MediaCommand::Next),
        (PLAYER, "Previous") => dispatch(message, published, MediaCommand::Previous),
        (PLAYER, "Seek") => {
            let offset: i64 = message.read1().ok()?;
            dispatch(
                message,
                published,
                MediaCommand::SeekBy(offset as f64 / 1_000_000.0),
            )
        }
        (PLAYER, "SetPosition") => {
            let (track, position): (dbus::Path<'_>, i64) = message.read2().ok()?;
            let current = published.snapshot.track.as_ref().map(|t| track_path(t.id));
            if current.as_ref() == Some(&track) && position >= 0 {
                dispatch(
                    message,
                    published,
                    MediaCommand::SetPosition(position as f64 / 1_000_000.0),
                )
            } else {
                Some(message.method_return())
            }
        }
        (PLAYER, "OpenUri") => {
            let uri: &str = message.read1().ok()?;
            match parse_file_uri(uri) {
                Some(path) => dispatch(message, published, MediaCommand::Open(path)),
                None => Some(error(
                    message,
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "Only local file URIs are supported",
                )),
            }
        }
        _ => Some(error(
            message,
            "org.freedesktop.DBus.Error.UnknownMethod",
            "Unknown method",
        )),
    }
}

fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

fn parse_file_uri(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = uri.strip_prefix("file://")?;
    let path = rest.get(rest.find('/')?..)?.as_bytes();
    let mut bytes = Vec::with_capacity(path.len());
    let mut index = 0;
    while index < path.len() {
        if path[index] == b'%' && index + 2 < path.len() {
            let hex = std::str::from_utf8(path.get(index + 1..index + 3)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            bytes.push(path[index]);
            index += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

const INTROSPECTION: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect"><arg name="xml" type="s" direction="out"/></method>
  </interface>
  <interface name="org.freedesktop.DBus.Peer">
    <method name="Ping"/>
  </interface>
  <interface name="org.freedesktop.DBus.Properties">
    <method name="Get"><arg name="interface" type="s" direction="in"/><arg name="name" type="s" direction="in"/><arg name="value" type="v" direction="out"/></method>
    <method name="GetAll"><arg name="interface" type="s" direction="in"/><arg name="properties" type="a{sv}" direction="out"/></method>
    <method name="Set"><arg name="interface" type="s" direction="in"/><arg name="name" type="s" direction="in"/><arg name="value" type="v" direction="in"/></method>
    <signal name="PropertiesChanged"><arg name="interface" type="s"/><arg name="changed" type="a{sv}"/><arg name="invalidated" type="as"/></signal>
  </interface>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/>
    <method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/>
    <property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/>
    <property name="Identity" type="s" access="read"/>
    <property name="DesktopEntry" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/>
    <property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/>
    <method name="Previous"/>
    <method name="Pause"/>
    <method name="PlayPause"/>
    <method name="Stop"/>
    <method name="Play"/>
    <method name="Seek"><arg name="Offset" type="x" direction="in"/></method>
    <method name="SetPosition"><arg name="TrackId" type="o" direction="in"/><arg name="Position" type="x" direction="in"/></method>
    <method name="OpenUri"><arg name="Uri" type="s" direction="in"/></method>
    <signal name="Seeked"><arg name="Position" type="x"/></signal>
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="LoopStatus" type="s" access="readwrite"/>
    <property name="Rate" type="d" access="readwrite"/>
    <property name="Shuffle" type="b" access="readwrite"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Volume" type="d" access="readwrite"/>
    <property name="Position" type="x" access="read"/>
    <property name="MinimumRate" type="d" access="read"/>
    <property name="MaximumRate" type="d" access="read"/>
    <property name="CanGoNext" type="b" access="read"/>
    <property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/>
    <property name="CanPause" type="b" access="read"/>
    <property name="CanSeek" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/>
  </interface>
</node>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_round_trip_unusual_names() {
        let path = Path::new("/home/me/Música/bôa - Duvet #1.flac");
        let uri = file_uri(path);
        assert_eq!(
            uri,
            "file:///home/me/M%C3%BAsica/b%C3%B4a%20-%20Duvet%20%231.flac"
        );
        assert_eq!(parse_file_uri(&uri).as_deref(), Some(path));
        assert_eq!(
            parse_file_uri("file://localhost/tmp/a%20b").as_deref(),
            Some(Path::new("/tmp/a b"))
        );
        assert!(parse_file_uri("https://example.com/a.mp3").is_none());
    }
}
