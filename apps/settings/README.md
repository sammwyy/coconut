# Settings

`coconut-settings` is Coconut's native Rust/CreamUI settings application. Its
package and executable are both named `coconut-settings`.

## Editing bars

Open Desktop → Statusbar or Desktop → Dockbar. Both pages start on Contents:

- The layout overview lists groups in their screen order. Select a group to
  inspect its items. Hidden and empty groups remain editable.
- Add group creates an empty group. Add item opens a searchable catalog with
  keyboard selection. Items may appear more than once, including in both bars.
- Drag an item's handle to place it before or after another item. The insertion
  line previews its destination; the configuration changes only on release.
- Drop an item onto another group in the overview to move it there, including
  empty or hidden groups. Drag group cards to reorder the groups themselves.
- Escape, dropping outside the editor, or losing focus cancels the drag.
  Holding near the scroll area's edge scrolls the list while dragging.
- Alt + Up/Down on an item handle reorders it; Alt + Left/Right moves it to
  another group. Alt + arrow keys on a group card reorders that group.
- Show group and Spacing are inline controls. Remove group is a direct action.
  An item's compact menu contains its settings (when available) and removal.
- Undo reverses the last successful content edit in the current Settings window.
  Removing a group can be undone, including its items and inline configuration.
- Layout controls visibility, auto-hide, screen edge, thickness, alignment,
  screen inset, end padding and group spacing. Style controls bar and
  island backgrounds/borders and whether each group shares a single surface.

Right-clicking the running bar opens a short contextual menu: bar settings on
empty space, Edit group / Remove group on group space, or Item settings / Remove
item / Bar settings on a widget. Editing a group opens its selected group in the
Contents editor. Undo last edit appears only when there is an edit to reverse.
Ordering, insertion, spacing and the catalog stay in Settings; there are no
nested contextual submenus. Menus use CreamUI's `ContextMenuHost` with a native
Wayland popup or an overlay fallback. Auto-hide waits while a menu is open.

The catalog includes clipboard history on Linux, application launching, open
windows, workspaces, clock, weather, media controls and the configured tray
widgets. Separate tray devices are offered when their tray mode/visibility makes
them available. Changes to tray mode still require a new session, as documented
on Status icons. Now playing may be absent from the running bar while no media
is playing; its placement remains visible in the editor.

Content edits read the latest configuration and save in a background task. A
changed bar snapshot prevents a stale menu from editing a different placement.
An atomic replacement preserves the previous file if writing fails. Other bar
settings and inline item overrides survive moves. Settings follows shell config
notifications, including edits made through the bar's context menu. Unsaved
content changes are never displayed as successfully applied; failures show an
error and leave Undo unchanged.

The Islands page manages shared widget settings and links to both content editors.
It does not implicitly insert items into the first group or remove every copy.

Direct routes:

```bash
cargo run -p coconut-settings -- '/desktop/statusbar?tab=contents'
cargo run -p coconut-settings -- '/desktop/dock?tab=contents'
```

## Validation

```bash
cargo check -p coconut-core -p coconut-plugin-kit -p coconut-settings -p coconut-shell
cargo test -p coconut-core -p coconut-plugin-kit -p coconut-settings -p coconut-shell
cargo clippy -p coconut-core -p coconut-plugin-kit -p coconut-settings -p coconut-shell --all-targets --no-deps -- -D warnings
```

Render the actual Settings tree in light/dark themes at 1100×760 and 640×480,
including empty/hidden groups, catalog filtering, no results, position,
appearance, and save errors:

```bash
mkdir -p /tmp/coconut-bars
COCONUT_BAR_SNAPSHOTS=/tmp/coconut-bars cargo test -p coconut-settings --lib bar_editor_scenes_and_transients
```

The tests never apply configuration to the host. Core tests cover rendered
ordering, repeated items, instance overrides, group operations and invalid
moves. Plugin-kit tests bound contextual menu size and exercise removal and Settings routes. Shell
scene tests compare primary hit geometry before/after contextual wrapping on
all four screen edges. Keyboard tests cover catalog selection and dismissal.

The drag scene tests drive the actual handle callbacks across reactive rebuilds
in light/dark and normal/compact windows. They cover item ordering, transfer to
another group, group ordering, cancellation and keyboard movement. Drag previews
are included in `COCONUT_BAR_SNAPSHOTS` output.
