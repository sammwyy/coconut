# TODO

- Implement native notification history and unread counts for the statusbar
  notification capsule in the shell design.
- `shell.toml` is only read at startup, including when edited through
  `coconut settings`; changes there require restarting the shell process to
  take effect. The active CreamUI theme (`cream/active_theme.toml`) is the
  exception — the settings window itself updates live when you switch it.
- `statusbar.position` and `dockbar.position` only anchor correctly on the
  Wayland layer-shell backend (`creamui-platform`'s `WindowRole::TopPanel`/`BottomPanel`
  handling). The Windows backend does not place panel-role windows at all
  yet (pre-existing gap, not specific to the top/bottom option), so the bar
  position setting has no visible effect there.
- Network, Bluetooth, and battery panels react to native D-Bus signals
  (NetworkManager, BlueZ, UPower); kernel-backlight brightness reacts to an
  inotify watch on its sysfs node; volume reacts to a direct libpipewire
  connection (FFI, linked via `pipewire-devel`) subscribed to the default
  sink's param changes and the session's default-sink metadata. None of
  these poll.
- DDC-controlled external-monitor brightness (`ddcutil`) has no push
  notifications to hook into, so it still polls every 2 seconds — inherent
  to DDC/CI, not fixable without polling.
- The Windows integrations have no native hook for anything yet and fall
  back to polling throughout.
- CreamUI's system theme now carries a preferred `font_family` (loaded from
  the system's installed fonts), but the settings app has no UI to pick or
  clear one; it can currently only be set by hand-editing
  `~/.config/cream/appearance.toml` or the `CREAMUI_FONT` environment
  variable.
- Four `apps/terminal` scene-index tests fail on this machine
  (`single_session_has_centered_title_and_actions_opposite_window_controls`,
  `tab_strip_reserves_compositor_controls_and_renders_terminal`,
  `settings::general_island_controls_render_edit_and_restore_defaults`,
  `settings::recorder_detects_conflicts_cancels_clears_and_saves_atomically`).
  They already failed before the migration, so the cause is environmental
  rather than a regression — investigate when the test environment is
  reproducible.
- Five `apps/files` scene tests fail on this machine
  (`browser::browser_draws_one_set_of_client_controls_for_single_and_multiple_tabs`,
  `settings::about_tab_shows_package_information_and_project_links`,
  `settings::header_keeps_injected_controls_clear_on_either_side`,
  `settings::preferences_tabs_render_and_navigate_without_changing_config`,
  `settings::settings_draw_client_controls_only_for_client_decorations`).
  They already failed in the original `dfiles` repository, so the cause is
  environmental rather than a migration regression.
- Three `apps/monitor` visual tests fail on this machine
  (`app::visual_tests::frame_ownership_and_refresh_keep_header_positions_stable`,
  `app::visual_tests::hybrid_controls_on_either_side_relocate_actions_without_overlap`,
  `app::visual_tests::tabs_stay_centered_in_the_window_for_server_client_and_hybrid_frames`).
  They already failed in the original `dmonitor` repository, so the cause is
  environmental rather than a migration regression.
- One `apps/settings` visual test fails on this machine
  (`visual_tests::connectivity_layout_matches_reference`, a window-drag
  hit-test assertion). It already failed on the pre-lint HEAD (verified via
  worktree checkout), so the cause predates the lint work — same
  environmental category as the failures above.
- Implement real multiple-file selection for `apps/files` portal requests:
  support Ctrl/Shift selection in the browser and return every selected file
  as a URI in the FileChooser portal response. (Inherited from the dfiles
  TODO.)
