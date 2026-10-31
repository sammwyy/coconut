# Coconut clipboard service

`coconut-integration-clipboard` provides the Linux session executable
`coconut-clipboard`, its versioned D-Bus API, and a blocking Rust client.
The service owns clipboard capture and history independently of the shell.
Restarting the shell or hiding its clipboard widget does not discard history.

The installed session starts it alongside `coconut-bg`. D-Bus activation and
the `coconut-clipboard.service` systemd user unit also start it on demand.
The development runner activates the staged service on its private bus.
It exits when its Wayland connection ends; systemd's graphical-session target
controls the installed unit's lifetime.

## History and configuration

Open **Settings → Privacy → Clipboard** to enable/pause history, set its entry
limit and retention time, and manage application decisions. These changes apply
immediately and save atomically. Pausing stops new history entries; current
clipboard access and existing history remain available. Clear history and Remove
do not clear the current clipboard.

Configuration lives at `$XDG_CONFIG_HOME/coconut/clipboard.toml`, defaulting to
`~/.config/coconut/clipboard.toml`:

```toml
enabled = true
max_entries = 50
max_item_bytes = 1048576
max_total_bytes = 8388608
max_age_seconds = 86400
# seat = "seat0"
```

Manual file edits take effect after restarting the service. Conflicting external
edits are preserved: Settings reports an error until the service is restarted.
A missing file uses
the defaults above. Invalid or unreadable configuration prevents startup; it is
not silently replaced. Supported limits are 1–1000 entries, 1 byte–4 MiB per
entry, up to 64 MiB total (at least one maximum-sized entry), and 1 second–30
days. The first advertised Wayland seat is selected unless `seat` names another.

History stays in memory and disappears when the service exits. Copied text is
never logged or written to disk. The newest duplicate replaces its older entry
with a new ID and timestamp. Entry count, text bytes, and monotonic age bound
the saved history. The current selection is kept independently until replaced,
even when its history entry expires or is deleted. Transfer buffers and queues
are also bounded: one incoming transfer, up to 16 outgoing transfers, a 32-command
queue and three-second transfer deadlines.

## Wayland support

The backend uses `zwlr_data_control_manager_v1`, supported by Blair. It receives
selection events without polling clipboard contents. It captures UTF-8 plain
text, ignores primary selection, non-text selections, oversized entries and
selections advertising `x-kde-passwordManagerHint`. Password hints are cooperative;
applications that copy unmarked secrets can still place them in history.

For plain-text-only offers the service keeps ownership so text remains available
after the copying application exits. Mixed offers, such as text plus HTML, keep
their original owner and formats; only their text representation enters history.
Explicit restoration publishes plain text. New selections cancel unfinished
reads, preventing an old transfer from becoming the displayed current item.
Compositors without wlr-data-control cannot run this backend.

## Permissions and identity

Content methods consult Coconut's existing permission portal on every call,
without caching grants or allowing access when the portal is unavailable:

| Permission | Access |
| --- | --- |
| `clipboard-read` | Read the current text selection |
| `clipboard-history` | List/read history, remove entries and clear history |
| `clipboard-write` | Replace the current clipboard |

Restore requires both history and write permission. Each prompt supports Deny,
Allow once and Always allow. Saved decisions share
`$XDG_DATA_HOME/coconut/permissions.json` with screen-capture decisions. Reset in
Settings removes a saved decision; the next request prompts again, except for
the installed shell/Settings defaults.

Only the `coconut-clipboard` executable beside the permission portal may delegate
clipboard authorization. The portal resolves the original caller's unique D-Bus
name to a PID and `/proc/<pid>/exe`, or its namespace's `.flatpak-info` application
identity. Applications cannot supply their own trusted subject. The installed
`coconut` shell and `coconut-settings` receive default access based on executable
file identity; explicit saved denials override it. Copies in other directories
are not trusted. Pending consent is bounded, coalesced, timed out and cancelled
when the client or delegating service disconnects.

