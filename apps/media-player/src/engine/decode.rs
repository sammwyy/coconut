//! Subprocess decoder running `ffmpeg` with two raw output streams: RGBA8 video
//! frames and interleaved `f32le` stereo PCM, paced at the input clock via `-readrate`.

use super::probe::{SubtitleSource, SubtitleStyle};
use crate::backends::{self, ProcessControl, SpawnedDecoder};
use std::io::Read;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};
use std::thread;
use std::time::Duration;

pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
const AUDIO_CHUNK: usize = 16 * 1024;

fn forward_audio(mut pipe: impl Read, tx: SyncSender<Vec<u8>>) {
    let mut buf = vec![0u8; AUDIO_CHUNK];
    let mut leftover = 0;
    loop {
        match pipe.read(&mut buf[leftover..]) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                let total = leftover + n;
                let aligned = total & !3;
                if aligned > 0 && tx.send(buf[..aligned].to_vec()).is_err() {
                    return;
                }
                leftover = total - aligned;
                buf.copy_within(aligned..total, 0);
            }
        }
    }
}

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub enum EngineEvent {
    /// Carries `(generation, exit_code, stderr_tail)`.
    Exited(u64, i32, String),
}

pub struct DecodeSession {
    control: ProcessControl,
    pub audio_rx: Option<Receiver<Vec<u8>>>,
    primed: bool,
    _waiter: thread::JoinHandle<()>,
}

#[allow(clippy::too_many_arguments)]
fn ffmpeg_args(
    path: &std::path::Path,
    pos: f64,
    speed: f32,
    video_stream: Option<usize>,
    audio_stream: Option<usize>,
    out_w: u32,
    out_h: u32,
    video_dest: Option<&str>,
    audio_dest: Option<&str>,
    subtitle: Option<&SubtitleSource>,
    style: SubtitleStyle,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
    ];
    if pos > 0.05 {
        args.extend(["-ss".into(), format!("{pos:.3}")]);
    }
    // Pace demuxing at `speed` times realtime; combined with `atempo` on the
    // audio stream, this scales playback rate while keeping A/V synchronized.
    args.extend([
        "-readrate".into(),
        format!("{speed:.4}"),
        "-readrate_initial_burst".into(),
        "0.25".into(),
    ]);
    args.extend(["-i".into(), path.to_string_lossy().into_owned()]);
    if let (Some(index), Some(video_dest)) = (video_stream, video_dest) {
        let mut filters = format!(
            "fps={:.4},scale={out_w}:{out_h},setsar=1",
            24.0 / speed.max(1.0)
        );
        if let Some(subtitle) = subtitle {
            let (file, ordinal) = match subtitle {
                SubtitleSource::Embedded { ordinal, .. } => (path, Some(*ordinal)),
                SubtitleSource::External(file) => (file.as_path(), None),
            };
            let escape = |text: &str, chars: &str| {
                text.chars().fold(String::new(), |mut result, ch| {
                    if chars.contains(ch) {
                        result.push('\\');
                    }
                    result.push(ch);
                    result
                })
            };
            let filename = escape(&escape(&file.to_string_lossy(), "\\':"), "\\'[],;");
            if pos > 0.05 {
                filters.push_str(&format!(",setpts=PTS+{pos:.3}/TB"));
            }
            filters.push_str(&format!(",subtitles=filename={filename}"));
            if let Some(ordinal) = ordinal {
                filters.push_str(&format!(":si={ordinal}"));
            }
            if style.customized {
                let horizontal = if (style.x - 0.5).abs() < 0.03 {
                    2
                } else if style.x < 0.5 {
                    1
                } else {
                    3
                };
                let alignment = horizontal + if style.y < 0.5 { 6 } else { 0 };
                let margin_y = ((if style.y < 0.5 {
                    style.y
                } else {
                    1.0 - style.y
                }) * out_h as f32)
                    .round() as u32;
                let margin_x = (if horizontal == 1 {
                    style.x
                } else if horizontal == 3 {
                    1.0 - style.x
                } else {
                    0.0
                } * out_w as f32)
                    .round() as u32;
                let margin = if horizontal == 3 {
                    "MarginR"
                } else {
                    "MarginL"
                };
                filters.push_str(&format!(
                    ":force_style=Alignment={alignment}\\,{margin}={margin_x}\\,MarginV={margin_y}\\,Outline={:.1}",
                    style.outline
                ));
            }
            if pos > 0.05 {
                filters.push_str(&format!(",setpts=PTS-{pos:.3}/TB"));
            }
        }
        args.extend([
            "-map".into(),
            format!("0:{index}"),
            "-an".into(),
            "-sn".into(),
            "-dn".into(),
            "-vf".into(),
            filters,
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            video_dest.into(),
        ]);
    }
    if let (Some(index), Some(audio_dest)) = (audio_stream, audio_dest) {
        args.extend([
            "-map".into(),
            format!("0:{index}"),
            "-vn".into(),
            "-sn".into(),
            "-dn".into(),
        ]);
        if (speed - 1.0).abs() > 0.001 {
            args.extend(["-af".into(), format!("atempo={speed:.4}")]);
        }
        args.extend([
            "-f".into(),
            "f32le".into(),
            "-ac".into(),
            AUDIO_CHANNELS.to_string(),
            "-ar".into(),
            AUDIO_RATE.to_string(),
            audio_dest.into(),
        ]);
    }
    args
}

