# Pinned apps

`coconut-plugin-pinned-apps` contributes the `pinned_apps` island and the
`pin_application` panel. The island works in the Statusbar, Dockbar and side
bars. New configurations include it between the launcher and open windows in
the Dockbar. Existing layouts can add it through Contents or Quick Edit.

- Click a pinned icon to launch its application.
- Right-click the widget and choose **Manage pinned applications…** to open a searchable,
  vertical list grouped by the same categories as the app drawer. Choosing a
  row toggles its pin; the list stays open and offers Pin or Unpin on each row.
- Right-click a pinned icon for **Unpin** or **Manage pinned applications…**.
- Drop `.desktop` files, including drags from the app drawer, on the widget.
  Only desktop launchers are accepted. Native Wayland transfers advertise Copy;
  source launchers are never moved or removed.
- More than six pinned icons use a flat overflow menu. Manage pinned applications
  can remove any pin, including overflow entries.
- An empty widget reserves an unmarked space between separators. Right-click it
  to manage pins. Failed writes show an error indicator;
  the picker and app drawer display the error and keep the saved pins intact.

Pinned and running icons use the same size, alignment, background and hover
presentation. Pinned icons inherit the Open windows appearance settings, including
inline overrides from the first Open windows placement in the same bar, even when
it is in another section. With no such placement, they use `modules/open_windows.toml`.
Quick Edit uses the same resolution. Running windows remain separate entries;
pinning a launcher never removes its windows from Open windows.

## Persistence

The shared file is `$XDG_CONFIG_HOME/coconut/pinned-apps.toml`
(default `~/.config/coconut/pinned-apps.toml`):

```toml
[[applications]]
source = "/usr/share/applications/example.desktop"
desktop_file = "/home/user/.local/share/coconut/pinned-apps/0123456789abcdef.desktop"
```

`source` identifies the original launcher; `desktop_file` points to a private
copy under `$XDG_DATA_HOME/coconut/pinned-apps` (default
`~/.local/share/coconut/pinned-apps`). All widget placements share this list.
Unpinning removes its entry and leaves the source untouched. Missing or invalid
copies remain removable through the icon's context menu.

`coconut-core::PinnedAppsStore` owns the format, validates imported desktop
entries, serializes writers with a separate file lock, and replaces the config
atomically. It reloads the saved list under that lock before editing. Invalid
configuration and failed imports do not replace the saved list.

The shared `coconut-plugin-kit::pins::Pins` client performs filesystem and icon
work in a serialized background queue and updates local consumers after successful
writes. The drawer and the plugin use this common client; neither invokes the
other plugin. The shell registers plugins and hosts generic interactions. It has
no pin/unpin commands or knowledge of the pin store. The file is authoritative;
the generic module-change event is an optional invalidation hint. Direct file
edits are read on startup or after that event.

## Validation

```bash
cargo test -p coconut-core -p coconut-plugin-kit -p coconut-plugin-pinned-apps -p coconut-shell
COCONUT_PINNED_SNAPSHOT=/tmp/pinned-apps cargo test -p coconut-plugin-pinned-apps
```

Scene tests cover normal and compact pickers in light/dark themes, grouping,
search, empty results, bounded row construction, icon centering in both axes,
keyboard focus and dismissal. Native drag transfer and widget drop dispatch are
covered in the sibling CreamUI repository. File drops use the native Wayland
backend; unsupported native drag backends report an error without moving files.
