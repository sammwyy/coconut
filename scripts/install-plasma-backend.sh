#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script="$root/plasma/contents/code/main.js"
if kpackagetool6 --type KWin/Script --list | grep -qx creamshell-dock; then
    kpackagetool6 --type KWin/Script --upgrade "$root/plasma"
else
    kpackagetool6 --type KWin/Script --install "$root/plasma"
fi
kwriteconfig6 --file kwinrc --group Plugins --key creamshell-dockEnabled true

busctl --user call org.kde.KWin /KWin org.kde.KWin reconfigure
busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting unloadScript s creamshell-dock || true
result="$(busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting loadScript ss "$script" creamshell-dock)"
id="${result#* }"
busctl --user call org.kde.KWin "/Scripting/Script$id" org.kde.kwin.Script run
