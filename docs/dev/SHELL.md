# Developing the Coconut shell

Coconut is a Rust workspace for a Wayland desktop shell. This guide covers the
local toolchain, native libraries, shell/session development, and everyday
development commands. For application architecture and UI conventions, read
[Developing Coconut apps](APPS.md). Commands below run from the Coconut checkout.

## Requirements

Use a current stable Rust toolchain and install the native development packages
for your distribution. Coconut links to PipeWire and uses `bindgen` while
building; therefore, the PipeWire headers, `pkg-config`, Clang, and libclang
must be present.

The workspace uses local path dependencies for CreamUI, Blair, Alphin, and delib.
Keep these repositories side by side, following the paths in the root
[Cargo.toml](../../Cargo.toml):

```text
workspace/
├── alphin/
├── blair/
├── creamui/
├── delib/
└── coconut/
```

If you use a different layout, update the path dependencies in the relevant
`Cargo.toml` files before building.

## Install system dependencies

### Arch Linux

```bash
sudo pacman -Syu --needed base-devel rustup pkgconf pipewire clang libclang \
  wayland libxkbcommon dbus
rustup default stable
```

### Debian and Ubuntu

```bash
sudo apt update
sudo apt install build-essential rustup pkg-config libpipewire-0.3-dev \
  clang libclang-dev libwayland-dev libxkbcommon-dev libdbus-1-dev
rustup default stable
```

