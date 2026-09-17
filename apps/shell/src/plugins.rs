//! The fixed, compile-time plugin registration list. Every domain (weather,
//! clock, tray, ...) is a separate crate under `plugins/<id>/` implementing
//! `coconut_plugin_kit::Plugin`; this is the one place they're all wired
//! together into the running shell.
//!
//! This is also the seam a future Lua scripting layer would extend: a
//! `plugins/lua-bridge` crate could discover `*.lua` scripts and push
//! `Box<dyn Plugin>` wrappers onto this same list, with no change needed to
//! `PluginRegistry`/`PanelHost`/the config schema.
use coconut_plugin_kit::Plugin;

pub fn all_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(coconut_plugin_logo::LogoPlugin),
        Box::new(coconut_plugin_weather::WeatherPlugin),
        Box::new(coconut_plugin_clock::ClockPlugin),
        Box::new(coconut_plugin_current_playing::CurrentPlayingPlugin),
        Box::new(coconut_plugin_app_launcher::AppLauncherPlugin),
        Box::new(coconut_plugin_app_drawer::AppDrawerPlugin),
        Box::new(coconut_plugin_tray::TrayPlugin),
    ]
}
