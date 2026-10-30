#!/bin/bash
# Coconut builder: compiles every installable binary in this repository with
# the release profile and stages the results in .build/ so install.sh can lay
# them down without a Rust toolchain or the sources at install time.
#
# When `upx` is available each staged binary is compressed in place; without
# it the binaries are staged uncompressed — compression is optional size
# shaving, never a requirement.
set -e
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${COCONUT_BUILD_DIR:-$ROOT/.build}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# ---------------------------------------------------------------- binaries

# Every "package:binary" the installer lays down. Package names come from
# each member's Cargo.toml; binary names from their [[bin]] section.
PACKAGES=(
    coconut-shell:coconut
    coconut-background:coconut-bg
    coconut-greeter:coconut-greeter
    coconut-settings:coconut-settings
    coconut-integration-appearance:coconut-appearance
    coconut-terminal:cterm
    coconut-files:cfiles
    coconut-monitor:cmonitor
    coconut-media-player:cplayer
)

pkgs=()
for entry in "${PACKAGES[@]}"; do
    pkgs+=(-p "${entry%%:*}")
done

cargo build --release --manifest-path "$ROOT/Cargo.toml" "${pkgs[@]}"
bash "$ROOT/integrations/qt/build.sh" "$BUILD_DIR/qt"

# ----------------------------------------------------------------- staging

mkdir -p "$BUILD_DIR"

for entry in "${PACKAGES[@]}"; do
    bin="${entry##*:}"
    install -m 755 "$TARGET_DIR/release/$bin" "$BUILD_DIR/$bin"
done

# -------------------------------------------------------------------- size

if command -v upx >/dev/null 2>&1; then
    # Default UCL compression; --best (LZMA) is ~50x slower for only ~2%
    # better ratio. Binaries compress in parallel — each is independent.
    for entry in "${PACKAGES[@]}"; do
        bin="${entry##*:}"
        upx -q "$BUILD_DIR/$bin" || echo "warning: upx could not compress $bin" >&2 &
    done
    wait
    # upx works through <name>.upx temp files; sweep any orphaned by a
    # failed or interrupted compression.
    rm -f "$BUILD_DIR"/*.upx
else
    echo "upx not found; binaries staged uncompressed."
fi

echo "Staged ${#PACKAGES[@]} release binaries in $BUILD_DIR:"
for entry in "${PACKAGES[@]}"; do
    bin="${entry##*:}"
    printf '  %-16s %s\n' "$bin" "$(du -h "$BUILD_DIR/$bin" | cut -f1)"
done
echo "Run ./install.sh (with sudo for a system-wide install) to lay them down."