On systems where `rustup` is unavailable from the configured APT repositories,
install the stable toolchain with the [official Rust installer](https://rustup.rs/).

### Fedora and RHEL-compatible distributions

```bash
sudo dnf install @development-tools rust cargo pkgconf-pkg-config pipewire-devel \
  clang clang-devel wayland-devel libxkbcommon-devel dbus-devel
```

On RHEL-compatible systems, enable the appropriate development repositories if
one or more `*-devel` packages are unavailable. If the packaged Rust version is
too old, install the current stable toolchain with [rustup](https://rustup.rs/).

### openSUSE

```bash
sudo zypper install -t pattern devel_basis
sudo zypper install rustup pkg-config pipewire-devel clang libclang-devel \
  wayland-devel libxkbcommon-devel dbus-1-devel
rustup default stable
```

For another distribution, install the equivalent C compiler toolchain,
`pkg-config`, PipeWire development package, Clang/libclang development package,
Wayland and xkbcommon development packages, and D-Bus development package.

The release builder also compiles Coconut's Qt platform-theme plugin with
CMake and Qt 6 Core/Gui/Widgets/DBus development and private headers. On Fedora
install `cmake qt6-qtbase-devel qt6-qtbase-private-devel`; on Debian/Ubuntu use
`cmake qt6-base-dev qt6-base-private-dev`; on Arch use `cmake qt6-base`.
GTK runtime integration needs GLib/GSettings, dconf,
`gsettings-desktop-schemas`, `xdg-desktop-portal`,
`xdg-desktop-portal-gtk`, and `xsettingsd` for X11/XWayland clients.
See [THEMES.md](../THEMES.md) for adapter builds and isolated native tests.

## Build and run

From the Coconut checkout:

```bash
./dev.sh --build
./dev.sh
```

`dev.sh --build` runs `build.sh --no-upx`, then starts Blair with `--run` and
launches Coconut's shell, background, Appearance, Permissions, FileChooser and
Clipboard services from `.build/`. Without `--build`, it uses the staged binaries as they
are. Application launches resolve `.build/` first, including apps opened from
the shell and file operation menus. The greeter is a separate login process and
is not started inside the desktop session.

The runner creates a private session bus and fresh XDG configuration, data,
cache, state and runtime directories under `.build/dev/session-*/`. D-Bus
activation definitions point to the staged binaries and avoid systemd activation;
the host session's services and activation environment are left alone. Coconut's
portal policy selects local Settings, FileChooser and Access implementations;
the portal broker and optional GTK fallback come from the system installation.
Install `python3`, `dbus-run-session`, GLib's `gdbus`, `xdg-desktop-portal` and
the native dependencies listed above. Other system services are not built by
Coconut. Development preferences start fresh on each run; normal files remain
accessible through the file manager.

Blair is selected from `.build/blair`, `../blair/.build/blair`,
`../blair/target/release/blair`, then `PATH`. `--build` builds Coconut; build Blair
in its checkout separately. Override its executable with `COCONUT_DEV_BLAIR`,
the broker with `COCONUT_DEV_PORTAL`, or the staging directory with
`COCONUT_BUILD_DIR`. Forward compositor arguments after `--`, for example
`./dev.sh -- --profile development`. A graphical parent session selects Blair's
Winit backend for a nested window; a TTY uses its normal backend selection.

Each run prints its directory. Component logs and a sourceable `environment`
file remain there after shutdown. To connect another terminal to the running
development session, source that file and start an app:

```bash
source .build/dev/session-EXAMPLE/environment
cfiles
```

Exit the nested compositor, close the shell, or press Ctrl+C in the launching
terminal to stop the session and its services. The runner checks service bus
ownership during startup and while the session is active.

For individual components on your current desktop:

```bash
cargo build
cargo run -p coconut-background &
cargo run -p coconut-shell
```

Start the settings application independently with:

```bash
cargo run -p coconut-settings
```

The application design and implementation guide is [APPS.md](APPS.md).
Run the terminal emulator with:

```bash
cargo run -p coconut-terminal
```

And the file manager with:

```bash
cargo run -p coconut-files
```

And the system monitor with:

```bash
cargo run -p coconut-monitor
```

And the media player with:

```bash
cargo run -p coconut-media-player
```

And the editor with:

```bash
cargo run -p coconut-editor
```

Run the standard and scientific calculator with:

```bash
cargo run -p coconut-calculator -- --scientific
```

Open an image in Image Viewer with:

```bash
cargo run -p coconut-image-viewer -- path/to/image.png
```

Preview the login screen in a normal, resizable window on any Wayland desktop:

```bash
cargo run -p coconut-greeter -- --preview
```

Preview mode supplies one sample account and session and requires neither
`ALPHIN_SOCKET`, an Alphin daemon, nor Blair. The centered account is ready for
password entry immediately. Use `coconut` to simulate a successful login, or
another password to see the error state. Press Enter or the arrow in the field
to submit. Restart and shut-down requests only show simulated feedback, and
successful sign-in keeps the preview open.

To preview multiple accounts and desktop sessions:

```bash
cargo run -p coconut-greeter -- --preview-multiple
```

With multiple accounts, a horizontal strip of avatars appears at the bottom
left; scroll it when there are more accounts than fit. The session dropdown
appears at the top left only when multiple sessions are available. In a real
login, the greeter prepares the selected account's PAM prompt automatically;
passwordless accounts still require confirmation before opening a session.

Show a small live-session credentials hint beneath the login with:

```bash
coconut-greeter --demouser "hello" --demopassord "world"
```

Both demo flags are required together and only display the supplied example
credentials. They also work with `--preview`; authentication keeps using its
configured backend. `--demopassword` is accepted as an alias for `--demopassord`.

For a real login, launch `coconut-greeter` without `--preview` in the Alphin
environment with `ALPHIN_SOCKET` set. Run `coconut-greeter --help` for usage.

Use the release profile when measuring performance or preparing a local
installation:

```bash
cargo build --release
```

To produce a full set of staged release binaries (compressed with `upx` when
available) and install them, use the repository scripts:

```bash
./build.sh
./install.sh
# Or install into ~/.local without sudo:
./install.sh --user
```

`build.sh` compiles every installable binary into `.build/`; `install.sh`
copies all staged binaries without compiling. Its default is system-wide
installation into `/usr/bin`, requesting `sudo` when needed. `--user` selects
`~/.local` and must run without `sudo`. Add `--build` to compile and stage before
installation; `build.sh --no-upx` stages without compression for development.
The installer reports the source and destination and warns when another copy
has priority in the invoking shell's `PATH`. For example, old `~/.local/bin`
copies can hide newly installed `/usr/bin` binaries. Update the preferred prefix
or remove obsolete copies, then restart running apps to load the new binaries.

## Verification and useful commands

The clipboard service runs separately from the shell and exposes a permission-controlled
D-Bus API. Its [integration guide](../../integrations/clipboard/README.md) covers
configuration, supported compositors and an isolated native clipboard workflow test.
The [optional widget](../../plugins/clipboard/README.md) follows the existing
island/panel architecture and can be placed in either bar.

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p test_dev_session.py -v
```

The development runner tests use a real private session bus with fixture
components. They require the system Python's GObject introspection bindings
(`python3-gobject` or `python3-gi`).

### Troubleshooting native dependencies

`libpipewire-0.3` not found means the PipeWire development package is missing.
Install the package listed above for your distribution; `pkg-config` should then
find `libpipewire-0.3.pc` without setting `PKG_CONFIG_PATH`.

If `bindgen` cannot find `libclang`, install the libclang development package.
On installations with a nonstandard LLVM location, point `LIBCLANG_PATH` at the
directory containing `libclang.so`, for example:

```bash
export LIBCLANG_PATH=/usr/lib/llvm-18/lib
```

Confirm the PipeWire metadata is visible with:

```bash
pkg-config --modversion libpipewire-0.3
```

## Desktop context menus

Desktop files use the shared private thumbnail cache for images, video, audio
covers and PE icons. A badge at the lower left identifies the MIME default
application when it has an icon; a badge at the lower right identifies symlinks.
Launcher icons and generic MIME/folder icons remain the fallback.

The desktop uses `coconut-file-menu` for item and directory-background actions.
Right-clicking an icon selects it without activating it; empty-space clicks clear
the selection. Common file actions match File Explorer. The desktop adds Sort
icons, Align to grid, Refresh and Desktop settings. Refresh preserves icon
positions by file path and discards superseded background loads.

Keep generic menu behavior in CreamUI, file operations and shared menu composition
in [coconut-file-menu](../../crates/file-menu/README.md), and desktop-only actions
in `desktop/shell/src/desktop/menus.rs`. The native Wayland backend uses
`xdg_popup` and layer-shell parenting, with a separate surface for each submenu.
The compositor places and constrains menus, and owns grabs and dismissal. Native
menus stay open when focus moves into a child popup; call `parent_focus_lost`
instead of closing them unconditionally. Other backends use an in-window overlay.
File creation and rename use the same themed input dialogs on both surfaces.

## Bar content editing

The Statusbar and Dockbar Contents pages provide group selection, a searchable
item catalog, ordering, visibility, spacing, placement-specific removal and Undo.
Drag handles in Settings to arrange items, move them between groups, or reorder
groups. Right-click the running bar for a short menu with settings and removal;
layout controls and the item catalog stay in Settings. Content edits apply live and Settings
follows changes made from the shell. See [Settings](../../apps/settings/README.md)
for routes, persistence behavior and scene tests.

Keep bar operations independent of widgets in `coconut-core::BarEdit`. Shared
catalog metadata, menu composition and background persistence live in
`coconut-plugin-kit::bar_editor`; shell hit routing and Settings presentation
remain in their respective consumers.
