//! Reads back the screen contents without depending on any particular
//! compositor: `wlr-screencopy` on Wayland, the root window image on X11.

use crate::image::Shot;

#[cfg(target_os = "linux")]
mod kwin;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

/// Captures every visible output composited into one image at native pixels.
pub fn screen() -> Result<Shot, String> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return wayland::screen();
        }
        if std::env::var_os("DISPLAY").is_some() {
            return x11::screen();
        }
        Err("neither WAYLAND_DISPLAY nor DISPLAY is set".into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("screen capture is only supported on Linux (Wayland/X11)".into())
    }
}
