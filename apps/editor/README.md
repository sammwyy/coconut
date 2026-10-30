# CEdit

Coconut's native CreamUI editor for text, code, Markdown and raw bytes.
Package: `coconut-editor`. Executable: `cedit`.

```sh
cargo run -p coconut-editor -- README.md
cedit file.txt notes.md
cedit --help
```

The application follows Coconut's system appearance and live theme changes.
Windows render through CreamUI's CPU backend; `CUI_OVERRIDE_RENDER_BACKEND=gpu`
opts into `wgpu` per launch. `build.sh` stages the executable; `install.sh`
installs it and its desktop entry. All application text is English.

## Editing

- Multiple documents, with tabs inside the titlebar when more than one is open.
- File name and unsaved marker in the native window title: `notes.md * - CEdit`.
- A hamburger menu, searchable command palette and searchable shortcut reference.
- Selection by dragging, word and document navigation, horizontal and vertical
  scrolling, line numbers, insert/overwrite, indentation and automatic indentation.
- Duplicate, delete and move lines; Markdown bold, italic and inline code commands.
- Grouped typing undo, redo, independent histories and saved-state tracking.
- Clipboard integration, drag-and-drop opening and multiple file arguments.
- Literal and regex search, case and whole-word options, navigation, replacement
  and capture expansion. HEX and Binary search and replacement accept byte values.
- Go to `line:column`, decimal byte offsets or hexadecimal offsets such as `0x100`.
- Atomic saves, external modification detection and Save/Discard/Cancel on close.

## Render and encoding

The single statusbar separates three controls:

- **Format:** Plain, Markdown or Bytes.
- **Mode:** Raw, Hybrid or Preview for Markdown; HEX or Binary for Bytes. Plain
  text does not need a mode selector. Hybrid keeps Markdown editable while
  enlarging headings and formatting inline content. Preview is read-only and
  keeps source line alignment.
- **Encoding:** ASCII, UTF-8, UTF-8 with BOM, UTF-16 LE and UTF-16 BE. Encoding
  conversion is undoable and changes the bytes written on the next save.

For example, Markdown Hybrid can use ASCII or UTF-16 without changing its format.
Bytes uses HEX/Binary for both display and editing of the file's actual bytes.
Saving preserves those bytes, including invalid Unicode and NUL bytes. Enter two
hexadecimal digits or eight binary digits to replace a byte; paste complete byte
values to insert several. Raw byte views use rows of sixteen bytes.

Markdown supports headings, emphasis, strong text, inline and fenced code, lists,
tasks, block quotes, links and strikethrough. Images are represented by their alt
text; source HTML is not a web view. Code syntax highlighting is not included.

## Keyboard

| Shortcut | Action |
| --- | --- |
| Ctrl+N / Ctrl+O | New tab / Open |
| Ctrl+S / Ctrl+Shift+S | Save / Save as |
| Ctrl+W / Ctrl+Q | Close tab / Quit |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / Previous tab |
| Ctrl+Z / Ctrl+Shift+Z | Undo / Redo |
| Ctrl+F / Ctrl+H | Find / Replace |
| F3 / Shift+F3 | Next / Previous match |
| Ctrl+G | Go to position |
| Ctrl+Shift+P / F1 / F10 | Commands / Shortcuts / Menu |
| Ctrl+Alt+0 | Plain text |
| Ctrl+Alt+1 / 2 / 3 | Markdown Raw / Preview / Hybrid |
| Ctrl+Alt+U / A / H / B | UTF-8 / ASCII / HEX / Binary |
| Ctrl+= / Ctrl+- / Ctrl+0 | Zoom in / out / reset |

F1 lists every command and binding. Shift extends navigation selections. Escape
closes transient panels; dropdowns and the command palette support arrows and Enter.

The command palette and shortcut reference use CreamUI's shared `CommandPalette` and
`CommandPaletteController`, also used by Coconut Terminal. Filtered palettes resize to their
results instead of reserving an empty result area.

Dialogs use CreamUI's shared `DialogHeader`, `DialogContent`, `DialogFooter` and
`ModalActions` components. Position entry uses left placement, and confirmation
actions share the available footer width.

## Large files

Ropey's shared, indexed rope stores document content and undo snapshots. The editor
extracts and paints only visible rows and bounded horizontal slices, including
when a single line contains a million characters. File I/O, encoding conversion,
search, replacement and Markdown block indexing run in background workers. Search
keeps one worker and supersedes obsolete requests.

Tests cover a million lines, a million-character line, viewport-bounded painting,
Unicode/byte round trips, encoding separation, undo, CRLF and file conflicts.
Regex and case-insensitive searches materialize a contiguous snapshot in their
worker; this requires additional memory proportional to file size. Histories are
bounded to 512 transactions.

```sh
cargo test -p coconut-editor
cargo clippy -p coconut-editor --all-targets --no-deps -- -D warnings
cargo run -p coconut-editor --example preview -- /tmp/cedit-previews
```

The preview example needs a desktop session but opens no window. It renders light
and dark scenes at two sizes, checks titlebar control exclusion on both sides and
captures menus, dialogs and independent Markdown/ASCII controls.
