use crate::platform::Playback;

pub fn summary(playback: Option<&Playback>) -> String {
    playback
        .map(|item| format!("{} · {}", item.artist, item.title))
        .unwrap_or_else(|| "Nothing playing".into())
}
