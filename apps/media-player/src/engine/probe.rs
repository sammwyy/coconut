use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubtitleSource {
    Embedded { index: usize, ordinal: usize },
    External(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubtitleStyle {
    pub x: f32,
    pub y: f32,
    pub outline: f32,
    pub customized: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.88,
            outline: 2.0,
            customized: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub source: SubtitleSource,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct AudioTrack {
    pub index: usize,
    pub label: String,
}

fn external_subtitles(path: &Path) -> Vec<SubtitleTrack> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut tracks: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file = entry.path();
            let ext = file.extension()?.to_str()?;
            (file.file_stem() == path.file_stem()
                && ["ass", "ssa", "srt", "vtt"]
                    .iter()
                    .any(|supported| ext.eq_ignore_ascii_case(supported))
                && file.is_file())
            .then(|| SubtitleTrack {
                label: format!("{} (file)", ext.to_uppercase()),
                source: SubtitleSource::External(file.clone()),
            })
        })
        .collect();
    tracks.sort_by(|a, b| a.label.cmp(&b.label));
    tracks
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Duration in seconds, or `0.0` if unknown.
    pub duration: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_stream: Option<usize>,
    pub audio_stream: Option<usize>,
    pub audio_tracks: Vec<AudioTrack>,
    pub subtitles: Vec<SubtitleTrack>,
    /// True if the file contains an attached picture stream (embedded cover art).
    pub attached_picture: bool,
    /// Display dimensions after applying stream rotation metadata.
    pub width: u32,
    pub height: u32,
}

impl MediaInfo {
    /// Scaled output dimensions used by the video filter.
    pub fn output_size(&self) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (0, 0);
        }
        let scale = (1280.0 / self.width.max(self.height) as f64).min(1.0);
        let even = |dimension: u32| ((dimension as f64 * scale / 2.0).round() as u32 * 2).max(2);
        (even(self.width), even(self.height))
    }
}

#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Stream {
    index: usize,
    codec_type: Option<String>,
    codec_name: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
    #[serde(default)]
    disposition: Disposition,
    #[serde(default)]
    side_data_list: Vec<serde_json::Value>,
}

#[derive(Default, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u32,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

fn tag<'a>(tags: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn rotation(stream: &Stream) -> i32 {
    for data in &stream.side_data_list {
        // ffprobe may encode display matrix rotation as either an integer or a string.
        let value = data
            .get("rotation")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
        if let Some(degrees) = value {
            return degrees as i32;
        }
    }
    0
}

fn embedded_subtitles(streams: &[Stream]) -> Vec<SubtitleTrack> {
    let mut subtitles = Vec::new();
    for stream in streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("subtitle"))
    {
        let language = tag(&stream.tags, "language").unwrap_or("Unknown");
        let title = tag(&stream.tags, "title").unwrap_or("");
        subtitles.push(SubtitleTrack {
            source: SubtitleSource::Embedded {
                index: stream.index,
                ordinal: subtitles.len(),
            },
            label: format!(
                "{} · {} {} (track {})",
                language,
                stream
                    .codec_name
                    .as_deref()
                    .unwrap_or("subtitle")
                    .to_uppercase(),
                title,
                stream.index + 1
            ),
        });
    }
    subtitles
}

fn audio_tracks(streams: &[Stream]) -> Vec<AudioTrack> {
    streams
        .iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
        .map(|stream| AudioTrack {
            index: stream.index,
            label: format!(
                "{} · {} {} (track {})",
                tag(&stream.tags, "language").unwrap_or("Unknown"),
                stream
                    .codec_name
                    .as_deref()
                    .unwrap_or("audio")
                    .to_uppercase(),
                tag(&stream.tags, "title").unwrap_or(""),
                stream.index + 1
            ),
        })
        .collect()
}

