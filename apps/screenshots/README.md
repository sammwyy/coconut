# Coconut Screenshots

Coconut's interactive screenshot and annotation tool. Package and binary:
`coconut-screenshots`.

```bash
cargo run -p coconut-screenshots            # capture the screen, open the editor
cargo run -p coconut-screenshots -- --screen       # save the full screen, no UI
cargo run -p coconut-screenshots -- --screen --save-as  # pick where to save
coconut-screenshots --help
```

Launching with no arguments captures the whole screen and opens a maximized,
undecorated editor with the desktop dimmed behind it. Works on Wayland
(`wlr-screencopy` + `xdg-output`) and X11 (root window image) — the app picks
the backend from `WAYLAND_DISPLAY`/`DISPLAY` and stays independent of the
running compositor.

## Selecting

- Left-drag on the dimmed screen draws a selection rectangle; a tiny label
  shows its pixel size.
- Once a selection exists (Select tool): drag inside it to move, drag the
  eight handles to resize. Right-drag anywhere also moves the selection.
- `Ctrl+A` selects the entire screen. Clicking outside starts a new
  selection; a sub-3px drag cancels it.

## Annotating

A floating HUD docks below the selection (above when there is no room,
bottom-center when nothing is selected) with:

- **Select, Brush, Blur, Eraser** tools.
- A six-color palette and three brush sizes.
- **Copy** (`Ctrl+C`), **Save** (`Ctrl+S`), **Save as** (`Ctrl+Shift+S`),
  and **Close** (`Escape`).

Brush and blur paint onto a separate annotation layer: the eraser only
clears that layer, so erasing can never damage the captured image itself.
Blur dabs composite a cached box-blurred copy of the screen into the
annotation layer, which the eraser also removes.

## Output

- `Ctrl+C` copies the selection (or the whole screen when nothing is
  selected) to the clipboard and quits.
- `Ctrl+S` writes it to `~/Pictures/Screenshots/` — or the localized XDG
  pictures directory — as `Screenshot from YYYY-MM-DD HH-MM-SS.png`.
- `Ctrl+Shift+S` and `--save-as` open the platform save dialog instead.
- `--screen` skips the UI entirely and writes the same timestamped file.

## Keybinds

Coconut ships three shell shortcuts, editable under Settings → Keyboard:

| Shortcut | Action |
| --- | --- |
| `Print` | Take screenshot (opens the editor) |
| `Ctrl+Print` | Capture screen (`--screen`) |
| `Ctrl+Shift+Print` | Capture and save as (`--screen --save-as`) |
