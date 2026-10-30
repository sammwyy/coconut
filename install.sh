#!/bin/bash
# Coconut installer: lays down the release binaries staged by ./build.sh in
# .build/, registers the portal backends, and creates a "Coconut" session
# profile (startcoconut + wayland-sessions entry) so greeters and session
# managers can discover it.
#
# Usage: ./install.sh [WM] [--build] [--user] [--force|force]
#
#   WM        compositor for the session profile (e.g. blair, kwin,
#             hyprland); used even when not auto-detected. No WM argument
#             asks interactively.
#   --build   run ./build.sh first to compile and stage the binaries.
#   --user    install for the current user (~/.local) instead of
#             system-wide; without it the script elevates with sudo.
#   --force   never prompt: a missing WM detection is accepted silently and
#             an unset WM picks the first detected (or blair).
#
# COCONUT_WM still selects the compositor when no WM argument is given.
# External compositors are only *used*, never installed or configured here.
set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${COCONUT_BUILD_DIR:-$ROOT/.build}"
CALLER_PATH="${COCONUT_INSTALL_CALLER_PATH:-$PATH}"
BINARIES=(coconut coconut-bg coconut-greeter coconut-settings coconut-appearance
    cterm cfiles cmonitor cplayer cedit ccalc cviewer coconut-screenshots coconut-permission-portal)

# -------------------------------------------------------------- arguments

WM_ARG=""
DO_BUILD=0
USER_INSTALL=0
FORCE=0
for arg in "$@"; do
    case "$arg" in
        --build) DO_BUILD=1 ;;
        --user) USER_INSTALL=1 ;;
        --force | force) FORCE=1 ;;
        -h | --help)
            sed -n '7,18p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        --*)
            echo "install.sh: unknown flag: $arg" >&2
            exit 2
            ;;
        *)
            if [ -z "$WM_ARG" ]; then
                WM_ARG="$arg"
            else
                echo "install.sh: unexpected extra argument: $arg" >&2
                exit 2
            fi
            ;;
    esac
done

if [ "$USER_INSTALL" -eq 1 ] && [ "$(id -u)" -eq 0 ]; then
    echo "install.sh: run --user without sudo; it installs into the invoking user's directories" >&2
    exit 2
fi

# ------------------------------------------------------------------ build

# Build as the invoking user, before any sudo elevation, so target/ stays
# owned by them and sudo never touches the toolchain.
if [ "$DO_BUILD" -eq 1 ] && [ "${COCONUT_INSTALL_BUILT:-}" != 1 ]; then
    bash "$ROOT/build.sh"
fi

# ------------------------------------------------------- privilege check

# Global install is the default: elevate with sudo unless --user was given.
# COCONUT_INSTALL_BUILT marks that the build already ran pre-elevation.
if [ "$USER_INSTALL" -eq 0 ] && [ "$(id -u)" -ne 0 ]; then
    if ! command -v sudo >/dev/null 2>&1; then
        echo "install.sh: a system-wide install needs root; install sudo or pass --user" >&2
        exit 1
    fi
    exec sudo COCONUT_INSTALL_BUILT=1 COCONUT_BUILD_DIR="$BUILD_DIR" \
        COCONUT_INSTALL_CALLER_PATH="$CALLER_PATH" bash "$ROOT/install.sh" "$@"
fi

# ---------------------------------------------------------------- binaries

missing=()
for bin in "${BINARIES[@]}"; do
    [ -f "$BUILD_DIR/$bin" ] || missing+=("$bin")
done
if [ "${#missing[@]}" -ne 0 ]; then
    echo "install.sh: staged binaries missing: ${missing[*]}" >&2
    echo "Run ./build.sh first (as a regular user), then re-run this script." >&2
    exit 1
fi
qt_plugins=("$BUILD_DIR"/qt/plugins/platformthemes/libcoconut-qt*.so)
if [ ! -f "${qt_plugins[0]}" ]; then
    echo "install.sh: Qt theme plugin missing; run ./build.sh first" >&2
    exit 1
fi

# ------------------------------------------------------------ install root

if [ "$(id -u)" -eq 0 ]; then
    INSTALL_BIN=/usr/bin
    DATA=/usr/share
    CONF_DIR=/etc/xdg-desktop-portal
    USER_UNITS=/usr/lib/systemd/user
else
    INSTALL_BIN="${COCONUT_INSTALL_BIN:-$HOME/.local/bin}"
    DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
    CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
    CONF_DIR="$CONFIG/xdg-desktop-portal"
    USER_UNITS="$CONFIG/systemd/user"
fi

mkdir -p "$INSTALL_BIN" \
    "$DATA/dbus-1/services" \
    "$DATA/xdg-desktop-portal/portals" \
    "$DATA/applications" \
    "$USER_UNITS" \
    "$CONF_DIR"