pub fn probe(path: &Path) -> Result<MediaInfo, String> {
    let output = crate::backends::background_command("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("could not run ffprobe: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed: {}",
            stderr.lines().next().unwrap_or("unknown error")
        ));
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("could not parse ffprobe output: {e}"))?;

    let video = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0);
    let picture = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic != 0);
    let audio = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));
    let audio_tracks = audio_tracks(&probe.streams);
    let mut subtitles = embedded_subtitles(&probe.streams);
    if video.is_some() {
        subtitles.extend(external_subtitles(path));
    }

    if video.is_none() && audio.is_none() {
        return Err("no playable audio or video stream".to_string());
    }

    let format_tags = probe.format.as_ref().map(|f| &f.tags);
    let file_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace(['_', '-'], " ");
    let title = format_tags
        .and_then(|t| tag(t, "title"))
        .map(str::to_string)
        .unwrap_or(file_title);

    let mut width = video.and_then(|v| v.width).unwrap_or(0);
    let mut height = video.and_then(|v| v.height).unwrap_or(0);
    if let Some(v) = video {
        match rotation(v).rem_euclid(360) {
            90 | 270 => std::mem::swap(&mut width, &mut height),
            _ => {}
        }
    }

    let duration = probe
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| {
            video
                .and_then(|s| s.duration.as_deref())
                .or_else(|| audio.and_then(|s| s.duration.as_deref()))
        })
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(MediaInfo {
        path: path.to_path_buf(),
        title,
        artist: format_tags
            .and_then(|t| tag(t, "artist").or_else(|| tag(t, "album_artist")))
            .map(str::to_string),
        album: format_tags
            .and_then(|t| tag(t, "album"))
            .map(str::to_string),
        duration,
        has_video: video.is_some(),
        has_audio: audio.is_some(),
        video_stream: video.map(|s| s.index),
        audio_stream: audio.map(|s| s.index),
        audio_tracks,
        subtitles,
        attached_picture: picture.is_some(),
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_each_audio_stream_for_track_selection() {
        let probe: Probe = serde_json::from_str(
            r#"{"streams":[
                {"index":0,"codec_type":"video"},
                {"index":1,"codec_type":"audio","codec_name":"aac","tags":{"language":"spa"}},
                {"index":2,"codec_type":"subtitle"},
                {"index":4,"codec_type":"audio","codec_name":"ac3","tags":{"language":"eng","title":"Surround"}}
            ]}"#
        ).unwrap();
        let tracks = audio_tracks(&probe.streams);
        assert_eq!(
            tracks.iter().map(|track| track.index).collect::<Vec<_>>(),
            [1, 4]
        );
        assert!(tracks[1].label.contains("Surround"));
    }

    #[test]
    fn embedded_tracks_preserve_stream_index_and_subtitle_ordinal() {
        let probe: Probe = serde_json::from_str(r#"{"streams":[
            {"index":0,"codec_type":"video"},
            {"index":2,"codec_type":"subtitle","codec_name":"ass","tags":{"language":"spa"}},
            {"index":3,"codec_type":"audio"},
            {"index":5,"codec_type":"subtitle","codec_name":"subrip","tags":{"language":"eng","title":"SDH"}}
        ]}"#).unwrap();
        let subtitles = embedded_subtitles(&probe.streams);
        assert_eq!(subtitles.len(), 2);
        assert_eq!(
            subtitles[0].source,
            SubtitleSource::Embedded {
                index: 2,
                ordinal: 0
            }
        );
        assert_eq!(
            subtitles[1].source,
            SubtitleSource::Embedded {
                index: 5,
                ordinal: 1
            }
        );
        assert!(subtitles[1].label.contains("SDH"));
    }

    #[test]
    fn finds_same_name_external_subtitles() {
        let dir = std::env::temp_dir().join(format!(
            "cplayer-subtitles-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let video = dir.join("episode.mkv");
        let ass = dir.join("episode.ASS");
        let other = dir.join("unrelated.srt");
        std::fs::write(&ass, "").unwrap();
        std::fs::write(&other, "").unwrap();
        let tracks = external_subtitles(&video);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].source, SubtitleSource::External(ass.clone()));
        std::fs::remove_file(ass).unwrap();
        std::fs::remove_file(other).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn output_is_bounded_for_large_portrait_and_landscape_videos() {
        let mut info = MediaInfo {
            path: PathBuf::new(),
            title: String::new(),
            artist: None,
            album: None,
            duration: 0.0,
            has_video: true,
            has_audio: false,
            video_stream: Some(0),
            audio_stream: None,
            audio_tracks: vec![],
            subtitles: vec![],
            attached_picture: false,
            width: 3840,
            height: 2160,
        };
        assert_eq!(info.output_size(), (1280, 720));
        std::mem::swap(&mut info.width, &mut info.height);
        assert_eq!(info.output_size(), (720, 1280));
    }
}
