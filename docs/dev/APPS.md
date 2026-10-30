# Developing Coconut apps

This guide defines the conventions for native applications in `apps/`. It is for
human contributors and coding agents, and is an acceptance guide for new apps and
substantial UI changes. Existing apps illustrate individual patterns; this guide
states which patterns to carry forward.

**Must** denotes a requirement. **Should** denotes the default unless the task or
application's workflow calls for another choice. Explain material deviations in
the app's documentation or change description. Resolve routine choices without
adding approval steps to development or product flows.

Read [CODESTYLE.md](../../CODESTYLE.md) for source rules and
[SHELL.md](SHELL.md) for checkout layout, system dependencies, and build setup.
All commands in this guide run from the Coconut repository root.

## 1. Start from the workflow

Before adding widgets, identify the primary task, the document/session/item being
acted on, the actions that modify it, and the state that must survive UI rebuilds.
List the empty, loading, error, unsaved, and unavailable states that actually apply.
Choose which controls belong in the titlebar, content, transient panels, or statusbar.

The first implementation must have usable structure and interactions, not merely
expose every state variable as a button. Build one complete workflow with loading,
editing or navigation, completion, and dismissal before expanding the feature set.

Use English for application text, documentation, identifiers, and technical
comments. Preserve user-provided names and content as entered.

### References by purpose

| Reference | Patterns to inspect |
| --- | --- |
| [CEdit](../../apps/editor/README.md) | Document state, unsaved titles, independent format/mode/encoding, byte-safe editing, bounded painting, atomic saves, preview scenes |
| [Terminal](../../apps/terminal/README.md) | Sessions and panes, configurable shortcuts, keyboard routing outside UI context, shared command palette, separate settings, live configuration |
| [File Explorer](../../apps/files/README.md) | Navigation, selection, MIME default applications, XDG file chooser portal, directory search, tabs |
| [System Monitor](../../apps/monitor/README.md) | Background telemetry, bounded histories, graphs, process identity checks, confirmed destructive actions |
| [Media Player](../../apps/media-player/README.md) | Media-specific transport UI, backend isolation, native media controls, fullscreen/background behavior, optional integrations |
| [Coconut UI](../../crates/ui/src/lib.rs) | Shared app identity and About pages using the invoking package's metadata |

Use the reference that matches the problem. A player can need an immersive
transport surface; a monitor needs information displays; a file chooser has a
selection/confirmation workflow. These are not reasons to duplicate generic menus,
palettes, dialogs, or window controls.

## 2. Package and architecture

Create an app under `apps/<name>/` with a `coconut-<name>` package, an explicit
executable name, workspace package metadata, and workspace lints. The package,
executable, and display name may differ; document all three in its README.

The workspace already includes `apps/*`. Add dependency versions and sibling paths
to the root [Cargo.toml](../../Cargo.toml), then inherit them with
`dependency.workspace = true`. Add only the dependencies and features the app uses.

A useful structure is:

```text
apps/<name>/
├── Cargo.toml
├── README.md
├── data/
│   └── <binary>.desktop
├── examples/                 # Optional developer previews
└── src/
    ├── main.rs               # CLI parsing and startup
    ├── lib.rs                # When reusable/testable modules justify it
    ├── app/                  # Coordination, commands, views, window lifecycle
    ├── appearance.rs         # System appearance and live reload
    ├── commands.rs           # Action catalog and shortcut routing
    ├── config.rs             # When preferences exist
    ├── model.rs              # Domain state; choose a descriptive actual name
    └── backends/             # When platform/engine implementations differ
```

This is a boundary map, not a requirement to create empty files. Split by
responsibility as complexity grows. Keep domain operations independent of widget
construction. Window startup should not contain the editor, filesystem browser,
process controller, or playback engine itself.

### Ownership and state

- Keep document/session state separate from window chrome and transient UI state.
  Query, selection, cursor, active tab, scroll position, and unsaved state need
  deliberate ownership and lifetime.
- Keep controllers in persistent window/app state. Rebuilding a widget tree must
  not recreate its `TextController`, `ScrollController`, palette controller, or
  selection controller and lose interaction state.
- Use signals for observable UI state and shared handles for substantial models.
  Avoid copying complete documents or datasets just to notify the renderer.
- End `RefCell` borrows before operations that can synchronously rebuild UI. Use
  `creamui_reactive::batch` for coordinated changes, and avoid publishing an
  intermediate state that a view cannot safely read.
- Make state transitions and command availability explicit. A visible control
  must perform its action; unavailable actions need a clear disabled state or a
  useful explanation.

## 3. CreamUI and platform dependencies

