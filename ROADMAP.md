# Coconut desktop roadmap

Build Coconut into a complete Linux desktop for everyday applications, work,
gaming, and software development, with the reliability expected from an
environment such as KDE Plasma.

The next development phase should prioritize compatibility, system integration,
and stability. Coconut already has a substantial desktop interface; the largest
remaining gaps are complete workflows and recovery when something fails.

This plan is based on a static review of Coconut and the adjacent Blair checkout
on October 31, 2026. It is not a hardware compatibility certification or the
result of running a graphical session. Confirmed implementation gaps are
identified below; other items are implementation or validation targets.

## Existing foundation

Coconut already provides configurable bars, an application launcher, window and
workspace integration, notifications, Settings, GTK/Qt appearance adapters,
clipboard history, a file manager, terminal, editor, media applications,
screenshots, and a greeter. NetworkManager, BlueZ, PipeWire, and power services
have native integrations.

Build on these components by completing their workflows and testing them
together. Component-specific behavior belongs in application READMEs and shared
development conventions belong in the existing guides.

## Priorities and ownership

- **P0:** Blocks a basic desktop workflow, application compatibility, session
  reliability, or safe locking.
- **P1:** Required for a practical daily-use desktop and its initial release.
- **P2:** Extends hardware coverage, advanced workflows, or desktop polish after
  the foundations work.

Coconut owns the shell, session orchestration, Settings, desktop services, and
application integration. Blair owns compositor protocols, display handling,
input routing, and presentation. CreamUI owns reusable widgets, accessibility
semantics, and application platform support. Integrate established system
services where appropriate and record compositor dependencies explicitly.

Use [TODO.md](TODO.md) for concrete deferred work and known failures. This roadmap
provides the broader priorities and milestones; existing TODO entries remain
relevant until their work is completed.

## Confirmed blockers

### 1. Connect to a new protected Wi-Fi network — P0

The current NetworkManager integration activates saved profiles and creates
connections to open networks, but returns without connecting when an unknown
network is protected.

- [ ] Add password entry, connection profile creation, and connection progress.
- [ ] Report authentication failures and support cancellation and retries.
- [ ] Integrate NetworkManager secret-agent requests and credential storage.
- [ ] Verify that a fresh user can connect without preparing a profile elsewhere.

Reference: [NetworkManager integration](integrations/network-manager/src/lib.rs).

### 2. Add XWayland support in Blair — P0

Blair's TODO explicitly lists XWayland as missing, preventing X11-only
applications from running under that compositor.

- [ ] Implement XWayland lifecycle and window integration in Blair.
- [ ] Validate focus, clipboard, drag and drop, window identity, and fullscreen
  behavior across Wayland and X11 applications.
- [ ] Include X11 applications and Steam/Proton workloads in compatibility checks.

Track implementation in Blair; Coconut should expose those windows consistently
through its launcher, dock, and workspace controls.

### 3. Support multiple monitors and output hotplug — P0

Blair's DRM backend currently activates one connector. Its TODO lists extended
and mirrored output and runtime output hotplug as pending work.

- [ ] Implement multiple physical outputs and connection/disconnection in Blair.
- [ ] Add extended and mirrored display arrangements.
- [ ] Expose layout, scale, rotation, primary-display policy, and safe changes in
  Coconut Settings.
- [ ] Remember monitor and dock configurations and recover displaced windows.
- [ ] Validate mixed scales, refresh rates, and lock behavior on added outputs.

Reference: [current display integration](integrations/system/src/hardware.rs).

### 4. Complete portal-based screen sharing — P0

Coconut's portal policy selects Settings, FileChooser, and Access implementations,
but does not configure a dedicated ScreenCast backend. Working screenshots alone
do not establish a complete screen-sharing workflow.

- [ ] Implement or integrate a ScreenCast backend compatible with Blair.
- [ ] Provide source selection and PipeWire streams for supported capture types.
- [ ] Show an active-sharing indicator and a direct stop action.
- [ ] Validate browser calls, OBS, cancellation, permissions, and Flatpak clients.
- [ ] Plan RemoteDesktop support as a separate capability with explicit consent.

References: [portal policy](integrations/appearance/data/coconut-portals.conf),
[permission service](apps/permission-portal/README.md), and the
[ScreenCast specification](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html).

### 5. Confirm the session lock before suspension — P0