for bin in "${BINARIES[@]}"; do
    install -m 755 "$BUILD_DIR/$bin" "$INSTALL_BIN/$bin"
done

mkdir -p "$DATA/coconut/themes" "$DATA/coconut/qt/plugins/platformthemes"
cp -a "$ROOT/themes/." "$DATA/coconut/themes/"
mkdir -p "$DATA/fonts/coconut"
install -m 644 "$ROOT/assets/fonts/plus-jakarta-sans/Regular.ttf" \
    "$ROOT/assets/fonts/plus-jakarta-sans/SemiBold.ttf" "$DATA/fonts/coconut/"
command -v fc-cache >/dev/null && fc-cache "$DATA/fonts/coconut" || true
for plugin in "${qt_plugins[@]}"; do
    install -m 755 "$plugin" "$DATA/coconut/qt/plugins/platformthemes/"
done
data_sed="$(printf '%s' "$DATA" | sed 's/[&|\\]/\\&/g')"
sed "s|@INSTALL_DATA@|$data_sed|g" "$ROOT/packaging/coconut-environment" > "$DATA/coconut/environment"

# sed replacement treats &, | and \ specially; escape them so install
# roots like "cargo & apps" survive the substitution.
bin_sed="$(printf '%s' "$INSTALL_BIN" | sed 's/[&|\\]/\\&/g')"

# --------------------------------------------------- portal + applications

# The service definitions ship in the repo referencing /usr/bin; rewrite the
# prefix for user-local installs.
sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/apps/files/data/org.freedesktop.impl.portal.desktop.cfiles.service" \
    > "$DATA/dbus-1/services/org.freedesktop.impl.portal.desktop.cfiles.service"

sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/apps/files/data/cfiles-portal.service" \
    > "$USER_UNITS/cfiles-portal.service"

cp "$ROOT/apps/files/data/cfiles.portal" \
    "$DATA/xdg-desktop-portal/portals/"

sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/integrations/appearance/data/org.freedesktop.impl.portal.desktop.coconut.service" \
    > "$DATA/dbus-1/services/org.freedesktop.impl.portal.desktop.coconut.service"
sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/integrations/appearance/data/coconut-appearance.service" \
    > "$USER_UNITS/coconut-appearance.service"
cp "$ROOT/integrations/appearance/data/coconut.portal" "$DATA/xdg-desktop-portal/portals/"

sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/apps/permission-portal/data/org.coconut.Permissions.service" \
    > "$DATA/dbus-1/services/org.coconut.Permissions.service"
sed "s|/usr/bin|$bin_sed|g" \
    "$ROOT/apps/permission-portal/data/coconut-permission-portal.service" \
    > "$USER_UNITS/coconut-permission-portal.service"
cp "$ROOT/apps/permission-portal/data/coconut-permissions.portal" \
    "$DATA/xdg-desktop-portal/portals/"
cp "$ROOT/apps/files/data/cfiles.desktop" \
    "$ROOT/apps/terminal/data/cterm.desktop" \
    "$ROOT/apps/monitor/data/cmonitor.desktop" \
    "$ROOT/apps/media-player/data/cplayer.desktop" \
    "$ROOT/apps/editor/data/cedit.desktop" \
    "$ROOT/apps/calculator/data/ccalc.desktop" \
    "$ROOT/apps/viewer/data/cviewer.desktop" \
    "$ROOT/apps/screenshots/data/coconut-screenshots.desktop" \
    "$DATA/applications/"

for conf in blair-portals.conf coconut-portals.conf; do
    # Upgrade previously generated policies; preserve custom user selections.
    old_policy=$(printf '[preferred]\norg.freedesktop.impl.portal.FileChooser=cfiles')
    shipped_policy=$(printf '[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.Settings=coconut\norg.freedesktop.impl.portal.FileChooser=cfiles;gtk')
    current="$(cat "$CONF_DIR/$conf" 2>/dev/null)"
    if [ ! -f "$CONF_DIR/$conf" ] || [ "$current" = "$old_policy" ] || [ "$current" = "$shipped_policy" ]; then
        cp "$ROOT/integrations/appearance/data/coconut-portals.conf" "$CONF_DIR/$conf"
    fi
done

command -v update-desktop-database >/dev/null &&
    update-desktop-database "$DATA/applications" || true
# User managers load units from ~/.config/systemd/user eagerly; the packaged
# /usr/lib/systemd/user path is picked up on next login, so only the
# user-local install needs a reload.
if [ "$(id -u)" -ne 0 ]; then
    command -v systemctl >/dev/null && systemctl --user daemon-reload || true
fi

# ------------------------------------------------------------- WM + session

# Compositors Coconut knows how to boot a session under. `blair` and
# `kwin_wayland` have native session hooks in startcoconut; anything else
# uses the generic "wait for the socket, then start the shell" path.
WM_CANDIDATES=(blair kwin_wayland Hyprland sway labwc river niri weston)

