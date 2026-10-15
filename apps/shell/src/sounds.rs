#[cfg(not(test))]
use std::{
    io,
    path::Path,
    process::{Command, Stdio},
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};

use delib::sounds::{SoundRequest, SoundResolution, SoundResolver};

#[cfg(not(test))]
pub fn play(theme: String, event: String) {
    static REQUESTS: OnceLock<mpsc::SyncSender<(String, String)>> = OnceLock::new();
    let requests = REQUESTS.get_or_init(|| {
        let (requests, events) = mpsc::sync_channel::<(String, String)>(8);
        std::thread::spawn(move || {
            let mut previous: Option<((String, String), Instant)> = None;
            for request in events {
                if previous.as_ref().is_some_and(|(last, time)| {
                    last == &request && time.elapsed() < Duration::from_millis(100)
                }) {
                    continue;
                }
                previous = Some((request.clone(), Instant::now()));
                let resolver = SoundResolver::new(request.0);
                if let Some(SoundResolution::File(path)) =
                    resolver.resolve(&SoundRequest::stereo(request.1))
                {
                    if let Err(error) = play_file(&path) {
                        eprintln!("sounds: {}: {error}", path.display());
                    }
                }
            }
        });
        requests
    });
    match requests.try_send((theme, event)) {
        Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
        Err(mpsc::TrySendError::Disconnected(_)) => {
            eprintln!("sounds: playback worker disconnected")
        }
    }
}

#[cfg(not(test))]
fn play_file(path: &Path) -> io::Result<()> {
    for player in ["pw-play", "paplay"] {
        match Command::new(player)
            .arg("--")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => return Err(io::Error::other(format!("{player} exited with {status}"))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Install pw-play or paplay to play desktop event sounds",
    ))
}

#[cfg(test)]
std::thread_local! { static RECORDED_SOUNDS: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) }; }

#[cfg(test)]
pub fn play(theme: String, event: String) {
    RECORDED_SOUNDS.with(|sounds| sounds.borrow_mut().push((theme, event)));
}

pub fn notification_event(notification: &coconut_api::notifications::Notification) -> Option<&str> {
    if notification.suppress_sound {
        return None;
    }
    if let Some(event) = notification.sound_name.as_deref().filter(|event| {
        !event.is_empty()
            && event.len() <= 255
            && event
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    }) {
        return Some(event);
    }
    Some(if notification.urgency >= 2 {
        "dialog-warning"
    } else {
        "message-new-instant"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_resolve_different_events_and_explicit_masks_disable_playback() {
        let root = std::env::temp_dir().join(format!("coconut-sounds-{}", std::process::id()));
        let paths = delib::xdg::XdgPaths {
            data_home: root.clone(),
            data_dirs: vec![],
            config_home: root.join("config"),
            config_dirs: vec![],
        };
        for theme in ["First", "Second"] {
            let directory = root.join("sounds").join(theme).join("stereo");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("message-new-instant.wav"), b"fixture").unwrap();
        }
        let request = SoundRequest::stereo("message-new-instant");
        let first = SoundResolver::with_paths("First", paths.clone())
            .resolve_file(&request)
            .unwrap();
        let second = SoundResolver::with_paths("Second", paths.clone())
            .resolve_file(&request)
            .unwrap();
        assert_ne!(first, second);
        std::fs::write(
            root.join("sounds/Second/stereo/message-new-instant.disabled"),
            b"",
        )
        .unwrap();
        assert_eq!(
            SoundResolver::with_paths("Second", paths).resolve(&request),
            Some(SoundResolution::Disabled)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