Coconut apps use native Rust/CreamUI. Use the root workspace's sibling CreamUI
checkout; do not introduce a separate UI toolkit or a second independent CreamUI
version for an ordinary Coconut app.

| Dependency | Responsibility |
| --- | --- |
| `creamui-core` | Widget/painter interfaces, geometry, layout, scene rendering |
| `creamui-reactive` | Signals, batching, contexts, persistent reactive state |
| `creamui-widgets` | Themed components, raw controls, and interaction controllers |
| `creamui-render` | Application/window lifecycle, native events, background completion, rendering backends |
| `creamui-theme` | Theme tokens and the current themed context |
| `creamui-theme-watch` | System appearance change notifications |
| `creamui-fonts` | Font registration, resolution, and text metrics when needed |
| `coconut-integration-cream` | Coconut's resolved system appearance and font preference |
| `coconut-ui` | Coconut-specific shared application UI, including About metadata |
| `coconut-file-menu` | Shared file/folder context menu composition and operations for apps and desktop |
| `delib` | Existing desktop/configuration facilities where relevant |

The root disables default features for `creamui-render`. Select the actual
backends and integrations deliberately. Follow a current app manifest for Linux
Wayland/Blair/KWin support and target-specific Windows support. Keep Linux-only
libraries and features in Linux target dependencies; do not imply that an app is
cross-platform merely because its widgets compile on several platforms.

For native desktop file dialogs, use the existing `rfd` integration. On Linux,
select its `xdg-portal` backend so the desktop's configured file chooser, including
CFiles when installed, can serve the request. Keep file dialogs distinct from
in-app confirmation and input dialogs.

### Appearance lifecycle

