# PipeWire audio controls

The Linux volume integration uses PipeWire registry, node and metadata listeners
for inventory/volume changes. A background worker reads `pw-dump -N` and obtains
effective levels/mute from `wpctl get-volume`, including hardware-backed controls.
The UI only reads cached devices and streams.

`VolumeIntegration` exposes playback/capture devices, default-device selection,
master/device/stream gain and mute. Public IDs are PipeWire object serials. Before
executing a queued command, the worker resolves that serial against a fresh
inventory; a disappeared node produces an error rather than addressing an ID
reused by another application. `wpctl set-default`, `set-volume` and `set-mute`
perform writes. Default selection follows WirePlumber's policy and persistence.

A single bounded queue coalesces pending gain/mute writes per target and default
selection per direction. Native notifications request inventory refreshes; there
is no periodic inventory polling. Command errors are cached and surfaced in the
Sound panel. If inventory refresh is unavailable, master volume/mute can still
use WirePlumber's default-sink identifier.

Runtime requirements are PipeWire, WirePlumber's `wpctl`, PipeWire's `pw-dump`,
and coreutils `timeout`. Capture/playback streams appear while applications have
registered them with PipeWire; this is not a process list. Each stream is a
separate mixer channel. Routing individual streams to specific devices is outside
this interface.

```bash
cargo test -p coconut-integration-pipewire --lib
cargo test -p coconut-integration-pipewire reads_live_inventory_without_changing_audio -- --ignored
```

The explicit live inventory test only reads the running session. Other ignored
native tests change volume and should only run in a suitable test session.

Protocol/CLI reference: [WirePlumber wpctl](https://pipewire.pages.freedesktop.org/wireplumber/man/wpctl.html).
