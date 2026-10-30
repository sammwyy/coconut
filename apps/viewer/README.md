# Image Viewer

A minimal, read-only image viewer using CreamUI's image renderer. Package:
`coconut-image-viewer`; binary: `cviewer`; display name: `Image Viewer`.

```bash
cargo run -p coconut-image-viewer -- path/to/photo.jpg
cviewer photo.jpg
cviewer first.png second.webp
cviewer --help
```

Open files from the titlebar, command palette, desktop file association or by
dropping them into the window. A single file discovers supported images in its
folder, sorted by filename. Multiple explicit files keep their supplied order.
Browsing is bounded to 10000 images and does not change existing MIME defaults.

The default window is 960×680, with a 640×480 minimum. The system theme, preferred
font and live appearance changes are honored. The image occupies the main canvas;
**Adjust** and **Info** reveal an optional inspector. AppTitlebar owns caption
integration, including compositor controls on either side. The native title uses
`filename — Image Viewer`, or `Image Viewer` when no image is loaded.

## Viewing

- Scroll over the image to zoom toward the pointer; `+` / `−` zoom around the
  canvas center. Zoom ranges from 1% to 3200%.
- **Fit** / `0` fits the entire image without enlarging small images. **1:1** / `1`
  displays one image pixel per logical window pixel.
- Drag to pan an enlarged image. Panning is clamped to its bounds; smaller axes
  stay centered. WASD pans with the keyboard.
- When an image extends beyond the canvas, a small overview appears in the lower
  right. Its accent rectangle marks the visible area. Click or drag the overview
  to move to another part of the image.
- **Rotate** / `R` rotates 90° clockwise; Shift+R rotates counterclockwise. EXIF
  orientation is applied automatically on load, including mirrored orientations.
- Previous/next controls browse the current selection or discovered folder.

The footer keeps navigation, zoom and rotation in one compact row. It does not
reserve an extra status area. Open, loading, decode failure and empty states have
usable controls; a failed file can be retried or skipped with navigation.

## Temporary adjustments

The inspector offers brightness, contrast and saturation from 0–200%, plus
**Black and white** and **Invert colors**. **Reset adjustments** restores colors;
Ctrl+0 resets colors, rotation, zoom and pan together. Adjusting saturation while
black and white is selected returns to saturation control.

These are renderer properties. They do not replace the image's pixels, re-encode
it, create edited files or mark it as unsaved. The GPU applies a color matrix and
rotated texture coordinates while reusing the same image texture. The software
renderer applies the equivalent matrix after sampling, with temporary storage
bounded by the visible image area. The minimap reuses the original image too.

Each newly loaded image starts fitted with default adjustments. An open inspector
stays open while browsing. The application contains no save or overwrite action.

## Information and metadata

Info displays filename, displayed/source dimensions, format, source color type,
file size, modification time, full path, applied EXIF orientation and embedded ICC
profile size. It also lists up to 128 EXIF fields exposed by the decoder, including
camera and exposure information when present. Long field values are bounded and
the inspector scrolls. Absent EXIF data is shown explicitly; malformed optional
metadata produces a notice while usable image pixels remain viewable.

Supported raster formats are PNG, JPEG, WebP, GIF, BMP, TIFF and ICO. Animated
GIF/WebP and multipage TIFF currently show the first frame/page. SVG, RAW and
HEIC are not supported. Images are displayed as RGBA8; embedded ICC profiles are
reported but are not applied as color-management transforms. These limitations
are tracked in the root TODO where applicable.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| Ctrl+O | Open images through the native file chooser |
| Ctrl+W | Close the image and return to the empty viewer |
| Left / Right | Previous / next image |
| Page Up / Page Down | Previous / next image, including while adjusting sliders |
| Home / End | First / last image |
| `+` / `−` | Zoom in / out |
| `0` / `1` | Fit / actual size |
| WASD | Pan |
| R / Shift+R | Rotate clockwise / counterclockwise |
| E / I | Toggle adjustments / information |
| G / N | Toggle black and white / inverted colors |
| Ctrl+0 | Reset the complete view |
| F1 or Ctrl+Shift+P | Search commands and shortcut labels |
| Escape | Dismiss commands, otherwise close the inspector |

Tab/Shift+Tab and Space activate normal controls. While the adjustments inspector
is open, Left/Right reach the focused slider; Page Up/Down continue to browse.
Command search owns typing, selection, clipboard shortcuts, arrows and dismissal.
Primary view shortcuts also work when the canvas itself has no text-input focus.

## Architecture and resource limits

`asset.rs` reads, decodes, orients and inspects images on an AppHandle background
job. There is at most one active image job and one replaceable pending request.
Generation checks discard old successes and failures, including after closing an
image. A cancelled file chooser preserves the current view. Directory discovery
also runs in the worker. Image data is shared through Arc handles across rebuilds.

`view.rs` owns geometry and adjustment math independently of widgets. The custom
canvas uses CreamUI image drawing, clipping and native scene drag/wheel handlers.
Ordinary chrome, sliders, checkboxes, scrolling and command search use shared
CreamUI components. The painter never reads files or modifies source pixels.

Input limits: 128 MiB encoded files, 80 megapixels, 32768 pixels per axis, a
512 MiB decoder allocation budget and 4 MiB of EXIF bytes. A decoded RGBA8 image
can occupy about 305 MiB at the pixel limit, with additional temporary decoder
storage. EXIF output is limited to 128 fields with 512 characters per stored value.
The application does not promise tiled gigapixel viewing.

Linux uses Wayland/X11 and the XDG portal file chooser, with optional Blair/KWin
window integration. Windows backend dependencies are selected separately;
Windows runtime behavior has not been validated.

## Validation

```bash
cargo fmt --check
cargo check -p coconut-image-viewer
cargo test -p coconut-image-viewer
cargo clippy -p coconut-image-viewer --all-targets --no-deps -- -D warnings
cargo test -p coconut-files installation_tests
CVIEWER_PREVIEW_DIR=/tmp/cviewer-previews cargo test -p coconut-image-viewer
```

Tests cover anchor zoom, bounded panning, minimap navigation, rotation, alpha,
source preservation, EXIF orientation and fields, malformed files, queued/stale
loads, native keyboard routing, actual scene pointer handlers, inspectors and
caption ownership. Scene captures include light/dark, 960×680, 640×480 and
1100×760 windows, with fit/zoom/rotation, adjustments, information, empty, loading,
error and command states. The environment variable writes PNGs with SceneRecorder
and Rasterizer; no display server is required for these tests.

The sibling CreamUI renderer changes require their focused checks:

```bash
cargo test --manifest-path ../creamui/Cargo.toml -p creamui-core
cargo test --manifest-path ../creamui/Cargo.toml -p creamui-render --lib
cargo test --manifest-path ../creamui/Cargo.toml -p creamui-image
```

GPU/CPU parity tests cover color matrices, all four rotations, clipping,
transparency and unchanged texture bindings. The image effects change must be
available in the sibling CreamUI checkout before building the app.

`build.sh` stages `cviewer`; `install.sh` installs the binary and `cviewer.desktop`.
The CFiles installer fixture verifies both against temporary installation roots.
