# Developing Coconut

Coconut is a Rust workspace for a Wayland desktop shell. This guide covers the
local toolchain, native libraries, and everyday development commands.

## Requirements

Use a current stable Rust toolchain and install the native development packages
for your distribution. Coconut links to PipeWire and uses `bindgen` while
building; therefore, the PipeWire headers, `pkg-config`, Clang, and libclang
must be present.

The repository currently uses local path dependencies for CreamUI and Blair.
Keep all three repositories side by side:

```text
workspace/
├── blair/
├── creamui/
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

## Build and run

From the Coconut checkout:

```bash
cargo build
cargo run -p coconut-background &
cargo run -p coconut-shell
```

Start the settings application independently with:

```bash
cargo run -p coconut-settings
```

And the terminal emulator with:

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
sudo ./install.sh
```

`build.sh` compiles every installable binary into `.build/`; `install.sh`
lays them down without compiling, system-wide with `sudo` or under
`~/.local` without it.

## Verification and useful commands

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

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
