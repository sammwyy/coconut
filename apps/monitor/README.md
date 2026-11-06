# System Monitor

Coconut's system monitor, built with [CreamUI](https://github.com/sammwyy/creamui).
The executable is `cmonitor`; the desktop name is **System Monitor**.

- **Five top tabs** — Overview, Apps, Performance, Networking, and Services.
  Apps switches between grouped applications and all processes without leaving
  the view.
- **Native data on Linux and Windows** — Linux reads `/proc`, `/sys`, mount
  information, and `statvfs`; Windows uses `sysinfo`, Win32 process/power APIs,
  disk performance counters, `netstat`, and Windows services. It does not need
  a background agent.
- **Desktop applications** — Linux matches the current user's running processes
  to desktop launchers (via `delib`'s desktop-entry catalog, including Flatpak
  exports). Windows groups executable families by their parent processes. Both
  aggregate application CPU and memory.
- **Compact overview and performance detail** — CPU, memory, disk, GPU (when
  available), and network cards; selectable devices with history graphs.
- **Focused networking** — Connections and Ports, independently filtered per
  interface.
- **Explicit controls** — inspect, pause/resume, end, or force-kill your
  processes; end grouped applications; start, stop, or restart services.
  Linux supports system and user `systemd` scopes; Windows supports system
  services. Destructive actions require confirmation and process identities
  are checked against PID reuse. On Windows, ending and force-killing a task
  both terminate it immediately.
- **Respects the desktop** — system theme, live CreamUI theme updates, and
  hybrid compositor decorations with fallbacks. Navigation stays centered
  and uses compact labels in narrow windows, with actions opposite the window
  controls.
- **CPU rendering** — windows rasterize through CreamUI's CPU backend;
  `CUI_OVERRIDE_RENDER_BACKEND=gpu` opts into `wgpu` per launch.

## Build

```sh
cargo run -p coconut-monitor
```

## Install on Linux

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

Telemetry snapshots are shared between views and the sampling worker rather
than copied on each read. Each device history retains at most 30 samples;
disconnected devices are removed on the next sample. Reading a chart copies
only that chart's samples. Process tables mount only the visible rows, though
filtering and sorting still visit the full process list.

Platform implementations live in `src/system/{linux,windows}.rs`,
`src/telemetry/{linux,windows}.rs`, and `src/actions/{linux,windows}.rs`.
The shared snapshot, sampling, actions, and views select those modules at
compile time. Windows disk/service metadata and GPU identification use the
included Windows PowerShell; GPU utilization and temperature remain available
only when the active platform and driver expose those counters.