Blair supports session locking, but its TODO identifies a race in which suspend
can happen before the locker confirms presentation of locked content.

- [ ] Hold a logind sleep-delay inhibitor until the lock is confirmed.
- [ ] Coordinate lock policy with idle, lid-close, and explicit suspend actions.
- [ ] Validate resume, locker failure, and lock coverage on every output.

Keep compositor enforcement in Blair and session policy integration explicit.

### 6. Recover the shell independently of applications — P0

The session launcher starts the background and clipboard processes and then
executes the shell as its session process. It has no explicit supervision layer
for independent component recovery.

- [ ] Supervise session components and define restart and shutdown policies.
- [ ] Recover from a shell crash while preserving running applications.
- [ ] Detect service startup failures and offer useful diagnostics.
- [ ] Verify graphical activation environment propagation after compositor startup.
- [ ] Integrate graphical session targets on systemd-based systems and define
  lifecycle behavior for other supported configurations.

References: [session launcher](packaging/coconut-session),
[bootstrapper](packaging/startcoconut), and
[portal system integration](https://flatpak.github.io/xdg-desktop-portal/docs/system-integration.html).

### 7. Provide administrative authentication in Coconut sessions — P0

Settings currently attempts to start KDE's Polkit agent only when it detects a
KDE session. Coconut needs a defined authentication-agent lifecycle of its own.

- [ ] Start and supervise a suitable Polkit agent for the Coconut session.
- [ ] Make authorization prompts available to applications and system settings.
- [ ] Surface denied, cancelled, and failed operations consistently.

Reference: [current Polkit startup](apps/settings/src/polkit.rs).

### 8. Complete Bluetooth pairing — P1

The BlueZ integration discovers, connects, disconnects, and forgets devices, but
the review found no Coconut agent for PIN, passkey, and confirmation requests.

- [ ] Implement or integrate a pairing agent and its consent dialogs.
- [ ] Support cancellation, timeouts, trust decisions, and actionable errors.
- [ ] Validate headphones, keyboards, mice, and game controllers.

Reference: [BlueZ integration](integrations/bluez/src/lib.rs).

## Desktop capability backlog

These are implementation and validation goals. Existing partial support should
be assessed before creating tasks that duplicate working features.

| Area | Required outcome | Priority | Main responsibility |
| --- | --- | --- | --- |
| Session and power | Lock, log out, restart, shut down, and suspend; handle idle, lid close, critical battery, and application inhibitors. | P0 | Coconut session + Blair + system services |
| Credentials | Integrate a Secret Service provider, login-time unlocking, and network/application secret storage. | P0 | Coconut session + credential provider |
| File operations | Multiple selection, cut/move, trash/restore, progress, cancellation, and copy conflict handling. | P0 | CFiles + shared file operations |
| External storage | Mount, unmount, eject, and unlock encrypted volumes with understandable failures through UDisks2 integration. | P1 | Coconut services + CFiles |
| Application tray | Host StatusNotifierItem icons and application menus for messaging, synchronization, VPN, and other background apps. | P1 | Coconut shell |
| Audio | Per-application mixer, input/output selection, headset profiles, microphone level meter, and device-disconnection recovery. | P1 | Coconut + PipeWire/WirePlumber |
| Application management | Install, remove, and update apps; manage defaults; report progress and failures. Settings currently checks for updates without installing them. | P1 | Coconut + package/Flatpak services |
| Advanced networking | VPN, enterprise Wi-Fi, hidden networks, proxy, DNS, and captive-portal detection. | P1 | Coconut + NetworkManager |
| Notifications | History, unread state, do not disturb, per-app preferences, and persistent actions where appropriate. | P1 | Coconut shell + notification service |
| Window workflows | Consistent Alt-Tab, edge snapping, interactive tiling, moving between displays, and recovery of off-screen windows. | P1 | Blair + Coconut controls |
| Accessibility and languages | Screen-reader support, accessible widget semantics, complete keyboard navigation, translations, text composition, and input-method integration. | P1 | CreamUI + Coconut + Blair |
| Everyday work | Printing queues and cancellation, PDF workflows, archives, SMB/SFTP resources, and file search. | P1–P2 | Coconut apps + system integrations |
| Advanced hardware | Validate hybrid GPUs, ICC/color management, HDR, VRR, and drawing tablets; expose supported capabilities honestly. | P2 | Blair + CreamUI + Coconut Settings |
| Distribution | Reproducible builds, coordinated dependency versions, packages, and verified installation, removal, and upgrades. | P1 | Build and release tooling |

The current `tray` plugin groups Coconut's system controls; an application tray
host is a separate integration target. FileChooser multiple-file selection and
native file clipboard formats/cut-and-move are already tracked in
[TODO.md](TODO.md). See [shared file operations](crates/file-menu/README.md) for
the existing behavior.

## Gaming readiness

Maintain a repeatable compatibility matrix covering native Wayland applications,
X11 applications, and Steam/Proton workloads once XWayland is available.

- [ ] Test installation, launch, fullscreen, Alt-Tab, exit, and desktop recovery.
- [ ] Validate resolution changes without losing input, audio, or windows.
- [ ] Test relative pointer motion and pointer capture with real games; Blair
  already has protocol implementations that need workload validation.
- [ ] Test USB/Bluetooth controllers, headsets, and device changes during play.
- [ ] Assess explicit synchronization, multiple GPUs, VRR, and direct scanout;
  distinguish implemented support from configuration fields and planned work.
- [ ] Validate recording/streaming, suspend inhibition, and notification behavior.
- [ ] Measure frame pacing, latency, and resource use on actual hardware.

Acceptance target: a user can install a game, play, exit, and return to a working
desktop with input and audio intact. Publish the tested hardware and workloads.

## Software development readiness

- [ ] Validate Terminal with tmux, Neovim, htop, debuggers, and SSH. Prioritize
  terminal compatibility and mouse reporting, currently documented as missing.
- [ ] Extend CEdit with syntax highlighting, project/folder navigation, and
  search across files.
- [ ] Add LSP, diagnostics, formatting, and Git integration if CEdit's scope
  expands into a development editor.
- [ ] Include VS Code, JetBrains applications, and other external tools in the
  desktop compatibility matrix.
- [ ] Verify that containers, virtual network interfaces, and heavy compilation
  workloads do not make the shell unresponsive.

References: [Terminal](apps/terminal/README.md) and [CEdit](apps/editor/README.md).

## Accessibility foundation

Start accessible semantics in CreamUI early. Contrast and magnification settings
are useful, but applications also need to expose control roles, names, states,
and focus to assistive technologies. Verify real screen-reader interaction and
keyboard-only completion of primary workflows as shared components evolve.

## Delivery milestones

### Milestone 1: Sign in and get connected

Complete session supervision, protected Wi-Fi, administrative authentication,
credential integration, safe locking, and suspend/resume.

Acceptance: a fresh user can sign in, connect to a protected network, authorize
a system change, lock, suspend, and resume. A shell failure does not discard
running applications.

### Milestone 2: Use existing applications

Complete Blair's XWayland and multi-monitor foundations, portal integration,
application tray hosting, essential file operations, and removable storage.

Acceptance: users can run supported Wayland/X11 apps, use file dialogs, move
files between local and external storage, and connect/disconnect a display
without stranding windows.

### Milestone 3: Work, develop, and play

Validate video calls, screen sharing, audio routing, Steam/Proton, development
tools, printing, and peripherals against published compatibility scenarios.

Acceptance: the work, development, and gaming scenarios above pass on the
declared supported hardware and software combinations.

### Milestone 4: Make the desktop distributable

Complete reproducible installation and updates, recovery, accessibility,
translations, and testing across representative GPU configurations.

Acceptance: another person can install, upgrade, diagnose, and remove Coconut
using documented steps, and complete the supported daily workflows.

## Validation and task tracking

Introduce CI and an integrated desktop smoke test from the first milestone:

1. Sign in with a fresh user configuration.
2. Connect to protected Wi-Fi.
3. Open applications and exercise their file dialogs.
4. Copy and move files.
5. Share the screen in a call.
6. Suspend and resume with the session locked.
7. Close applications and log out cleanly.

Add failure cases such as bad credentials, cancelled prompts, service loss,
component crashes, disconnected devices, and failed writes. Turn the existing
environment-dependent scene/test failures in [TODO.md](TODO.md) into reproducible
fixtures and separate them from new regressions.

Track each implementation task with a responsible component, priority,
dependencies, acceptance scenario, and validation evidence. Start Coconut work
with protected Wi-Fi, the authorization agent, and session supervision while
Blair develops XWayland and multi-monitor support. Keep accessibility and release
infrastructure progressing throughout the milestones.
