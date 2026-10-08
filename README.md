# Coconut

> A focused, modern desktop shell for Wayland.

> [!WARNING]
> Coconut is under active development and is not yet intended for daily use or
> distribution packaging.

Coconut puts common desktop actions within immediate reach: launch an
application, move between running windows, control media, or adjust your system
without losing focus. Its top statusbar and centered dockbar are designed to
stay calm, responsive, and consistent.

## What it includes

- A configurable statusbar and dockbar, with widgets usable in either or both
- Island groups for visually separated widget sections
- A centered running-app dockbar with active-window feedback
- Application browsing, category navigation, and search
- Now-playing information with playback controls
- Clock and weather widgets
- A control center for network, brightness, volume, Bluetooth, battery, and
  power profiles
- Floating volume and brightness indicators with configurable placement
- D-Bus desktop notifications with icons, actions, replacement and expiration
- Dedicated panels for the same system controls when you need more detail
- Native desktop integrations for Blair and KDE Wayland, plus Windows 10

## Applications

| Command | Purpose |
| --- | --- |
| `coconut` | Start the shell. |
| `coconut settings` | Open Coconut Settings. |
| `coconut-settings` | Open Coconut Settings directly. |

## Notifications and status updates

Open **Settings → Desktop → Notifications & popups** to set the screen edge,
alignment, edge offset and duration separately for application notifications
and volume/brightness indicators. Coconut also shows its own notifications
when Wi-Fi or Bluetooth connects, disconnects, or is turned on/off. These use
the notification preferences directly and work without the D-Bus notification
service. Changes apply immediately. The same options
are stored in `[notifications]` and `[status_updates]` in `shell.toml`:
`enabled`, `edge` (`top`/`bottom`), `align` (`left`/`center`/`right`), `offset`
(in logical pixels) and `duration_ms`.

On Linux, Coconut provides `org.freedesktop.Notifications` on the session bus
when no other notification service owns that name. Test it with
`notify-send -i dialog-information "Coconut" "Notifications are ready"`.
Application timeouts take precedence over the configured default; critical
notifications stay visible until dismissed unless the application sets a timeout.

## Platform notes

On Linux, Coconut integrates with Blair through its
`org.blair.Compositor1` D-Bus API for window management and layer-shell
surfaces. KDE Wayland services are supported where available.

On Windows 10, Coconut uses native APIs and PowerShell for window discovery,
application launching, media sessions, volume, power, display brightness,
network, and Bluetooth. Unavailable hardware features fall back gracefully.

When Weather is enabled, Coconut resolves the configured location on first use,
stores its coordinates in the user profile, and retrieves forecasts directly
from MET Norway. Disabling the widget prevents new weather requests.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for the required checkout layout, Linux
dependencies for Arch, Debian/Ubuntu, Fedora/RHEL, and openSUSE, build commands,
and troubleshooting guidance.

For a local development session:

```bash
cargo run -p coconut-shell
```

Launch the settings application separately with:

```bash
cargo run -p coconut-settings
```
