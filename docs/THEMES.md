# Coconut desktop themes

Coconut owns desktop theme packages and the user's selection. CreamUI remains
an independent toolkit. The same effective Coconut tokens feed the shell,
CreamUI, GTK and Qt, while each framework can have its own optional adaptation.

## Packages and selection

Packages are discovered in XDG data order, with user packages before system
packages:

```text
~/.local/share/coconut/themes/my-theme/
/usr/share/coconut/themes/my-theme/

my-theme/
├── theme.toml
├── tokens.toml
├── shell.toml
├── variants/
│   ├── light.toml
│   ├── dark.toml
│   └── midnight.toml
├── cream/                 # optional token overrides
├── gtk/                   # optional token overrides and CSS templates
├── qt/                    # optional token overrides, palette and QSS
└── assets/                # optional package resources
```

The directory name must match the manifest ID. IDs contain ASCII letters,
digits, hyphens or underscores. All resource paths in a manifest are relative
to its package. Start with a copy of the bundled `themes/coconut` package and
change its ID and name.

Installation also registers Coconut's bundled Plus Jakarta Sans fonts in the
native font directory so GTK and Qt can use the same family as CreamUI.
Other chosen families must be installed on the system.

```toml
# theme.toml
schema = 1
id = "my-theme"
name = "My Theme"
default_variant = "dark"
tokens = "tokens.toml"
shell = "shell.toml"

[variants.light]
appearance = "light"
file = "variants/light.toml"

[variants.dark]
appearance = "dark"
file = "variants/dark.toml"

[variants.midnight]
appearance = "dark"
file = "variants/midnight.toml"

[accent]
customizable = true
presets = ["#0099d3", "#8271e9", "#d45591"]
```

Variants may have arbitrary names; `appearance` must explicitly be `light` or
`dark`. Midnight is a dark variant, not a third standard color-scheme value.
Themes do not have to supply all three bundled variants.

The only authoritative selection is
`$XDG_CONFIG_HOME/coconut/appearance.toml` (normally
`~/.config/coconut/appearance.toml`):

```toml
theme = "my-theme"
variant = "midnight"

[overrides]
accent = "#db82ad"
font_family = "Inter, system-ui"
corners = "soft"

[accessibility]
high_contrast = false
text_scale = 1.0
```

An omitted override inherits the theme's value. Omit `variant` to use the
package's default. Corners can be `square`, `soft` or `round`; they multiply
authored radii by zero, one half or one respectively. Text scale ranges from
0.75 to 2. The theme package is never rewritten when the selection changes.

Settings lists Coconut packages and their variants. Themes with
`accent.customizable = false` keep their authored accent and disable the
custom accent control. Existing CreamUI preferences are left intact: on first
adoption Coconut imports the old selection/accessibility preferences, and
copies a custom legacy CreamUI theme into a Coconut package if needed.

## Tokens and resolution

See `themes/coconut/tokens.toml` for the complete schema. Shared groups are:

- `colors`: surfaces, hover surface, text roles, accent states, `on_accent`,
  selection colors, borders and semantic danger/warning/success colors.
- `typography`: family stack and caption/body/section/title sizes in logical
  pixels.
- `geometry`: control/input/card/menu radii, spacing and border width.
- `custom`: optional author-defined tokens available to templates, such as
  `custom.brand_color` or `custom.sidebar_width`.

Colors accept `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`. Variant files are partial
overrides of the base groups. They can also contain a `[shell]` table. For
example:

```toml
# variants/midnight.toml
[colors]
surface = "#060606f8"
surface_elevated = "#0e0e0ef8"

[shell]
dock_opacity = 0.96
island_opacity = 0.92
```

Resolution applies package defaults, then the chosen variant, then a target's
token overrides, then explicit user overrides, then accessibility. Every target
starts from the same authored base; overrides for GTK do not affect CreamUI or
Qt. Custom accents also resolve hover/pressed colors and an appropriate black
or white `on_accent`. High contrast resolves strong foreground/background
colors, increases border width, and makes shell surfaces opaque without blur.

## Automatic and manual adapters

