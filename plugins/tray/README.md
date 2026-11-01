# System controls

The tray plugin provides the Control center, individual status icons and the
network, Bluetooth, sound, display and power panels. Device views use the actual
negotiated popup size and preserve independent scroll positions across navigation.

## Layout and editing

The grid uses **four small columns** and grows vertically. A small cell is the
base unit: width supports 1–4 cells and height supports 1–3 independently. The
default places Wi-Fi and Bluetooth at 2×1 across the top, vertical brightness
and volume controls below, and battery/keep-awake cards alongside them.

The popup fits the visible grid vertically, grows for editing and device views,
and shrinks again on return. Tall layouts scroll within the available screen
height rather than extending beyond it.

Choose **Edit** beside the panel title or right-click a card. Drag a card to move it; drag its
bottom-right corner grip to resize it. The grip uses a diagonal resize cursor,
and an outline previews the destination. Colliding cards move downward while
other positions, including intentional gaps, stay unchanged.

The context menu provides Width, Height, Size presets, Move and Remove. Presets
include Small (1×1), Medium (2×1), Large (2×2), Tall (1×2) and Wide (4×1). **Add**
opens the catalog; controls already present are disabled. **Reset** restores the
default draft. **Done** saves, while **Cancel** or closing the popup discards the
unsaved draft. A failed save keeps the draft open with a retryable error.

Tab focuses a card. Arrow keys move it; Shift+arrows resize; Delete removes;
Enter/Space opens its menu. Removing a card changes the panel without disabling
the device or removing its bar icon.

Presentation follows the card's shape:

- Small level controls use circular dials and small battery cards use charge rings.
- Tall level controls fill upward and respond to vertical dragging and the wheel.
- Wide cards use horizontal sliders and show connection/status details.
- Larger cards include bigger values, slider endpoints or settings navigation.
- Level controls support arrows, Home and End; a dial's center does not jump its value.

## Device utilities

**Sound** has Output, Input and Applications tabs. It controls master volume/mute,
selects default playback and capture devices, adjusts each device, and controls
each application's stream volume and mute independently. Empty inventories and
command failures have visible states. PipeWire caches its inventory off the UI
thread, coalesces writes and identifies nodes by serial so a vanished stream
cannot accidentally address a reused node ID. See the
[audio backend](../../integrations/pipewire/README.md).

**Network** provides nearby/saved lists, scanning, radio control, interface/IP
information, disconnection and saved-profile removal. New WPA/WPA2/WPA3 Personal
networks prompt for a password; Open and Enhanced Open do not require one. A
connection shows progress, authentication failures and timeouts. Cancelling a
password dialog clears the field and starts no connection. Password reveal reads
saved secrets asynchronously. Enterprise/EAP and WEP setup are handed to Network
settings rather than pretending a single password can configure them.

**Bluetooth** scans, pairs/connects, disconnects, forgets devices and controls
trust for automatic connections. Its scoped BlueZ agent supports code comparison,
PIN/passkey entry and displaying codes for keyboards, with rejection/cancellation
and timeouts. The agent belongs to the initiating connection and does not replace
the system's default agent. Errors remain visible for retry. See the
[Bluetooth backend](../../integrations/bluez/README.md).

**Display** provides a brightness slider, 10/25/50/75/100% presets and keep-awake.
It reports unavailable backlights and write failures. Brightness writes retain
the latest slider value instead of spawning a process for every pointer move.

**Power** selects supported power profiles and controls keep-awake. When UPower
reports them, it also shows estimated charge/discharge time, battery health,
charge/discharge rate and cycle count. Unsupported readings are omitted.

## Configuration

Tray mode, bar visibility and battery percentage use `modules/tray.toml` in the
Coconut XDG configuration directory. `off` excludes a control from the panel and
Add catalog; `hidden` retains it in the panel while hiding its bar icon. These
tray preferences apply on a new session.

The independent `modules/control-center.toml` stores a `tiles` array. Entries have
`control`, `column`, `row`, `width` and `height`: coordinates are zero-based,
width is 1–4 and height is 1–3. IDs are `wifi`, `bluetooth`, `battery`, `keep_awake`,
`brightness` and `volume`. An empty array intentionally leaves the panel empty.
Missing/unreadable files use defaults; duplicate controls, invalid dimensions and
collisions are normalized. Existing layouts retain their cell coordinates;
Reset loads the new default arrangement.

Layout saves run in a background worker and replace the file atomically. Saved
edits survive reopening and restarting. Manual edits are loaded at shell startup.
Device operations depend on the available platform backend; unsupported services
show unavailable/empty states instead of fabricated device choices.

## Validation

```bash
cargo test -p coconut-plugin-tray -p coconut-plugin-kit -p coconut-shell --lib
cargo clippy -p coconut-plugin-tray --all-targets --no-deps -- -D warnings
COCONUT_TRAY_SNAPSHOT=/tmp/control-center cargo test -p coconut-plugin-tray --lib
```

Scenes cover light/dark themes, 380×620 and 320×480 popups, layout editing and
persistence, compact dials, vertical dragging, wheel/key input, corner hit/cursor
behavior, clipped scrolling, device navigation, audio target selection/mixing,
Wi-Fi password submit/cancel/masking, Bluetooth pairing/trust and power/display
actions. Backends include parser, command-queue and credential-validation tests;
Bluetooth additionally has a private-bus agent fixture. Physical hardware changes
require a separate native session and are not part of the ordinary test run.
