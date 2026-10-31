# System Monitor

Coconut's system monitor, built with [CreamUI](https://github.com/sammwyy/creamui).
The executable is `cmonitor`; the desktop name is **System Monitor**.

- **Five top tabs** — Overview, Apps, Performance, Networking, and Services.
  Apps switches between grouped applications and all processes without leaving
  the view.
- **Linux-native data** — reads `/proc`, `/sys`, mount information, and
  `statvfs`; it does not need a background agent.
- **Desktop applications** — matches the current user's running processes to
  desktop launchers (via `delib`'s desktop-entry catalog, including Flatpak
  exports) and groups application helpers with aggregate CPU/memory.
- **Compact overview and performance detail** — CPU, memory, disk, GPU (when
  available), and network cards; selectable devices with history graphs.
- **Focused networking** — Connections and Ports, independently filtered per
  interface.
- **Explicit controls** — inspect, pause/resume, end, or force-kill your
  processes; end grouped applications; start, stop, or restart system/user
  services. Destructive actions require confirmation and process identities
  are checked against PID reuse.
- **Respects the desktop** — system theme, live CreamUI theme updates, and
  hybrid compositor decorations with fallbacks.
- **CPU rendering** — windows rasterize through CreamUI's CPU backend;
  `CUI_OVERRIDE_RENDER_BACKEND=gpu` opts into `wgpu` per launch.

## Build

```sh
cargo run -p coconut-monitor
```

## Install

```sh
install -Dm755 target/release/cmonitor ~/.local/bin/cmonitor
install -Dm644 data/cmonitor.desktop ~/.local/share/applications/cmonitor.desktop
```

The development installer (`../../scripts/install.sh`)
installs both.

## Development

```sh
cargo test -p coconut-monitor
cargo clippy -p coconut-monitor --all-targets -- -D warnings
cargo fmt --check
```

`CMONITOR_PREVIEW_DIR=target/previews cargo test -p coconut-monitor -- --test-threads=1`
rasterizes preview PNGs without a display server.