All three adapters are automatic when their manifest sections are absent.
Their optional settings are declared under `[adapters.cream]`,
`[adapters.gtk]` or `[adapters.qt]` in `theme.toml`.

```toml
[adapters.cream]
tokens = "cream/tokens.toml"

[adapters.gtk]
base = "auto"
tokens = "gtk/tokens.toml"
gtk3 = "gtk/extra-gtk3.css.in"
gtk4 = "gtk/extra-gtk4.css.in"

[adapters.gtk.variants.midnight]
gtk3 = "gtk/midnight-gtk3.css.in"
gtk4 = "gtk/midnight-gtk4.css.in"

[adapters.qt]
base = "auto"
palette = "qt/palette.toml"
stylesheet = "qt/extra.qss.in"
```

An adapter's `tokens` file uses the same groups as the shared tokens file, but
all groups and fields are optional. It may contain typed references:

```toml
# gtk/tokens.toml
[geometry]
control_radius = 12
card_radius = "${tokens.geometry.card_radius}"
```

Token-overlay references read the authored base plus variant, so user corner
scaling and text scaling apply exactly once. CSS, QSS and palette templates
read the final effective tokens after user preferences and accessibility.

Available variables include:

```text
${tokens.colors.surface}
${tokens.colors.text_primary}
${tokens.colors.accent}
${tokens.colors.on_accent}
${tokens.geometry.control_radius}
${tokens.typography.family}
${tokens.custom.brand_color}
${appearance.theme}
${appearance.variant}
${appearance.color_scheme}
${appearance.accent}
${appearance.high_contrast}
${appearance.text_scale}
${package.uri}
```

`package.uri` is a file URI ending in `/` for disk-backed packages. It allows
native CSS/QSS to reference assets without copying them into generated themes:
`url("${package.uri}assets/background.svg")`.

References substitute known scalar values. An entire TOML string containing a
reference preserves the referenced number/boolean/string type. Unknown tokens
and malformed references are errors. There are no executable scripts or
conditional expressions: use per-variant adapter overrides for differences.

### GTK

Automatic GTK output imports the toolkit's built-in Adwaita/Default stylesheet
and maps Coconut tokens onto named colors and common controls. Translucent
surface/hover colors are composed onto suitable opaque backgrounds for native
applications. Optional CSS templates are appended after this generated base:

```css
/* gtk/extra-gtk3.css.in */
button.suggested-action {
    background-color: ${tokens.colors.accent};
    color: ${tokens.colors.on_accent};
    border-radius: ${tokens.geometry.control_radius}px;
}
```

With `base = "manual"`, the supplied `gtk3` and `gtk4` templates replace the
automatic output; both must be present. They can import the toolkit's base
stylesheet themselves. Literal styles are intentional overrides: authors who
want custom accents and accessibility values to remain dynamic should use the
corresponding effective token references.

GTK themes are generated in the standard XDG themes directory as `Coconut` and
`Coconut-Alt`. Two bounded slots let an accent-only edit change `gtk-theme-name`
and trigger a native reload without creating an unlimited number of themes.
The bridge updates GSettings and the owned keys in GTK 3/4 `settings.ini`,
preserving unrelated settings and leaving user `gtk.css` files alone.
`xsettingsd` provides updates to X11/XWayland clients when a display is present;
it does not replace another desktop's XSettings owner.

Libadwaita and apps with their own styling primarily consume standard
light/dark/accent/contrast preferences. They do not promise to reproduce a
complete third-party GTK theme.

### Qt

Automatic output maps the effective tokens onto all palette roles for active,
inactive and disabled states. A palette file may override individual roles:

```toml
# qt/palette.toml
[active]
Window = "${tokens.colors.surface}"
WindowText = "${tokens.colors.text_primary}"
Highlight = "${tokens.colors.accent}"
HighlightedText = "${tokens.colors.on_accent}"
ToolTipBase = "#303030"
ToolTipText = "#ffffff"
```

