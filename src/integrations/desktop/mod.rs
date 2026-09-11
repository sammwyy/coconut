mod kwin_dbus;

use crate::platform::OpenWindow;
use creamui_render::WindowHandle;

pub trait DesktopIntegration {
    fn prepare_window(&self, window: &WindowHandle);
    fn windows(&self) -> Vec<OpenWindow>;
    fn activate_window(&self, id: &str);
}

pub struct Fallback;
pub use kwin_dbus::KWinDbus;

impl DesktopIntegration for Fallback {
    fn prepare_window(&self, window: &WindowHandle) {
        window.set_always_on_top(true);
    }
    fn windows(&self) -> Vec<OpenWindow> {
        Vec::new()
    }
    fn activate_window(&self, _id: &str) {}
}
