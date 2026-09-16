#!/usr/bin/env bash
# Configures the Blair compositor to launch coconut as its primary client.
set -euo pipefail

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
CONFIG_DIR="$CONFIG_HOME/blair"
CONFIG_FILE="$CONFIG_DIR/compositor.toml"

info() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

if ! command -v coconut >/dev/null 2>&1; then
  warn "coconut is not on PATH — run ./scripts/install.sh first, or make sure"
  warn "its install location is on PATH before starting a Blair session."
fi

mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_FILE" ]; then
  info "creating $CONFIG_FILE"
  cat > "$CONFIG_FILE" <<'EOF'
[general]
primary_client = "coconut"
spawn_primary_client = true
backend = "auto"
EOF
  info "Blair will launch coconut as its primary client."
  exit 0
fi

info "updating $CONFIG_FILE"
cp "$CONFIG_FILE" "$CONFIG_FILE.bak"

TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT

# Rewrites primary_client/spawn_primary_client right after the [general]
# header, dropping any pre-existing occurrences of those two keys further
# down in the section. Appends a whole new [general] section at the end if
# the file has none. Every other key, section, and blank line passes through
# untouched.
awk '
BEGIN { in_general = 0; general_seen = 0 }
/^\[general\]/ {
    print
    print "primary_client = \"coconut\""
    print "spawn_primary_client = true"
    in_general = 1
    general_seen = 1
    next
}
/^\[/ {
    in_general = 0
    print
    next
}
{
    if (in_general && $0 ~ /^primary_client[[:space:]]*=/) { next }
    if (in_general && $0 ~ /^spawn_primary_client[[:space:]]*=/) { next }
    print
}
END {
    if (!general_seen) {
        print ""
        print "[general]"
        print "primary_client = \"coconut\""
        print "spawn_primary_client = true"
    }
}
' "$CONFIG_FILE" > "$TMP_FILE"

mv "$TMP_FILE" "$CONFIG_FILE"
trap - EXIT

info "Blair will launch coconut as its primary client (backup: $CONFIG_FILE.bak)."
