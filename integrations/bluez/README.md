# BlueZ controls and pairing

The Linux Bluetooth integration uses BlueZ's D-Bus object manager and property
signals for adapters, scanning and device state. Connect pairs an unpaired device
first, then marks it trusted and connects its profiles. Separate actions handle
disconnect, removal and trust. Busy/error state is published to the shell so failed
actions can be retried without reopening the panel.

Pairing registers a `KeyboardDisplay` Agent1 on the initiating worker connection.
That connection sends Pair asynchronously and processes agent calls and replies
on one thread. It validates the BlueZ sender and requested device path, presents
confirmation/authorization/PIN/passkey/display prompts, and correlates UI answers
with the current prompt ID. Codes preserve leading zeroes; invalid entries keep
the prompt open for correction. Reject/cancel and a 90-second deadline terminate
the operation. The agent unregisters on completion and never requests default-agent
status, so unrelated system pairing flows retain their existing agent.

```bash
cargo test -p coconut-integration-bluez --lib
cargo test -p coconut-integration-bluez private_bus_pairing_handles_confirmation_rejection_and_passkeys -- --ignored
```

The second command requires `dbus-daemon` and creates its own private bus and BlueZ
fixture. It validates registration, confirmation acceptance/rejection, passkey
replies, cleanup and prompt propagation without accessing a real adapter. Other
ignored BlueZ tests require hardware and may change adapter power.

References: [BlueZ Agent API](https://bluez.readthedocs.io/en/latest/agent-api/),
[Device API](https://bluez.readthedocs.io/en/latest/device-api/).
