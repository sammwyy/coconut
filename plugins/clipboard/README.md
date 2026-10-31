# Clipboard history widget

The `coconut-plugin-clipboard` shell plugin registers the optional `clipboard`
island and popup. Enable **Clipboard history** for the statusbar, dockbar or both
in **Settings → Desktop → Islands**, then restart the shell to apply its layout.
It is absent from the default layout. In `shell.toml`, add it to the desired
existing section as `[[statusbar.section.island]]` (or
`[[dockbar.section.island]]`) with `id = "clipboard"`.

Click its clipboard icon to view the current copied text and history, newest
first. Select an entry to restore its full text and close the popup. Remove and
Clear history leave the current clipboard intact. Pause history prevents new
entries; Resume history starts saving subsequent copies. Settings opens the
Privacy → Clipboard page for retention limits and application permissions.

The popup shows loading, empty, paused and unavailable/error states. Its list is
paged in groups of 20 with persistent scroll state during an open popup. Tab and
Shift+Tab navigate controls; Enter restores a focused entry. Escape, outside
click and focus dismissal use the shell's standard panel host.

Content and commands use the separate
[clipboard service](../../integrations/clipboard/README.md). D-Bus work runs off
the UI thread. While open, the popup waits for revision changes and reloads its
current page; it does not repeatedly poll clipboard contents. Closed popups drop
their UI state, and outstanding wait callbacks use weak references. The service
keeps collecting when the popup closes or the island is hidden.

Scene tests render ready, empty, paused and unavailable states in light/dark
themes at normal and compact sizes and exercise keyboard restoration:

```bash
COCONUT_CLIPBOARD_SNAPSHOT=/tmp/clipboard cargo test -p coconut-plugin-clipboard
```
