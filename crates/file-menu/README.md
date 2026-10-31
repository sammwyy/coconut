# Coconut File Menu

`coconut-file-menu` supplies file and folder context menus for File Explorer and
the shell desktop. CreamUI owns the generic menu widget, nested items, callbacks,
placement, scrolling, pointer shielding and keyboard navigation.

Keep one `FileMenuController` per window. Call `open` with a `WindowHandle`, a
window-local pointer position and either `FileMenuTarget::Item` or
`FileMenuTarget::Background`. Append `controller.overlays(size)` after the window's
content. Forward native keys to `handle_key`, call `parent_focus_lost` on focus
loss, and call `close_menu` on navigation and tab changes. `has_panel` lets native
shortcut handlers leave text input to the active form.


On CreamUI's native Wayland backend, menus use `xdg_popup` surfaces positioned
at the initiating right-click. Each submenu is a child popup with its own surface,
so menus can extend outside the application. The compositor constrains placement
and size, owns the input grab and dismisses outside clicks. This uses the standard
XDG shell protocol shared by Hyprland, KWin and Blair, including layer-shell popup
parenting for the desktop. There is no compositor-name detection.

The input serial is captured before background menu inspection. Basic actions
appear immediately; background inspection enables Paste and fills Open with in
place without moving the menu. Dismissed menus ignore later inspection results.
The menu remains
open when keyboard focus moves from its parent to a popup, and popup descendants
close before their ancestors. Losing focus from the deepest popup dismisses the
whole menu; transferring focus into a submenu keeps the menu open.
Missing platform support, a missing input serial,
or popup creation failure selects the same menu inside the application instead.
Windows and the Winit backend use this hosted fallback.

`FileMenuOptions` supplies the surface's Open callback and refresh callback.
`open_label` customizes its label, such as Use file name in a Save chooser.
`extras` appends surface-specific actions without duplicating common actions;
`editable(false)` removes creation, rename and paste for restricted surfaces.
`properties(false)` hides folder metadata on the desktop background, whose
settings action belongs to the desktop rather than the underlying directory.
Use `perform` for additional background actions that need the same error feedback.

Item menus offer Open, Open with, Copy, Rename and Properties. Folder items also
accept Paste. Background menus offer New > Folder / Empty file and Paste. Both
variants provide Open terminal here and Open in File Explorer. File Explorer adds
sorting, hidden files, refresh and preferences. The desktop adds icon sorting,
grid arrangement, refresh and desktop settings.

Application discovery uses Delib's XDG MIME associations, including user additions,
removals and desktop-file overrides. Launching expands desktop-entry file codes
without a shell and honors `Terminal` and `Path`. Application launching uses
Coconut Core's process helpers, including configured terminal selection and
sibling-binary discovery for File Explorer. The terminal action starts
`cterm --working-directory`; opening the explorer starts `cfiles`.

The crate runs menu inspection, clipboard access, properties, copying, creation,
rename and its process-launch actions on the window's background executor.
Obsolete menu loads are discarded. Rename uses Linux `RENAME_NOREPLACE`, and creation/copy use exclusive
writes. Existing destinations are never overwritten. Copy preserves symbolic
links and rejects copying a folder into itself. A failed recursive copy may leave
completed entries in the destination; the error explains this and refreshes the
surface.

Copy exports local file URIs as text and Paste accepts local file-URI lists.
The clipboard owner stays alive for the process lifetime; Linux uses Wayland
data-control where supported, with the X11 backend available as a fallback.
Native file clipboard formats such as `x-special/gnome-copied-files` and cut/move are not implemented.