Load appearance through
`coconut_integration_cream::load_system_appearance()`, apply the returned font
preference when present, and pass its theme into the window options. Follow live
changes through `ThemeListener` and update the window's theme. Inspect
[CEdit's appearance module](../../apps/editor/src/appearance.rs) and the other app
startup modules for the current integration.

Use semantic colors, typography, spacing, radii, selection, and interaction tokens.
Keep light/dark variants and text scale usable. An application may have a domain
palette, such as terminal ANSI colors or an artwork-derived player accent, but
ordinary controls must still follow the system UI theme.

`use_theme()` requires an active CreamUI context. Native keyboard callbacks,
workers, and domain operations must not construct themed widgets to perform their
work. Keep those paths independent of render-time context. In particular,
`CommandPaletteController::handle_key` supports native routing outside that context.

## 4. Reuse shared components

Consult [CreamUI components](../../../creamui/docs/components.md) and the local
widget implementations before writing a control. Prefer themed components for
ordinary app UI. Raw widgets are appropriate for composition and specialized
interaction, provided theme, focus, keyboard, and pointer behavior remain correct.

| Need | Use |
| --- | --- |
| Application titlebar and native caption integration | `AppTitlebar` |
| Commands and searchable action menus | `CommandPalette`, `CommandPaletteController`, `CommandPaletteItem` |
| Modal surface and placement | `Dialog`, `DialogPlacement` |
| Modal title, explanation, content, footer | `DialogHeader`, `DialogMessage`, `DialogContent`, `DialogFooter` |
| Evenly sized confirmation actions | `ModalActions` with themed `Button` children |
| Ordinary actions and editable fields | `Button`, `TextInput`, `TextArea`, and their controllers |
| Choice fields and small mutually exclusive options | `Select`, `ComboBox`, `SegmentedControl`, or an appropriate compact popup |
| Transient popup surfaces | `ContextMenuHost` for context menus; `Popover`, `MenuPopup` for composed surfaces; preserve dismissal and pointer shielding |
| Scrolling and large collections | `ScrollView`, `ScrollController`, and virtual list/table components |
| Tabs and navigation | `Tabs`, `Tab`, `Sidebar`, `TreeView`, or composed titlebar tabs |
| Application identity | `coconut_ui::about!` when an About page is needed |

A document editor, terminal screen, graph, or video surface can require custom
painting. That does not require a custom implementation of every surrounding
button or dialog. Check whether a standard text area meets the workload before
building an editing engine; justify a custom viewport with actual requirements.

If a common interaction is missing or inadequate, improve CreamUI's themed
components and migrate consumers. Keep Coconut-specific branding in `coconut-ui`
and domain behavior in the app. A reusable dialog or palette does not need a new
crate simply to share its presentation.

## 5. Window structure and visual hierarchy

### Visual language

Use the theme's normal UI font for chrome, forms, menus, and settings. Use a
monospaced font where the content requires character alignment, such as source
text or byte grids. Keep font metrics, zoom, text scale, and hit testing consistent.

Use a small, deliberate spacing hierarchy. Compact does not mean reducing hit
areas until controls are hard to use; it means removing redundant wrappers,
repeated text, oversized padding, and empty reserved panels. Group related actions
and expose one clear primary action in a decision dialog.

Prefer CreamUI icons or the app's established icon source over emoji or unrelated
symbol styles. Icon-only controls need a discoverable name through their matching
menu/palette action or other supported affordance. Keep body copy and field labels
left-aligned; centered identity blocks, artwork, or empty states are separate
compositions rather than the default for every screen.

### Titlebar and tabs

Use `AppTitlebar` to negotiate decorations and reserve compositor/client control
regions. Do not draw a second set of minimize/maximize/close buttons or assume
controls always live at the right edge.

- With one document/session, show its title and a compact new-tab action when the
  app supports tabs. Avoid a redundant one-tab strip.
- With multiple documents/sessions, put compact, closable tabs and the adjacent
  new-tab action in the titlebar where practical. Keep the active tab visible.
- Honor existing user-configurable tab placement and navigation preferences.
  The default composition is not a reason to silently remove a supported layout.
- Put secondary actions in a compact menu, rather than filling the titlebar with
  separate Commands/Open/Save/Settings buttons.
- Mirror or reposition actions when the compositor puts controls on the leading
  side. Use `reserved()`, `controls_lead()`, `center_margin()`, and the titlebar's
  slots/body for the appropriate composition.
- Keep controls clear of drag and resize regions. Empty titlebar space should
  remain draggable, and long titles/tabs must not overlap caption controls.

Titles should identify the current file, location, session, or app. Document apps
should update the native title and expose unsaved state, for example
`notes.md * - CEdit`, using `WindowHandle::set_title`. Cosmetic view changes should
not falsely mark content as edited; changes to saved representation must be tracked.

### Content and status

Give the primary content most of the window. Use one compact statusbar when one
is useful; avoid a second bottom strip for permanent "Ready" text and shortcut
advertising. Keep persistent instructions in menus, help, or the palette.

Align position and count text to the left. Put relevant selectors and secondary
status controls in stable positions. If a displayed value represents a user
choice, offer an appropriate control there rather than duplicating it in another
bar. Actual telemetry and other informational values may remain read-only.

Do not add controls, remove controls, or change their widths in a way that moves
the trigger under the pointer. Reserve selector slots or otherwise anchor their
positions across state changes. A shorter label is not a reason to resize a
control in the middle of an interaction.

### Independent concepts need independent controls

Separate the type of content, its presentation, and its saved representation.
CEdit provides a useful concrete example:

| Control | Values and behavior |
| --- | --- |
| Format | `Plain`, `Markdown`, `Bytes` |
| Mode | Markdown: `Raw`, `Hybrid`, `Preview`; Bytes: `HEX`, `Binary`; no extra choice for Plain |
| Encoding | `UTF-8`, `ASCII`, `UTF-8 BOM`, `UTF-16 LE`, `UTF-16 BE`; applicable to text |

Markdown can use ASCII or UTF-16. HEX/Binary are byte editing modes and belong
under Bytes, not as duplicate choices in the text encoding menu. Byte views save
actual bytes. Hide inapplicable encoding/line-ending controls while preserving
layout stability. Explain genuinely coupled operations and preserve data when
switching representations; never silently replace undecodable content.

Apply the same reasoning elsewhere: a monitor's view/grouping is separate from
sort order; a player's playback mode is separate from its selected audio track.
Use labels that name the choice rather than expose internal implementation flags.

## 6. Dialogs and palettes

### Dialogs

Build dialogs with CreamUI's shared sections. Use `Dialog::custom` for forms and
other custom content; `Dialog::new` or `AlertDialog` can cover simple messages.
Keep visibility and the pending action in app state.

- Header: one clear title, with an icon or short description only when useful.
- Content: the information or inputs needed to make the decision, left-aligned.
- Footer: semantic actions with `ModalActions` distributing their available width
  and maintaining small gaps. Use appropriate `ButtonVariant` values.
- Use the sections' `compact()` variants for short prompts. Content should
  determine height; do not reserve a large empty card for a one-sentence question.
- Avoid repeating the same unsaved/error statement in an eyebrow, title,
  description, file-name block, and body. Identify the object and decision once.
- A position/navigation input can use left/top-left placement. A blocking
  confirmation can be centered. Choose placement for the workflow rather than
  centering every element, label, and input automatically.
- Shield background pointer interaction and focus appropriately. Route Escape
  through the same cancellation callback; keep Enter and the focused field's
  submit action consistent. Explicit action labels should explain the outcome.
- Cancel and backdrop dismissal must not accidentally confirm a destructive
  action. Completion, cancellation, and failure must leave the app usable.

Keep native open/save choosers in the desktop portal path. In-app forms and
confirmations belong to the shared dialog components.

### Command palettes

Use `CommandPalette` rather than maintaining a local search field plus a hand-built
list of padded, centered command strings. CEdit and Terminal use the same component.
Each application supplies command metadata and callbacks:

```rust
use creamui_core::{BoxedWidget, Size};
use creamui_widgets::{CommandPalette, CommandPaletteController, CommandPaletteItem};

fn build_palette(
    controller: &CommandPaletteController,
    size: Size,
    open_file: impl Fn() + 'static,
    dismiss: impl Fn() + 'static,
) -> BoxedWidget {
    let items = vec![
        CommandPaletteItem::new("open", "Open file", open_file)
            .category("File")
            .shortcut("Ctrl+O"),
    ];
    Box::new(CommandPalette::new(controller, items, dismiss).viewport(size))
}
```

Keep the controller in persistent app/window state. Reset it when opening a fresh
palette and focus the input after the surface becomes visible. Supply actual
configured shortcut labels, stable command IDs, and callbacks that dispatch the
same actions as menus and key bindings.

The shared component owns filtering, selected result, scrolling, result rows,
shortcut badges, and height adjustment. Few results should produce a short palette;
zero results need a small useful empty state. It supports arrows, Enter, Escape,
and a shortcut-reference presentation. Do not add a separate oversized heading
above a second search header or reserve unused result height in an app wrapper.

File Explorer's directory search is a file filter, not an existing command palette.
Do not replace a domain search with a command menu just to reuse a similar shape.

## 7. Keyboard, pointer, and focus

Maintain one action catalog/dispatch path for the app. Menus, palette entries,
toolbar controls, and shortcuts must invoke the same operations. A palette must
show the current binding when bindings are configurable.

- Make the primary workflow usable without the mouse. Provide meaningful bindings
  for frequent actions and a discoverable path to secondary actions.
- Respect focused fields. Typing, selection, copy/paste, Home/End, and word
  navigation in an input must not also execute document or global commands.
- Handle pressed/released keys and modifiers deliberately. Native event handlers
  run before widget dispatch; consume an event only when handled, so the same key
  cannot execute twice.
- Apps such as Terminal intentionally intercept native keys. Forward palette
  input through its shared controller and preserve PTY input routing when the
  palette is closed. Do not construct themed inputs from a native callback.
- Opening a transient input must focus it. Closing it must restore the appropriate
  document/session focus. A newly opened/switched tab should be ready for its
  primary interaction.
- Provide visible hover, pressed, focus, selected, and disabled states where
  applicable. Keyboard focus must remain distinguishable from pointer hover.
- Keep pointer hit areas, drag selection, scrollbars, and painted positions in
  agreement, including zoom and styled text. Styling must not make the cursor or
  selection target a different character than the one under the pointer.
- Check Escape, Enter, Tab, Shift+Tab, and selection/clipboard behavior inside
  dialogs and palettes. Test native routing outside the rendering context.

## 8. Data integrity and asynchronous work

Blocking work must not run in widget builders, painters, or ordinary UI callbacks.
Use `AppHandle::spawn_background` or the app's established worker/event model for
I/O, decoding, telemetry, expensive searches, and substantial conversions. Apply
results to signals and UI-owned state on the UI thread.

Tag asynchronous work with the identity/revision/query it belongs to. Discard
obsolete completions rather than applying an old result to a new tab, query, or
selection. Bound queued work; typing must not spawn an unlimited backlog of full
searches. Prefer desktop/native events to polling when available; justify polling
when the underlying interface has no notification mechanism.

For document/configuration writes, use appropriate atomic write and validation
patterns. Preserve the representation and content the user selected. Detect
external changes where overwriting could lose another version, and expose errors
with useful context.

Use `WindowHandle::on_close_requested` when unsaved work needs a deferred close.
Handle Save/Discard/Cancel, chooser cancellation, save failure, tab close, and native
window close consistently. Returning to the editor after cancelling must work.
Keep undo/redo, saved revision, and unsaved markers consistent after conversion,
save completion, and branched history.

Validate the identity of destructive targets. System Monitor's PID-reuse check is
an example: a stale displayed identifier must not act on a different live object.
Use confirmations for actual destructive decisions, not for routine navigation or
reversible mode switches. Errors must not be swallowed or leave a silent no-op.

## 9. Rendering and performance

Choose data structures and rendering boundaries for the supported workload.

- Paint visible rows and bounded horizontal slices. Virtualize large lists/tables.
  A million-character line must not be fully shaped just to show one viewport.
- Keep snapshots/history efficient and bounded. Avoid full-buffer clones for
  typing, cursor motion, scroll, or unchanged status updates.
- Reuse text metrics, prepared data, and controllers. Keep layout/paint free from
  filesystem scans, process creation, background task creation, and repeated full
  parsing of unchanged input.
- Keep telemetry sampling, history storage, and draw frequency bounded. Request
  animation frames only while an animation or media workflow needs them.
- Measure both many-row and very-wide-row cases, plus Unicode and realistic
  selections. State memory costs and known limits instead of claiming "fast"
  based only on opening a small fixture.

A custom widget must implement the CreamUI interaction hooks its workflow needs,
including focus registration/key handling, text-input acceptance, selection,
layout reporting, scrolling, and pointer hit behavior. Test its scene, not just
its data model. A focus ID alone does not prove a viewport is visible or usable.

## 10. Configuration and desktop integration

Use the existing `delib`/XDG configuration pattern where preferences are needed.
Validate configuration before applying it, preserve a usable configuration on
failure, and document defaults, supported overrides, and when changes take effect.
Changes advertised as live must affect running windows without replacing their
active documents/sessions.

Add `.desktop` metadata and suitable file/URI handling for the app. Respect existing
MIME default applications, file chooser portal policy, and platform capabilities.
Add D-Bus, media, tray, or single-instance integration only when the app needs it.
Document platform support and optional services honestly.

For an installable executable, update:

1. Its app manifest, data assets, and README.
2. The package/binary list in [build.sh](../../build.sh).
3. The executable and desktop-entry installation in [install.sh](../../install.sh).
4. Affected installation fixtures, currently including
   [the CFiles installer test](../../apps/files/src/installation_tests.rs).
5. The [root application list](../../README.md) and relevant developer commands.

## 11. Validation and review

Run checks appropriate to the changed package and any shared component consumers:

```bash
cargo fmt --check
cargo check -p coconut-editor
cargo test -p coconut-editor
cargo clippy -p coconut-editor --all-targets --no-deps -- -D warnings
```

Substitute the actual package. Shared CreamUI changes need their own focused tests
and integration checks in affected apps. `--no-deps` keeps the app's Clippy check
focused on that package; it does not validate dependency lint cleanliness. Record
pre-existing/environmental failures separately from new failures.

Inspect actual rendered scenes before declaring UI complete. Use `SceneRecorder`
and `Rasterizer` or an appropriate preview window. A blank snapshot or an invisible
focus target is a failure, even if the rendering call returned successfully.

Check at least a normal window and a compact window, such as 1100×760 and 640×480,
in light and dark themes. Include long names, empty/filtered results, and the
workflow's error or confirmation state. Verify compositor controls on both sides,
client decorations, keyboard focus, and transient dismissal when relevant.

For layout stability, compare the trigger's actual hit bounds across modes rather
than comparing glyph positions: different labels can have slightly different
pixel rounding inside a stationary control. For palettes, verify result filtering,
height reduction, selection reset, scrolling, callbacks, and native key routing.
For large-file claims, verify that painting work stays proportional to the viewport.

The [CEdit preview example](../../apps/editor/examples/preview.rs) exercises these
cases. Terminal and other apps also have package-specific scene/fixture tests;
consult their READMEs for snapshot environment variables.

### Completion criteria

- [ ] The primary workflow and applicable empty/loading/error/unsaved states work.
- [ ] Generic UI uses shared CreamUI components and semantic theme tokens.
- [ ] Titlebar controls, tabs, and drag regions respect decoration ownership.
- [ ] Status and selector layout is compact, left-aligned where appropriate, and stable.
- [ ] Format, presentation, and saved representation are not conflated.
- [ ] Dialogs and palettes have purposeful content, compact geometry, and correct actions.
- [ ] Keyboard, pointer, focus, clipboard, and dismissal paths are usable.
- [ ] Expensive work is off the UI thread, bounded, and protected from stale results.
- [ ] Data integrity and native close behavior match the app's workflow.
- [ ] Relevant checks pass and rendered output has been inspected.
- [ ] README, packaging, and shared docs reflect the actual behavior.
- [ ] Remaining limitations are recorded in the root [TODO.md](../../TODO.md).

Keep comments short and technical, code free of dead scaffolding, and commits
focused on one component or feature. Improvements shared by several apps belong
in CreamUI first; app-specific integration can then remain small and consistent.
