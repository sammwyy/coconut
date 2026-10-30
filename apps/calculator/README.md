# Calculator

Native CreamUI calculator and unit converter. Package: `coconut-calculator`;
executable: `ccalc`; window and desktop title: `Calculator`.

```bash
cargo run -p coconut-calculator
cargo run -p coconut-calculator -- --scientific
ccalc --help
```

The app follows the system theme and preferred font, including live appearance
changes. Linux supports Wayland and X11 with optional Blair/KWin integration.
The manifest also selects CreamUI's Windows backend; Windows runtime behavior
has not been validated.

## Navigation and window sizes

The upper-left mode button opens a compact navigation overlay, hidden at startup.
It has **Calculator** (Standard, Scientific) and **Converter** sections. It does
not reserve width, move the display/keypad, dim the window or resize the window.
Choosing a mode, typing, clicking outside or pressing Escape closes it. Ctrl+B
toggles it. The navigation list scrolls independently when needed.

CreamUI Router owns the current route. The application frame, display, memory,
history and dispatch path are shared; only the routed keypad/conversion layout
changes. The numeric keypad uses layout variants for standard and converter
views; scientific mode places function keys beside it. Standard and Scientific
share one calculation; each conversion category retains its own entry and units.

| Mode | Default window | Minimum window |
| --- | --- | --- |
| Standard | 360×520 | 320×480 |
| Scientific | 720×560 | 620×500 |
| Converter | 500×620 | 460×540 |

Sizes are logical pixels. Switching modes while the window has the previous
mode's default size changes both dimensions to the new default. A custom size is
preserved, expanding only dimensions below the new minimum. Pending resize
requests are tracked so rapid mode changes use the intended size. Showing or
hiding navigation never changes these sizes. The native minimum is applied
through `WindowHandle::set_min_size` in the sibling CreamUI checkout, including
Wayland `xdg_toplevel` constraints and winit's desktop backends.

## Calculating

The display is read-only, with a large right-aligned result and a smaller
expression. There is no text field, caret or selection to focus. While the window
is active, type numbers, decimal points, operators or function expressions from
any normal control focus. Buttons and native keys invoke the same actions. Tab
and Space remain available for control navigation and activation; open unit
popups and command search own their keyboard input.

Enter or `=` evaluates, Backspace removes the previous Unicode character, and C,
Delete or Escape clears the entry. Entering a number after evaluation starts a
new calculation; an operator continues from the answer. Functions wrap the current
expression, or start a function call when the entry is empty. `Ans` inserts the
last successful calculation or conversion result. Empty input represents zero.
Failed calculations preserve the entry, memory, answer and history.

The keypad and function panel scroll independently when the window is too short
or text is enlarged. Display and common controls remain visible. Secondary actions
and less common functions are available through F1 / Ctrl+Shift+P, without a
permanent command-menu button. Ctrl+V replaces the current entry with a validated
clipboard expression. Ctrl+C copies the computed result at round-trip precision.
Clipboard errors appear beside the result.

| Syntax | Meaning |
| --- | --- |
| `+ - * /` or `+ − × ÷` | Arithmetic with multiplication/division before addition/subtraction |
| `(expression)` | Grouping; parentheses must be closed explicitly |
| `x^y`, `pow(x,y)` | Powers, associated to the right: `2^3^2 = 512` |
| `-2^2`, `(-2)^2` | `-4` and `4`, respectively |
| `x!`, `fact(x)` | Factorial of a whole number from 0 through 170 |
| `x%` | Divide x by 100: `200*10% = 20`; `200+10% = 200.1` |
| `x mod y` | Remainder, with the sign of x |
| `2(3+4)`, `2pi` | Implicit multiplication, at the same precedence as `*` |
| `1.25e-3` | Decimal scientific notation; use `.` as the decimal separator |
| `pi` / `π`, `tau` / `τ`, `e` | Mathematical constants |
| `ans`, `mem` | Last successful answer and memory (initially zero) |
| `sin`, `cos`, `tan`, `asin`, `acos`, `atan` | Trigonometric functions using the selected angle unit, including inverse results |
| `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh` | Hyperbolic functions, independent of angle units |
| `sqrt(x)`, `cbrt(x)`, `abs(x)`, `reciprocal(x)` | Square root, cube root, absolute value, reciprocal |
| `ln(x)`, `log(x)`, `log10(x)`, `log2(x)`, `log(x,base)` | Natural, decimal, binary and arbitrary-base logarithms |
| `exp(x)` | e raised to x |
| `floor(x)`, `ceil(x)`, `round(x)` | Rounding; half values round away from zero |
| `ncr(n,r)`, `npr(n,r)` | Combinations and permutations; whole numbers with `0 ≤ r ≤ n ≤ 10000` |
| `sum(...)`, `mean(...)`, `min(...)`, `max(...)` | Aggregates with 1–64 arguments |

Functions require parentheses; commas separate arguments. Names are case
insensitive. Scientific functions remain valid when standard mode is selected.

## Converting units

