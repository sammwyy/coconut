#[cfg(not(target_os = "windows"))]
mod kwin_dbus;
#[cfg(target_os = "windows")]
mod windows;

use crate::platform::OpenWindow;
use creamui_render::WindowHandle;
use std::sync::{mpsc::Receiver, Arc, Mutex};

#[derive(Clone)]
pub struct WindowChangeListener {
    receiver: Arc<Mutex<Receiver<()>>>,
}

impl WindowChangeListener {
    pub(super) fn new(receiver: Receiver<()>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn wait(&self) -> bool {
        self.receiver
            .lock()
            .is_ok_and(|receiver| receiver.recv().is_ok())
    }
}

pub trait DesktopIntegration {
    fn prepare_window(&self, window: &WindowHandle);
    fn windows(&self) -> Vec<OpenWindow>;
    fn activate_window(&self, id: &str);
    fn window_changes(&self) -> Option<WindowChangeListener> {
        None
    }
}

pub struct Fallback;
#[cfg(not(target_os = "windows"))]
pub use kwin_dbus::KWinDbus;
#[cfg(target_os = "windows")]
pub use windows::WindowsDesktop;

impl DesktopIntegration for Fallback {
    fn prepare_window(&self, window: &WindowHandle) {
        window.set_always_on_top(true);
    }
    fn windows(&self) -> Vec<OpenWindow> {
        Vec::new()
    }
    fn activate_window(&self, _id: &str) {}
}
