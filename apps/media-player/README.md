# Media Player

Coconut's video and music player, built with [CreamUI](https://github.com/sammwyy/creamui).
The executable is `cplayer`; the desktop name is **Media Player**.

- Video and audio playback through libmpv or FFmpeg: mp4, mkv, webm, avi,
  mov, mp3, flac, ogg, opus, m4a, wav and anything else ffmpeg understands.
- Floating transport card with seek, play/pause, queue, shuffle, repeat,
  speed, volume, and optional blur over video or artwork.
- Subtitle selector (embedded tracks and same-name external files) with
  placement/outline controls, and an audio-track selector.
- Music view with embedded cover art over a blurred backdrop; the accent
  color follows the artwork.
- Desktop media controls: MPRIS 2 on Linux and System Media Transport
  Controls on Windows — shell media widgets, media keys, and `playerctl`
  drive the player.
- Optional Discord Rich Presence (off by default), optional background
  playback with a tray icon, single-instance IPC (`--player` opens files in
  the running window, `--quit` stops it).
- File open dialogs go through the XDG Desktop Portal (`rfd` with the
  `xdg-portal` backend) — served by File Explorer's `cfiles` portal backend
  when installed.
- Live system-theme reload and configurable keyboard shortcuts.

## Build

```sh
cargo run -p coconut-media-player
cargo run -p coconut-media-player -- ~/Music
cargo run -p coconut-media-player -- --settings
```

## Usage

| Command | What it does |
| --- | --- |
| `cplayer [FILE\|DIR ...]` | Opens the player, queueing any files or folders given. |
| `cplayer --player ...` | Forwards the files to the running player window. |
| `cplayer --settings` | Opens Media Player settings. |
| `cplayer --quit` | Stops playback and closes the running player. |

Keyboard: `space` play/pause · `←` `→` seek 5 s (`shift` = 30 s) · `↑` `↓`
volume · `f` maximize · `m` mute · `[` `]` speed · `n`/`p` next/previous ·
`l` repeat · `s` shuffle · `t` queue · `o` open · `,` settings · `q`/`Esc`
close the window.

## Configuration

`delib` resolves `$XDG_CONFIG_HOME/coconut/cplayer.toml` (normally
`~/.config/coconut/cplayer.toml`). Settings covers playback backend
(libmpv/FFmpeg/auto), startup volume/speed/repeat/shuffle, autoplay, seek
steps, controls auto-hide, animations, transport bar style/opacity/blur,
background playback + tray, Discord presence, accent color, and music
background/cover shape — applied live to open players.

Discord presence needs a Discord application id in
`CPLAYER_DISCORD_CLIENT_ID`; the `cplayer` asset key covers the static
large image when a track has no uploadable cover.

`CPLAYER_BACKEND` forces a playback engine (`mpv`, `ffmpeg`, `audio`).
`CPLAYER_MPV_TEST_FILE` and `CPLAYER_FFMPEG_TEST_FILE` point the decoder
tests at real fixtures.

## Development

```sh
cargo test -p coconut-media-player
cargo clippy -p coconut-media-player --all-targets -- -D warnings
cargo fmt --check
```