Only the installed shell/Settings may configure or pause collection. Configuration
writes complete before memory changes; failures preserve running preferences.

This is Coconut's API and consent flow, not a new standard XDG Clipboard portal.
It protects access through this service. Native Wayland clipboard/data-control,
X11 clients and other independent clipboard services retain their compositor's
access policy. Enforcing consent on direct privileged Wayland access requires
compositor support. Unrestricted same-user applications also have access to
that user's configuration and permission files.

## D-Bus API

Session name `org.coconut.Clipboard`, path `/org/coconut/Clipboard1`, interface
`org.coconut.Clipboard1`. Tuple returns below are D-Bus output arguments, except
the entry tuples inside the returned array.

| Method | Result / behavior |
| --- | --- |
| `Status()` | `(available: b, enabled: b, error: s, revision: t)`; no content |
| `GetConfig()` | `(max_entries: u, max_item_bytes: u, max_total_bytes: u, max_age_seconds: t)` |
| `Current()` | `(has_text: b, text: s)` |
| `List(offset: u, limit: u)` | `a(tts)` entries: ID, Unix copy timestamp, preview; newest first |
| `Get(id: t)` | Complete saved text |
| `SetText(text: s)` | Publish nonempty text within the item limit |
| `Restore(id: t)` | Restore the exact saved text; stale IDs fail |
| `Remove(id: t)` | Whether an entry was removed |
| `Clear()` | Remove all history, preserving the current clipboard |
| `SetEnabled(enabled: b)` | Save the collection preference |
| `Configure(max_entries: u, max_item_bytes: u, max_total_bytes: u, max_age_seconds: t)` | Validate/save limits and prune immediately |
| `WaitChanged(revision: t)` | Next revision, or current revision after 25 seconds |
| `Changed(revision: t)` | Content-free revision signal |

List returns at most 50 previews per call, each at most 512 Unicode characters.
Use Get for full content and paginate with offset. IDs and revisions use a random
session seed so a restarted service does not reuse its initial tokens. Refresh
history after a service restart and keep entry IDs opaque. Status,
configuration and change notifications expose no text and need no content
permission. Applications should treat revisions as opaque change tokens.

```bash
gdbus call --session --dest org.coconut.Clipboard \
  --object-path /org/coconut/Clipboard1 \
  --method org.coconut.Clipboard1.List 0 20
```

Rust consumers can use `client::Client::session()` and its typed methods. It is a
blocking client: run calls on a worker, keep the connection for a request's
lifetime, and allow time for interactive consent.

## Validation

```bash
cargo test -p coconut-integration-clipboard -p coconut-permission-portal -p coconut-plugin-clipboard
cargo clippy -p coconut-integration-clipboard -p coconut-plugin-clipboard \
  -p coconut-permission-portal -p coconut-integration-system --all-targets --no-deps -- -D warnings
COCONUT_CLIPBOARD_SNAPSHOT=/tmp/clipboard cargo test -p coconut-plugin-clipboard
COCONUT_CLIPBOARD_SETTINGS_SNAPSHOT=/tmp/clipboard-settings \
  cargo test -p coconut-settings clipboard_preferences_and_saved_permission_controls_render
```

The opt-in session test uses a private bus, temporary configuration and a nested
Blair compositor. It never changes the parent session's clipboard. It needs a
graphical parent, Blair, `wl-copy`, `wl-paste`, `gdbus`, `dbus-daemon`, and Python's
GObject introspection bindings:

```bash
cargo build -p coconut-integration-clipboard -p coconut-permission-portal
COCONUT_CLIPBOARD_TEST_BLAIR="$PWD/../blair/target/release/blair" \
  PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -m unittest discover -s tests -p test_clipboard_session.py -v
```

Set `COCONUT_CLIPBOARD_TEST_BUILD` to a different binary directory if needed.
