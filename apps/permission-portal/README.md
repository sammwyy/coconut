# Coconut Permission Portal

`coconut-permission-portal` is Coconut's Linux session permission service and
consent dialog. The package and executable share that name.

```bash
coconut-permission-portal --service
coconut-permission-portal --list
coconut-permission-portal --set <subject> <permission> <allow|deny>
coconut-permission-portal --revoke <subject> <permission>
coconut-permission-portal --forget <subject>
```

The service is activated through `org.coconut.Permissions.service` or its
systemd user unit. It exposes:

- `org.coconut.Permissions1` at `/org/coconut/Permissions1`: `Ask`,
  `BeginRequest`, `WaitRequest`, `CancelRequest`, `Decide`, `DescribeRequest`,
  `List`, `Revoke`, `SetDecision`, `Forget`, `Manage`, and the `Changed` signal.
- `org.freedesktop.impl.portal.Access` at
  `/org/freedesktop/portal/desktop`: the XDG Access backend selected by
  `coconut-permissions.portal` and `coconut-portals.conf`.

## Consent and caller identity

Only the D-Bus connection owning `org.blair.Compositor` may begin a capture
permission request on behalf of a Wayland or D-Bus client. Blair supplies the
executable path from
`/proc/<pid>/exe` or the verified `flatpak:<app-id>` subject. The service
currently accepts `screen-capture` requests.

`BeginRequest(subject, permission)` returns a request ID immediately, allowing
the compositor to cancel it even while the consent dialog is open.
`WaitRequest(id)` returns `allow` or `deny`; `CancelRequest(id)` cancels only
that caller's request. A cancelled waiter is removed from its shared prompt,
and the final cancellation closes the prompt and reaps its process. The
service also cancels native requests when their D-Bus owner disconnects.
At most 256 native requests may be outstanding. The original blocking `Ask`
method remains available for older compositor clients.

An unknown decision opens a separate `--prompt <request-id>` process.
Only that child's PID may describe or decide its request; manually starting
another prompt or guessing a request ID grants no authority. Calls remain
concurrent while the user decides. Requests for the same subject and
capability share one dialog. Closing or losing the dialog, cancellation,
and the 180-second timeout deny outstanding requests and reap the child.

The prompt offers **Deny**, **Allow once**, and **Always allow** for screen
capture. Remembered decisions are saved before capture is authorized. A save
failure keeps the prompt open with an error, so the user can retry, allow
once, or deny. D-Bus writes run outside the UI thread.

Only the installed `coconut-settings` executable beside the service may
modify decisions directly. The service checks the caller's actual executable
file identity, not its reported name. CLI `--set`, `--revoke`, and `--forget`
use `Manage` and require a separate confirmation dialog for each change.
Cancellation or a write failure returns a nonzero CLI exit status.

The Access backend accepts requests only from the connection owning
`org.freedesktop.portal.Desktop`. It returns the numeric response and results
as the `AccessDialog` method reply, supports boolean and enumerated choices,
and exports `org.freedesktop.impl.portal.Request.Close` at the supplied handle
for the duration of the dialog. Only that request's frontend connection may
cancel it. Access decisions are not persisted by this backend.

## Storage and installation

Decisions are stored in `$XDG_DATA_HOME/coconut/permissions.json`, defaulting
to `~/.local/share/coconut/permissions.json`. Writes use a private temporary
file in the same directory and atomic replacement. Failed writes preserve
the previous file and in-memory decisions and return an error through D-Bus.
An unreadable or malformed existing store prevents service startup rather
than discarding the saved decisions.

Only the canonical path of `coconut-screenshots` beside the service receives
the default screen-capture grant. A binary with that name in another directory
is not trusted. An explicit saved denial overrides the default grant.
Settings → Privacy → Screen capture lists and edits saved screen-capture
decisions; revoking a stored decision restores the normal prompt or installed
screenshot-tool default.

These checks enforce the session's consent flow. Applications with unrestricted
access to the user's files can still modify that user's permission database;
this service does not provide process isolation.

Blair's `integrations.screencopy` policy controls enforcement: `allow` and
`deny` bypass the service, `ask` denies when it is unreachable, and `ask-safe`
allows instead. This policy covers Blair's `wlr-screencopy` capture path and
its D-Bus `Screenshot` method. Install the current portal before updating Blair
so its cancellable native protocol is available.

## Validation

```bash
cargo test -p coconut-permission-portal
cargo clippy -p coconut-permission-portal --all-targets --no-deps -- -D warnings
PERMISSION_PORTAL_PREVIEW_DIR=/tmp/coconut-permission-previews \
  cargo test -p coconut-permission-portal prompt::tests
```

The D-Bus tests start private `dbus-daemon` instances and separate prompt
processes. They exercise caller authentication, concurrent calls, stored and
one-time decisions, coalescing, cancellation, timeouts, storage failures, and
standard Access replies and choices, native request ownership, shared-prompt
cancellation, and compositor disconnection. Scene tests render the dialog in
light and dark themes at normal and compact sizes.

Other known limitations are tracked in the root `TODO.md`.