Choose a category in navigation, enter the source quantity and select **From**
and **To** units. The result updates immediately. **Swap** / Ctrl+U exchanges the
units and uses the converted value as the new source, preserving the physical
quantity when the entry is valid. Enter / `=` adds the conversion to shared
history; the next number starts a new source quantity. Arithmetic expressions
also work as source quantities.

| Category | Units include |
| --- | --- |
| Length | Metric lengths, inches, feet, yards, miles, nautical miles |
| Area | Metric areas, hectares, acres, square inches/feet/yards/miles |
| Volume | Liters, milliliters, cubic units, US and imperial gallons, US cups/pints/quarts/fluid ounces |
| Weight and mass | Metric mass, pounds, ounces, stones, US short tons, imperial long tons |
| Temperature | Celsius, Fahrenheit, Kelvin |
| Energy | Joules, kilojoules, megajoules, thermochemical calories, kilocalories, Wh, kWh, eV |
| Speed | m/s, km/h, mph, ft/s, knots |
| Time | Nanoseconds through weeks |
| Power | Milliwatts through megawatts |
| Pressure | Pa, kPa, MPa, bar, mbar, atmospheres, psi |
| Data | Bits, bytes, decimal kB/MB/GB/TB and binary KiB/MiB/GiB/TiB |
| Angle | Degrees, radians, gradians, revolutions, arcminutes, arcseconds |

US customary and imperial volumes are named separately. Conversion factors use
fixed definitions; time intentionally excludes variable calendar months/years.
Temperature conversion includes offsets and rejects values below absolute zero.
Overflow and invalid source expressions leave the entry and history intact.
Unit selectors support Tab, Space and arrows as well as pointer selection.

## Memory, history and shortcuts

MS stores the current entry's value, M+ adds it, M− subtracts it, MR inserts memory
and MC clears it. In converter modes memory uses the source expression's value,
without attaching a unit. Empty memory disables MR and MC; stored zero still
counts as populated memory. Invalid memory operations preserve the register.

History holds the last 100 successful calculations and conversions, newest first.
Conversion records include the category and source/destination units. Click a
result to reuse its numeric value in the current mode. Back or Escape returns to
the routed view. Clearing history preserves entries and registers. The shared
history stays open when switching modes. Closing command search restores the
underlying history view. Entries, units, mode, memory and history are session-only;
no files are written.

| Shortcut | Action |
| --- | --- |
| Enter or `=` | Evaluate / record conversion |
| Backspace | Delete previous character |
| Delete or Ctrl+L | Clear current entry |
| Escape | Dismiss the active transient/history, otherwise clear entry |
| Alt+1 / Alt+2 | Standard / Scientific |
| Ctrl+B | Toggle navigation overlay |
| Ctrl+H | Show or hide history |
| Ctrl+C / Ctrl+Shift+C | Copy result |
| Ctrl+V | Paste a validated expression |
| Ctrl+U | Swap conversion units |
| Ctrl+M / Ctrl+Shift+M | Store / clear memory |
| Ctrl+R | Recall memory |
| F1 or Ctrl+Shift+P | Search commands, modes and functions |
| Tab / Shift+Tab, Space | Focus navigation and control activation |

## Numeric limits and architecture

The engine calculates real numbers with IEEE 754 `f64`, not arbitrary precision
or symbolic algebra. Floating-point rounding applies, including tiny residuals
around trigonometric zeros. The display uses approximately 15 significant digits
and scientific notation for very large/small values; registers, history and
subsequent calculations retain the underlying value. Non-real results, overflow,
undefined tangent poles and division by zero produce errors.

Expressions are limited to 4096 UTF-8 bytes and 128 recursive parsing levels;
functions accept up to 64 arguments. Evaluation is synchronous with those bounds
and cached across unchanged views. Rendering bounds the visible expression and
history labels. History is capped at 100 entries. The entry, expression engine,
conversion tables and sizing policy are independent of widgets; routing and
persistent controllers belong to application state.

## Validation

```bash
cargo fmt --check
cargo check -p coconut-calculator
cargo test -p coconut-calculator
cargo clippy -p coconut-calculator --all-targets --no-deps -- -D warnings
cargo test -p coconut-files installation_tests
CCALC_PREVIEW_DIR=/tmp/ccalc-previews cargo test -p coconut-calculator
```

Tests cover arithmetic, scientific functions, domains, resource limits, conversion
round trips for every unit pair, affine temperatures, default/custom sizing,
window-wide keyboard input without a text field, router state retention, shared
history, pointer actions, overlay dismissal and unchanged keypad geometry.
Scene tests render every route in light/dark at default/minimum sizes, plus
history, errors, command search, text scaling and caption controls on either side.
The environment variable optionally saves PNGs using SceneRecorder/Rasterizer.

The sibling CreamUI change is validated with:

```bash
cargo test --manifest-path ../creamui/Cargo.toml -p creamui-platform --features wayland
cargo test --manifest-path ../creamui/Cargo.toml -p creamui-render --lib
```

`build.sh` stages `ccalc`; `install.sh` installs it and `ccalc.desktop`. The CFiles
installer fixture verifies this in temporary directories. Keep the CreamUI size
constraint change available before building this app.
