# File Explorer

Coconut's file manager, built with [CreamUI](https://github.com/sammwyy/creamui).
The executable is `cfiles`; the desktop name is **File Explorer**.

- Follows the system theme, including live CreamUI theme updates.
- Browses the home directory, locations from `user-dirs.dirs`, mounted drives,
  local paths, and `file://` URIs.
- Tabs, address-bar navigation, search, pinned folders, recently opened files,
  and storage information in one window.
- Opens files through the MIME default applications already configured on the
  desktop, so the application you chose remains the application that opens the
  file.
- Ships a `.desktop` entry and an optional XDG Desktop Portal FileChooser
  backend for open, save, and folder-selection dialogs.

## Build

```sh
cargo run -p coconut-files
cargo run -p coconut-files -- ~/Documents
cargo run -p coconut-files -- file:///home/me/Downloads
cargo run -p coconut-files -- --settings
```

## Usage

| Command | What it does |
| --- | --- |
| `cfiles` | Opens File Explorer at Home. |
| `cfiles /path/to/folder` | Opens a particular folder. |
| `cfiles file:///path/to/folder` | Opens a local `file://` URI. |
| `cfiles --settings` | Opens File Explorer settings. `--setings` is accepted as a compatibility alias. |
| `cfiles --portal` | Starts the XDG Desktop Portal FileChooser backend. |
| `cfiles --help` | Shows the available command-line options. |

## Configuration

`delib` resolves `$XDG_CONFIG_HOME/coconut/cfiles.toml` (normally
`~/.config/coconut/cfiles.toml`).

The `explorer.tab_position` preference accepts `"top"` (the default) or
`"bottom"`; `explorer.show_statusbar` controls the item-count/status area.
Both are also available from `cfiles --settings` and apply to newly opened
windows. `[shortcuts]` overrides named key bindings.

## Portal backend

`--portal` runs the FileChooser implementation on the session bus as
`org.freedesktop.impl.portal.desktop.cfiles`. File Explorer is a portal
backend, not a replacement for `xdg-desktop-portal`: the desktop's portal
policy (`*-portals.conf`) chooses whether to route file dialogs to it, and
other installed backends remain as fallbacks.

The assets under `data/` describe the desktop entry, the D-Bus activation
service, the systemd user unit, and the portal registration. The development
installer (`../../install.sh` at the repository root) installs the binary and
writes those files under the user's XDG directories, adding
`FileChooser=cfiles` to `blair-portals.conf`/`coconut-portals.conf` without
overwriting existing policies. Log out and back in (or restart
`xdg-desktop-portal`) after installing so the desktop discovers the backend.

## Development

```sh
cargo test -p coconut-files
cargo clippy -p coconut-files --all-targets -- -D warnings
cargo fmt --check
```

`CFILES_SNAPSHOT_DIR` and `CFILES_SETTINGS_SNAPSHOT_DIR` rasterize rendered
scenes for visual comparisons and are only used by tests.
`CFILES_ICON_THEME` overrides the resolved desktop icon theme.
