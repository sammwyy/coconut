# Coconut

> [!WARNING]
> Coconut is a work in progress. It is still being shaped, polished, and
> tested for everyday desktop use.

Coconut is a small, modern desktop shell built around a clean bottom bar and
lightweight pop-up panels. It aims to make everyday desktop actions feel
immediate: launch an app, switch windows, check what is playing, or adjust your
system without leaving your flow.

## Applications

- `coconut` runs the shell.
- `coconut settings` opens the settings application.
- `coconut-settings` opens the settings application directly.

For local development, run `cargo run -p coconut-shell` or
`cargo run -p coconut-settings`.

When the Weather widget is enabled, Coconut resolves the configured city,
country and region directly on first use, stores the resulting coordinates in
the user profile, and fetches the forecast directly from MET Norway. Disabling
the widget prevents new weather requests.

## Highlights

- A centered dock with running applications and active-window feedback
- An application launcher with search, categories, icons, and scrolling
- A compact media area with playback controls and now-playing text
- Live clock and weather entry points
- Quick access to network, brightness, volume, Bluetooth, and battery status
- Purpose-built pop-up panels that stay visually consistent with the dock
- Native integrations for Blair, KDE/Wayland, and Windows 10 when available

## Platforms

Windows 10 is supported through the built-in Windows APIs and PowerShell:
window discovery and activation, Start menu applications, system media
sessions, master volume, battery, brightness, network, and Bluetooth. Hardware
features that are not present, such as a laptop battery or an internal display,
fall back gracefully.

Linux integrates natively with Blair through its org.blair.Compositor1 D-Bus
API (window lifecycle, focus/minimize, work areas, and layer-shell surfaces).
KDE Wayland remains supported with its relevant desktop services.

## Widgets

- App launcher button
- Running-app dock
- Media controls and scrolling track title
- Weather summary
- Live clock
- Network indicator
- Brightness indicator
- Volume indicator
- Bluetooth indicator
- Battery indicator

## Panels

- **Applications** — browse, search, and launch installed apps
- **Now Playing** — view current media and control playback
- **Control Center** — quick system controls and status, with links into
  the dedicated panels below
- **Network**, **Bluetooth**, **Energy**, **Brightness**, **Volume** —
  dedicated panels for each device, reachable from the control center or,
  per `shell.toml`, directly from their own tray icon
- **Clock** — a focused time panel
- **Weather** — at-a-glance conditions

## Status

Coconut is not ready for installation or daily use yet. There is no
installation guide at this stage; the project is evolving quickly and its
packaging story will arrive once the shell is ready for it.
