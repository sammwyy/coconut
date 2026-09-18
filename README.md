# Coconut

> A focused, modern desktop shell for Wayland.

> [!WARNING]
> Coconut is under active development and is not yet intended for daily use or
> distribution packaging.

Coconut puts common desktop actions within immediate reach: launch an
application, move between running windows, control media, or adjust your system
without losing focus. Its centered dock and lightweight panels are designed to
stay calm, responsive, and consistent.

## What it includes

- A centered running-app dock with active-window feedback
- Application browsing, category navigation, and search
- Now-playing information with playback controls
- Clock and weather widgets
- A control center for network, brightness, volume, Bluetooth, battery, and
  power profiles
- Dedicated panels for the same system controls when you need more detail
- Native desktop integrations for Blair and KDE Wayland, plus Windows 10

## Applications

| Command | Purpose |
| --- | --- |
| `coconut` | Start the shell. |
| `coconut settings` | Open Coconut Settings. |
| `coconut-settings` | Open Coconut Settings directly. |

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