impl DecodeSession {
    /// `out_w` and `out_h` must match the scaled dimensions from
    /// [`crate::engine::probe::MediaInfo::output_size`] so raw frame reads align
    /// with frame boundaries.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        path: &std::path::Path,
        pos: f64,
        speed: f32,
        video_stream: Option<usize>,
        audio_stream: Option<usize>,
        out_w: u32,
        out_h: u32,
        subtitle: Option<&SubtitleSource>,
        style: SubtitleStyle,
        events_tx: Sender<EngineEvent>,
        video_tx: SyncSender<(u64, VideoFrame)>,
        generation: u64,
    ) -> std::io::Result<Self> {
        let SpawnedDecoder {
            mut child,
            control,
            video_reader,
            audio_reader,
        } = backends::spawn_ffmpeg(
            video_stream.is_some(),
            audio_stream.is_some(),
            |video_dest, audio_dest| {
                ffmpeg_args(
                    path,
                    pos,
                    speed,
                    video_stream,
                    audio_stream,
                    out_w,
                    out_h,
                    video_stream.map(|_| video_dest),
                    audio_stream.map(|_| audio_dest),
                    subtitle,
                    style,
                )
            },
        )?;

        let stderr = child.stderr.take();
        let waiter = thread::spawn(move || {
            let status = child.wait();
            let mut tail = String::new();
            if let Some(mut err) = stderr {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                tail = buf.lines().rev().take(4).collect::<Vec<_>>().join("\n");
            }
            let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let _ = events_tx.send(EngineEvent::Exited(generation, code, tail));
        });

        let (ready_tx, ready_rx) = sync_channel(1);
        if let Some(mut pipe) = video_reader {
            let frame_len = (out_w as usize * out_h as usize * 4).max(1);
            thread::spawn(move || {
                let mut pixels = vec![0u8; frame_len];
                let mut first = true;
                loop {
                    match pipe.read_exact(&mut pixels) {
                        Ok(()) => {
                            if first {
                                let _ = ready_tx.send(());
                                first = false;
                            }
                            let frame = VideoFrame {
                                width: out_w,
                                height: out_h,
                                pixels,
                            };
                            // Drop frames if the UI receiver is full so video
                            // decoding does not block on rendering.
                            match video_tx.try_send((generation, frame)) {
                                Ok(()) => pixels = vec![0u8; frame_len],
                                Err(std::sync::mpsc::TrySendError::Full((_, frame))) => {
                                    pixels = frame.pixels;
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return,
                            }
                        }
                        Err(_) => return,
                    }
                }
            });
        }

        let audio_rx = if let Some(pipe) = audio_reader {
            let (tx, rx) = sync_channel::<Vec<u8>>(64);
            thread::spawn(move || forward_audio(pipe, tx));
            Some(rx)
        } else {
            None
        };
        let primed = video_stream.is_some();
        if primed {
            if ready_rx.recv_timeout(Duration::from_secs(20)).is_err() {
                control.kill();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ffmpeg produced no video frame",
                ));
            }
            control.suspend();
        }

        Ok(DecodeSession {
            control,
            audio_rx,
            primed,
            _waiter: waiter,
        })
    }

    pub fn activate(&self, paused: bool) {
        if paused && !self.primed {
            self.suspend();
        } else if !paused && self.primed {
            self.resume();
        }
    }

    pub fn suspend(&self) {
        self.control.suspend();
    }

    pub fn resume(&self) {
        self.control.resume();
    }
}

