#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

INSTALL_PREFIX="${INSTALL_PREFIX:-/usr/local}"
BIN_DIR="$INSTALL_PREFIX/bin"

# ---------------------------------------------------------------------------

die()  { printf 'error: %s\n' "$*" >&2; exit 1; }
info() { printf '==> %s\n' "$*"; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

sudo_cmd() {
  if [ "${EUID:-$(id -u)}" -eq 0 ]; then
    "$@"
  else
    sudo "$@"
  fi
}

choose_from_menu() {
  local prompt="$1"; shift
  local options=("$@")
  [ "${#options[@]}" -gt 0 ] || die "no options for: $prompt"
  printf '\n%s\n' "$prompt" >&2
  local i=1
  for opt in "${options[@]}"; do
    printf '  %d) %s\n' "$i" "$opt" >&2
    i=$((i + 1))
  done
  local choice
  while true; do
    printf '> ' >&2
    if ! read -r choice; then
      printf '\n' >&2
      die "no input received; run from an interactive terminal"
    fi
    if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le "${#options[@]}" ]; then
      printf '%s\n' "${options[$((choice - 1))]}"
      return 0
    fi
    printf 'Invalid selection. Choose 1-%d.\n' "${#options[@]}" >&2
  done
}

build_coconut() {
  local cargo_args=("build" "-p" "coconut-shell" "-p" "coconut-settings")
  [ "$1" = "release" ] && cargo_args+=("--release")
  info "Building Coconut ($1)"
  cargo "${cargo_args[@]}"
}

install_binaries() {
  local dir="$1"
  [ -x "$ROOT_DIR/target/$dir/coconut" ] || die "coconut binary not found in target/$dir"
  [ -x "$ROOT_DIR/target/$dir/coconut-settings" ] || die "coconut-settings binary not found in target/$dir"
  info "Installing binaries into $BIN_DIR"
  sudo_cmd install -d "$BIN_DIR"
  sudo_cmd install -m 0755 "$ROOT_DIR/target/$dir/coconut" "$BIN_DIR/coconut"
  sudo_cmd install -m 0755 "$ROOT_DIR/target/$dir/coconut-settings" "$BIN_DIR/coconut-settings"
}

# ---------------------------------------------------------------------------

main() {
  cd "$ROOT_DIR"

  info "Coconut installer"

  command_exists cargo || die "cargo is not installed"
  if [ "${EUID:-$(id -u)}" -ne 0 ] && ! command_exists sudo; then
    die "sudo is required to install into $INSTALL_PREFIX"
  fi

  local build_mode
  build_mode="$(choose_from_menu "Choose build mode:" "debug" "release")"

  local profile_dir="debug"
  [ "$build_mode" = "release" ] && profile_dir="release"

  build_coconut "$build_mode"
  install_binaries "$profile_dir"

  info ""
  info "Installed:"
  info "  $BIN_DIR/coconut"
  info "  $BIN_DIR/coconut-settings"
  info ""
  info "Coconut writes its own config to ~/.config/coconut/shell.toml on first run."
  info "Run 'coconut' directly, or './scripts/setup-blair.sh' to configure the"
  info "Blair compositor to launch it as its primary client."
}

main "$@"
