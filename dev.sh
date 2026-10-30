#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${COCONUT_BUILD_DIR:-$ROOT/.build}"
DO_BUILD=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --build) DO_BUILD=1 ;;
        -h | --help)
            cat <<'USAGE'
Usage: ./dev.sh [--build] [-- BLAIR_ARGUMENTS...]

Start Coconut and its portals from .build/ on a private session bus.
  --build   Run build.sh --no-upx before starting the session.
  --        Forward remaining arguments to Blair.

COCONUT_BUILD_DIR selects the staged binaries directory.
COCONUT_DEV_BLAIR selects a Blair executable instead of the local build.
COCONUT_DEV_PORTAL selects the system xdg-desktop-portal executable.
USAGE
            exit 0
            ;;
        --) shift; break ;;
        *) echo "dev.sh: unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ "$DO_BUILD" -eq 1 ]; then
    bash "$ROOT/build.sh" --no-upx
fi

exec python3 "$ROOT/tools/dev_session.py" --build-dir "$BUILD_DIR" -- "$@"
