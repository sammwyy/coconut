#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        -h | --help)
            cat <<'USAGE'
Usage: ./scripts/setup.sh [--dry-run]

Install native build and development-session dependencies for Coconut and Blair.
  --dry-run   Show package manager commands without installing anything.

Run as your regular user; sudo is requested only for package manager commands.
You may also run the script with sudo. Manage Rust with rustup as your regular
user; this script never installs a system Rust toolchain.
Package managers may prompt before applying changes.
USAGE
            exit 0
            ;;
        *) echo "setup.sh: unknown argument: $arg" >&2; exit 2 ;;
    esac
done

release=/etc/os-release
if [ "$DRY_RUN" -eq 1 ]; then
    release="${COCONUT_SETUP_OS_RELEASE:-$release}"
fi
if [ ! -f "$release" ]; then
    echo "setup.sh: OS release file not found: $release" >&2
    exit 1
fi
unset ID ID_LIKE
. "$release"
id="${ID:-}"
case "$id" in
    fedora) family=fedora ;;
    debian | ubuntu) family=debian ;;
    arch) family=arch ;;
    opensuse*) family=opensuse ;;
    rhel | rocky | centos | almalinux | ol)
        echo "setup.sh: $id needs distribution-specific repositories and is not automated; see docs/dev/SHELL.md" >&2
        exit 1
        ;;
    *)
        family=""
        for candidate in ${ID_LIKE:-}; do
            case "$candidate" in
                fedora) family=fedora; break ;;
                debian | ubuntu) family=debian; break ;;
                arch) family=arch; break ;;
                opensuse | suse) family=opensuse; break ;;
            esac
        done
        ;;
esac
if [ -z "$family" ]; then
    echo "setup.sh: unsupported distribution: ${ID:-unknown} (ID_LIKE=${ID_LIKE:-none})" >&2
    exit 1
fi

case "$family" in
    fedora)
        manager=dnf
        packages=(
            @development-tools pkgconf-pkg-config
            pipewire-devel clang clang-devel wayland-devel libxkbcommon-devel dbus-devel
            libseat-devel libinput-devel systemd-devel libdrm-devel mesa-libgbm-devel
            mesa-libEGL-devel mesa-libGLES-devel cmake qt6-qtbase-devel
            qt6-qtbase-private-devel python3 python3-gobject dbus-daemon glib2
            xdg-desktop-portal xdg-desktop-portal-gtk glib2-devel dconf
            gsettings-desktop-schemas xsettingsd
        )
        ;;
    debian)
        manager=apt-get
        packages=(
            build-essential pkg-config libpipewire-0.3-dev
            clang libclang-dev libwayland-dev libxkbcommon-dev libdbus-1-dev
            libseat-dev libinput-dev libudev-dev libdrm-dev libgbm-dev
            libegl1-mesa-dev libgles2-mesa-dev cmake qt6-base-dev
            qt6-base-private-dev python3 python3-gi dbus dbus-user-session
            libglib2.0-bin xdg-desktop-portal xdg-desktop-portal-gtk
            dconf-gsettings-backend gsettings-desktop-schemas xsettingsd
        )
        ;;
    arch)
        manager=pacman
        packages=(
            base-devel pkgconf pipewire clang libclang wayland
            libxkbcommon dbus libseat libinput libdrm mesa systemd cmake qt6-base
            python python-gobject xdg-desktop-portal xdg-desktop-portal-gtk
            dconf gsettings-desktop-schemas xsettingsd
        )
        ;;
    opensuse)
        manager=zypper
        packages=(
            pkg-config pipewire-devel clang libclang-devel
            wayland-devel libxkbcommon-devel dbus-1-devel libseat-devel
            libinput-devel libudev-devel libdrm-devel Mesa-libgbm-devel
            Mesa-libEGL-devel Mesa-libGLESv2-devel cmake qt6-base-devel
            qt6-base-private-devel python3 python3-gobject
            xdg-desktop-portal xdg-desktop-portal-gtk dconf
            gsettings-desktop-schemas xsettingsd
        )
        ;;
esac

if [ "$DRY_RUN" -eq 0 ]; then
    command -v "$manager" >/dev/null || { echo "setup.sh: $manager not found on $id" >&2; exit 1; }
    if [ "$(id -u)" -ne 0 ]; then
        command -v sudo >/dev/null || { echo "setup.sh: sudo is required to install packages" >&2; exit 1; }
    fi
fi

run() {
    local command=("$@")
    if [ "$(id -u)" -ne 0 ]; then
        command=(sudo "${command[@]}")
    fi
    printf '  '
    printf '%q ' "${command[@]}"
    printf '\n'
    if [ "$DRY_RUN" -eq 0 ]; then
        "${command[@]}"
    fi
}

printf 'Distribution: %s (%s)\nPackage manager: %s\n' "$id" "$family" "$manager"
case "$family" in
    fedora) run dnf install "${packages[@]}" ;;
    debian)
        run apt-get update
        run apt-get install "${packages[@]}"
        ;;
    arch)
        printf 'Arch requires a full system upgrade when installing packages:\n'
        run pacman -Syu --needed "${packages[@]}"
        ;;
    opensuse)
        run zypper install -t pattern devel_basis
        run zypper install "${packages[@]}"
        ;;
esac
printf 'System dependencies %s.\n' \
    "$([ "$DRY_RUN" -eq 1 ] && printf planned || printf installed)"
if [ "$(id -u)" -ne 0 ]; then
    if ! command -v rustup >/dev/null || ! command -v cargo >/dev/null || ! command -v rustc >/dev/null; then
        printf 'Rust is not available in this user PATH; install rustup and stable as your regular user, without sudo.\n' >&2
    fi
elif [ -n "${SUDO_USER:-}" ]; then
    printf 'Check rustup and cargo as %s, not as root.\n' "$SUDO_USER"
fi