With `base = "manual"`, the palette must provide every role in each of
`[active]`, `[inactive]` and `[disabled]`. The accepted roles are listed in
`integrations/qt/src/lib.rs::ROLES`. Templates use shared color notation;
the adapter converts it to QColor's alpha ordering when writing native output.
An optional QSS template can customize Qt Widgets. An application's own
stylesheet takes precedence; the plugin only installs/replaces its own sheet.

The native `coconut` platform-theme plugin reads the generated palette and
receives live changes through D-Bus and file watching. It supplies system
fonts, appearance and a Fusion-based palette. The session selects Qt's
`xdgdesktopportal` wrapper with Coconut as its underlying theme so native file
dialogs still use the portal. This colors Qt Widgets and consumers of Qt's
system palette; application-specific Qt Quick styles may need their own support.

The plugin uses QPA private APIs and must be built against the Qt version
installed on the target system. `build.sh` builds Qt 6 by default. To also
build Qt 5, install its development/private headers and use
`COCONUT_QT_MAJORS="5 6" ./scripts/build.sh`. Qt 5 and Qt 6 plugins are installed
alongside each other; Qt's plugin metadata selects a compatible build. Qt 6 is
covered by the native integration test; Qt 5 requires validation on a Qt 5 host.
Sandboxed applications additionally need access to a compatible native theme or
plugin in their runtime.

## Session, persistence and notifications

The generated files under `$XDG_DATA_HOME/coconut/appearance/` are derived
artifacts, not user configuration:

```text
cream.toml    # Native, fully resolved CreamUI appearance
qt.json       # Native Qt palette, font, scheme and optional stylesheet
shell.toml    # Effective Coconut shell values
```

The session exports stable `CREAMUI_APPEARANCE` and
`COCONUT_QT_APPEARANCE` paths, plus the Qt plugin selection/search path, before
applications start. Their content changes without changing process environments.
CreamUI's generic loader understands its native snapshot without depending on
Coconut; existing `CREAMUI_THEME`/`CREAMUI_VARIANT` overrides can still select a
standalone CreamUI theme. Apps that opt into system theming can use the existing
`ThemeListener` to reload their source.

The shell starts the appearance coordinator in a background thread. The
`coconut-appearance` executable supplies the same service through D-Bus
activation when needed before the shell starts. A single bus name prevents two
active coordinators.

`org.freedesktop.impl.portal.desktop.coconut` implements the Settings backend.
The public `xdg-desktop-portal` broker exposes standard preferences to apps:
`org.freedesktop.appearance/color-scheme`, `accent-color`, and `contrast`.
Other GTK settings are delegated to the GTK backend, and only one backend
publishes Coconut's appearance values.

After new output is available, the coordinator sends standard
`SettingChanged` signals, `org.coconut.Appearance.Changed` at
`/org/coconut/Appearance`, and CreamUI's existing `ReloadTheme` signal. It also
checks the selection and package files once per second to pick up manual edits.
Invalid edits retain the last valid active appearance and report the error.
Files are replaced atomically and unchanged output is not rewritten.

## Development and checks

```bash
cargo test -p coconut-theme -p coconut-integration-cream \
  -p coconut-integration-gtk -p coconut-integration-qt \
  -p coconut-integration-appearance
cargo build -p coconut-integration-appearance
cargo run -p coconut-integration-appearance -- --check
cargo run -p coconut-integration-appearance -- --once
```

`--check` validates the selection, packages, references and Qt palette mappings.
`--once` exports and applies one appearance; normal operation is the long-lived
coordinator. Initial session setup runs it once before apps inherit their
source paths. Native CSS syntax remains subject to the target toolkit's parser.

The live test needs PyGObject with GTK 3/4, Qt development/private headers,
`xsettingsd`, `xdg-desktop-portal`, `xdg-desktop-portal-gtk`, D-Bus and Xvfb:

```bash
cmake -S integrations/qt/plugin -B .build/qt6 -DBUILD_TESTING=ON
cmake --build .build/qt6
dbus-run-session -- xvfb-run -a /usr/bin/python3 tests/appearance_session.py
```

It uses temporary XDG paths and a private bus/display to verify public portal
values/signals, live GTK colors, live Qt palette/font updates, accent-only
changes, native CreamUI export and recovery from invalid configuration.
