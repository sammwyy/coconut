# Coconut Permission Portal

`coconut-permission-portal` is Coconut's permission service. It records
per-application decisions for sensitive capabilities and shows a consent
prompt whenever an application without a saved decision requests one.

The same binary provides every entry point:

```
coconut-permission-portal --service
coconut-permission-portal --prompt <request-id>
coconut-permission-portal --list
coconut-permission-portal --set <subject> <permission> <allow|deny>
coconut-permission-portal --revoke <subject> <permission>
coconut-permission-portal --forget <subject>
```

`--service` is the session daemon. It is D-Bus activated by
`org.coconut.Permissions.service` (or its systemd user unit) and exposes two
interfaces:

- `org.coconut.Permissions1` at `/org/coconut/Permissions1` — Coconut's
  native API: `Ask`, `Decide`, `DescribeRequest`, `List`, `Revoke`,
  `SetDecision`, `Forget` plus a `Changed` signal. Callers such as Blair
  submit a subject and a capability and block until the user answers (or
  the 180 s timeout denies the request).
- `org.freedesktop.impl.portal.Access` at
  `/org/freedesktop/impl/portal/Access` — the standard Access backend so
  `xdg-desktop-portal` frontends can delegate consent dialogs to Coconut
  (`portals.conf` maps it to `coconut-permissions.portal`).

`--prompt` runs the consent dialog for one pending request; the daemon
spawns it and resolves the request with `deny` if the prompt dies without
deciding. Answers include *Deny*, *Allow once* (held in memory for that
request only) and *Always allow* when the capability supports persistence.

Decisions are keyed by *subject* — the connecting process' executable path
(`/proc/<pid>/exe`) or `flatpak:<app-id>` — and stored as JSON under
`$XDG_CONFIG_HOME/coconut/permissions.json`. `coconut-screenshots` is
preseeded with an `allow` decision so the Print Screen shortcut never
prompts. Settings → Privacy → Screen capture lists and edits the stored
decisions.

Blair consults the service according to its `integrations.screencopy`
policy: `allow` and `deny` never contact the daemon, `ask` denies when the
daemon is unreachable and `ask-safe` allows instead.

Known limitations live in the repository `TODO.md`.
