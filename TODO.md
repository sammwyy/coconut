# TODO

- `shell.toml` is only read at startup, including when edited through
  `coconut settings`; changes there require restarting the shell process to
  take effect. The active CreamUI theme (`cream/active_theme.toml`) is the
  exception — the settings window itself updates live when you switch it.
- `[bar] position = "top"` only anchors correctly on the Wayland layer-shell
  backend (`creamui-platform`'s `WindowRole::TopPanel`/`BottomPanel`
  handling). The Windows backend does not place panel-role windows at all
  yet (pre-existing gap, not specific to the top/bottom option), so the bar
  position setting has no visible effect there.
- Network, Bluetooth, and battery panels react to native D-Bus signals
  (NetworkManager, BlueZ, UPower); kernel-backlight brightness reacts to an
  inotify watch on its sysfs node; volume reacts to a direct libpipewire
  connection (FFI, linked via `pipewire-devel`) subscribed to the default
  sink's param changes and the session's default-sink metadata. None of
  these poll.
- DDC-controlled external-monitor brightness (`ddcutil`) has no push
  notifications to hook into, so it still polls every 2 seconds — inherent
  to DDC/CI, not fixable without polling.
- The Windows integrations have no native hook for anything yet and fall
  back to polling throughout.
- CreamUI's system theme now carries a preferred `font_family` (loaded from
  the system's installed fonts), but the settings app has no UI to pick or
  clear one; it can currently only be set by hand-editing
  `~/.config/cream/appearance.toml` or the `CREAMUI_FONT` environment
  variable.
