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
- Installs the File Explorer icon from `assets/icons/apps/app-files.svg` as
  `cfiles` in the desktop icon theme.
- Renders through CreamUI's CPU backend; `CUI_OVERRIDE_RENDER_BACKEND=gpu`
  opts into `wgpu` per launch.
- Shows image, video, audio cover and embedded Windows executable icon previews
  in directory views, falling back to themed file icons when unavailable.
  A lower-left badge shows the associated default application's icon when
  installed; a lower-right link badge identifies symbolic links (including
  linked folders). MIME badges are resolved off the UI thread. Previews are
  generated off the UI thread and shared with the desktop through
  [Coconut's private thumbnail cache](../../crates/thumbnails/README.md).

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
installer (`../../scripts/install.sh`) installs the binary and
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

## Directory performance

Folder navigation and refresh load metadata on a background worker. Each window
runs one directory load at a time, coalesces newer navigation requests, and ignores
results for obsolete locations or closed tabs. Columns mode uses these same
snapshots rather than reading directories during rendering.

List, Grid and Columns mount only visible rows with a small overscan through
CreamUI's virtual list. Filtering and sorting reuse a cached snapshot until the
entries, query, sort order or chooser filter changes. Icon fallback results are
cached as well. Directory metadata, filtered snapshots and row height indexes
still use memory proportional to the number of entries; mounted widgets and
painting are bounded by the viewport.

`large_directories_mount_only_visible_rows_and_keep_cached_results` exercises
20,000 entries, Unicode names, both themes, normal and compact windows, and
scrolling to the last entry in all three views.

## Context menus

Right-click a file or folder in Grid, List or Columns mode to act on that item
without opening it. Right-click empty directory space for New, Paste, Sort by,
hidden files, Refresh and File Explorer settings. In Columns mode the background
menu acts on the clicked column's directory. Home's pinned folders, drives and
recent files use the shared item menu. Its background offers Open home folder,
terminal, explorer, properties and settings actions; creation stays in directory
views. The chooser disables creation, rename and paste; a Save chooser uses
Use file name for file items.

Menus and file operation dialogs come from `coconut-file-menu`; generic menu
presentation and popup hosting come from CreamUI. The native Wayland backend opens
menus and submenus in compositor-managed popup surfaces at the pointer, allowing
them outside the File Explorer window. Other backends and failed popup requests
use the same component inside the window. The shell desktop uses the same common file
and directory actions. Arrow keys navigate submenus, Enter activates, Left closes
a submenu, and Escape or an outside click dismisses the menu. File operations run
in the background and surface failures in a dialog. See the
[shared crate](../../crates/file-menu/README.md) for integration and clipboard
support.