detected=()
for wm in "${WM_CANDIDATES[@]}"; do
    command -v "$wm" >/dev/null 2>&1 && detected+=("$wm")
done

# Friendly aliases for the candidate names: kwin -> kwin_wayland and
# case-insensitive matching for the rest (hyprland -> Hyprland, ...).
resolve_wm() {
    local want="${1,,}" candidate
    [ "$want" = "kwin" ] && want="kwin_wayland"
    for candidate in "${WM_CANDIDATES[@]}"; do
        if [ "${candidate,,}" = "$want" ]; then
            printf '%s' "$candidate"
            return 0
        fi
    done
    printf '%s' "$1"
}

if [ -n "$WM_ARG" ]; then
    # An explicit compositor wins even when it is not installed yet.
    wm="$(resolve_wm "$WM_ARG")"
    if ! printf '%s\n' "${detected[@]}" | grep -qxF "$wm" && [ "$FORCE" -eq 0 ]; then
        echo "note: '$wm' was not found in PATH; installing the session profile anyway."
    fi
elif [ -n "${COCONUT_WM:-}" ]; then
    wm="$(resolve_wm "$COCONUT_WM")"
elif [ "$FORCE" -eq 1 ]; then
    wm="${detected[0]:-blair}"
elif [ "${#detected[@]}" -eq 0 ]; then
    echo "No supported window manager found (looked for: ${WM_CANDIDATES[*]})."
    read -rp "Enter a compositor command to use, or leave empty to skip the session profile: " wm
else
    echo "Detected window managers:"
    for i in "${!detected[@]}"; do
        echo "  $((i + 1))) ${detected[$i]}"
    done
    read -rp "Window manager for the Coconut session [1, or a command]: " choice
    if [ -z "$choice" ]; then
        wm="${detected[0]}"
    elif [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#detected[@]}" ]; then
        wm="${detected[$((choice - 1))]}"
    else
        wm="$choice"
    fi
fi

if [ -z "${wm:-}" ]; then
    echo "Skipping session profile (no window manager selected)."
else
    SESSIONS_DIR="${COCONUT_SESSIONS_DIR:-$DATA/wayland-sessions}"
    mkdir -p "$SESSIONS_DIR"

    # sed replacement treats &, | and \ specially; escape them so install
    # roots like "cargo & apps" survive the substitution.
    wm_sed="$(printf '%s' "$wm" | sed 's/[&|\\]/\\&/g')"

    sed -e "s|@COCONUT_WM@|$wm_sed|g" \
        -e "s|@INSTALL_BIN@|$bin_sed|g" \
        -e "s|@INSTALL_DATA@|$data_sed|g" \
        "$ROOT/packaging/startcoconut" > "$INSTALL_BIN/startcoconut"
    chmod +x "$INSTALL_BIN/startcoconut"

    # The session launcher resolves its siblings from its own directory, so
    # it installs verbatim — no substitutions needed.
    install -m 755 "$ROOT/packaging/coconut-session" "$INSTALL_BIN/coconut-session"

    sed "s|@INSTALL_BIN@|$bin_sed|g" \
        "$ROOT/packaging/coconut.desktop" > "$SESSIONS_DIR/coconut.desktop"

    echo "Session profile \"Coconut\" installed:"
    echo "  bootstrapper : $INSTALL_BIN/startcoconut (wm: $wm)"
    echo "  session procs: $INSTALL_BIN/coconut-session (bg + shell)"
    echo "  session entry: $SESSIONS_DIR/coconut.desktop"
    if [ "$SESSIONS_DIR" != "/usr/share/wayland-sessions" ]; then
        echo "  note: system greeters may need it in /usr/share/wayland-sessions/"
        echo "        (re-run this script with sudo for a system-wide install)"
    fi
fi

printf '\nInstalled %s staged binaries from %s into %s.\n' "${#BINARIES[@]}" "$BUILD_DIR" "$INSTALL_BIN"
shadowed=()
unresolved=()
for bin in "${BINARIES[@]}"; do
    resolved="$(PATH="$CALLER_PATH" command -v "$bin" || true)"
    if [ -z "$resolved" ]; then
        unresolved+=("$bin")
    elif [ ! "$resolved" -ef "$INSTALL_BIN/$bin" ]; then
        shadowed+=("$bin: $resolved")
    fi
done
if [ "${#shadowed[@]}" -ne 0 ]; then
    echo "Warning: your PATH selects other copies instead of the binaries just installed:"
    printf '  %s\n' "${shadowed[@]}"
    echo "Update those copies with --user, remove obsolete installations, or adjust PATH."
fi
if [ "${#unresolved[@]}" -ne 0 ]; then
    echo "Note: these installed commands are not in your PATH: ${unresolved[*]}"
    echo "Add $INSTALL_BIN to PATH or launch them by their full path."
fi
