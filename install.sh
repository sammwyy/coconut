#!/bin/bash
# Coconut installer: builds and installs every binary in this repository,
# registers File Explorer's portal backend, and creates a "Coconut" session
# profile (startcoconut + wayland-sessions entry) so greeters and session
# managers can discover it.
#
# The window manager for the session is detected interactively; set
# COCONUT_WM (e.g. blair, kwin_wayland, Hyprland) to skip the prompt.
# External compositors are only *used*, never installed or configured here.
set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ---------------------------------------------------------------- binaries

for pkg in \
    desktop/shell \
    desktop/background \
    desktop/greeter \
    apps/settings \
    apps/terminal \
    apps/files \
    apps/monitor \
    apps/media-player; do
    cargo install --path "$ROOT/$pkg"
done

INSTALL_BIN="${CARGO_INSTALL_ROOT:-$HOME/.cargo}/bin"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"

# --------------------------------------------------- portal + applications

mkdir -p "$DATA/dbus-1/services" \
    "$DATA/xdg-desktop-portal/portals" \
    "$DATA/applications" \
    "$CONFIG/systemd/user" \
    "$CONFIG/xdg-desktop-portal"

cat > "$DATA/dbus-1/services/org.freedesktop.impl.portal.desktop.cfiles.service" <<EOF
[D-BUS Service]
Name=org.freedesktop.impl.portal.desktop.cfiles
Exec="$INSTALL_BIN/cfiles" --portal
SystemdService=cfiles-portal.service
EOF

cat > "$CONFIG/systemd/user/cfiles-portal.service" <<EOF
[Unit]
Description=File Explorer XDG Desktop Portal backend
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=dbus
BusName=org.freedesktop.impl.portal.desktop.cfiles
ExecStart="$INSTALL_BIN/cfiles" --portal
Environment=DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus
EOF

cp "$ROOT/apps/files/data/cfiles.portal" \
    "$DATA/xdg-desktop-portal/portals/"
cp "$ROOT/apps/files/data/cfiles.desktop" \
    "$ROOT/apps/terminal/data/cterm.desktop" \
    "$ROOT/apps/monitor/data/cmonitor.desktop" \
    "$ROOT/apps/media-player/data/cplayer.desktop" \
    "$DATA/applications/"

for conf in blair-portals.conf coconut-portals.conf; do
    if [ ! -f "$CONFIG/xdg-desktop-portal/$conf" ]; then
        cat > "$CONFIG/xdg-desktop-portal/$conf" <<EOF
[preferred]
org.freedesktop.impl.portal.FileChooser=cfiles
EOF
    fi
done

command -v update-desktop-database >/dev/null &&
    update-desktop-database "$DATA/applications" || true
command -v systemctl >/dev/null && systemctl --user daemon-reload || true

# ------------------------------------------------------------- WM + session

# Compositors Coconut knows how to boot a session under. `blair` and
# `kwin_wayland` have native session hooks in startcoconut; anything else
# uses the generic "wait for the socket, then start the shell" path.
WM_CANDIDATES=(blair kwin_wayland Hyprland sway labwc river niri weston)

detected=()
for wm in "${WM_CANDIDATES[@]}"; do
    command -v "$wm" >/dev/null 2>&1 && detected+=("$wm")
done

if [ -n "${COCONUT_WM:-}" ]; then
    wm="$COCONUT_WM"
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
    if [ "$(id -u)" -eq 0 ]; then
        SESSIONS_DIR=/usr/share/wayland-sessions
    else
        SESSIONS_DIR="${COCONUT_SESSIONS_DIR:-$DATA/wayland-sessions}"
    fi
    mkdir -p "$INSTALL_BIN" "$SESSIONS_DIR"

    # sed replacement treats &, | and \ specially; escape them so install
    # roots like "cargo & apps" survive the substitution.
    wm_sed="$(printf '%s' "$wm" | sed 's/[&|\\]/\\&/g')"
    bin_sed="$(printf '%s' "$INSTALL_BIN" | sed 's/[&|\\]/\\&/g')"

    sed -e "s|@COCONUT_WM@|$wm_sed|g" \
        -e "s|@INSTALL_BIN@|$bin_sed|g" \
        "$ROOT/packaging/startcoconut" > "$INSTALL_BIN/startcoconut"
    chmod +x "$INSTALL_BIN/startcoconut"

    sed "s|@INSTALL_BIN@|$bin_sed|g" \
        "$ROOT/packaging/coconut.desktop" > "$SESSIONS_DIR/coconut.desktop"

    echo "Session profile \"Coconut\" installed:"
    echo "  bootstrapper : $INSTALL_BIN/startcoconut (wm: $wm)"
    echo "  session entry: $SESSIONS_DIR/coconut.desktop"
    if [ "$SESSIONS_DIR" != "/usr/share/wayland-sessions" ]; then
        echo "  note: system greeters may need it in /usr/share/wayland-sessions/"
        echo "        (e.g. sudo cp \"$SESSIONS_DIR/coconut.desktop\" /usr/share/wayland-sessions/)"
    fi
fi