impl Drop for DecodeSession {
    fn drop(&mut self) {
        self.control.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ShortReads<'a>(&'a [u8]);
    impl Read for ShortReads<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.0.len()).min(3);
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn pcm_chunks_preserve_samples_across_short_reads() {
        let samples = [1.0f32, -0.25, 0.5];
        let bytes: Vec<_> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let (tx, rx) = sync_channel(4);
        forward_audio(ShortReads(&bytes), tx);
        let chunks: Vec<_> = rx.into_iter().collect();
        assert!(chunks.iter().all(|chunk| chunk.len() % 4 == 0));
        assert_eq!(chunks.concat(), bytes);
    }

    #[test]
    fn maps_actual_stream_indices_not_first_video_or_audio() {
        let args = ffmpeg_args(
            std::path::Path::new("video.mkv"),
            0.0,
            1.0,
            Some(3),
            Some(2),
            1280,
            720,
            Some("pipe:3"),
            Some("pipe:4"),
            None,
            SubtitleStyle::default(),
        );
        let maps: Vec<_> = args
            .windows(2)
            .filter(|pair| pair[0] == "-map")
            .map(|pair| pair[1].as_str())
            .collect();
        assert_eq!(maps, ["0:3", "0:2"]);
        let fast = ffmpeg_args(
            std::path::Path::new("video.mkv"),
            10.0,
            2.0,
            Some(3),
            Some(2),
            1280,
            720,
            Some("pipe:3"),
            Some("pipe:4"),
            None,
            SubtitleStyle::default(),
        );
        assert!(fast
            .windows(2)
            .any(|pair| pair[0] == "-vf" && pair[1].starts_with("fps=12.0000,scale=")));
    }

    #[test]
    fn subtitles_filter_selects_embedded_stream_or_external_ass() {
        let path = std::path::Path::new("movie.mkv");
        let filter = |subtitle: &SubtitleSource| {
            let args = ffmpeg_args(
                path,
                0.0,
                1.0,
                Some(0),
                None,
                320,
                180,
                Some("pipe:3"),
                None,
                Some(subtitle),
                SubtitleStyle::default(),
            );
            args.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1].clone()
        };
        assert!(filter(&SubtitleSource::Embedded {
            index: 4,
            ordinal: 1
        })
        .ends_with("subtitles=filename=movie.mkv:si=1"));
        assert!(filter(&SubtitleSource::External("movie.ass".into()))
            .ends_with("subtitles=filename=movie.ass"));
        let style = SubtitleStyle {
            x: 0.25,
            y: 0.15,
            outline: 4.0,
            customized: true,
        };
        let args = ffmpeg_args(
            path,
            0.0,
            1.0,
            Some(0),
            None,
            320,
            180,
            Some("pipe:3"),
            None,
            Some(&SubtitleSource::External("movie.ass".into())),
            style,
        );
        let filter = &args.windows(2).find(|pair| pair[0] == "-vf").unwrap()[1];
        assert!(filter.contains("Alignment=7\\,MarginL=80\\,MarginV=27\\,Outline=4.0"));
    }
}
