use std::path::PathBuf;

use super::ChangeListener;
use creamui_render::WindowHandle;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenWindow {
    pub id: String,
    pub app_name: String,
    pub title: String,
    pub icon_path: Option<PathBuf>,
    pub active: bool,
}

pub type WindowChangeListener = ChangeListener;

pub trait DesktopIntegration {
    fn prepare_window(&self, window: &WindowHandle);
    fn windows(&self) -> Vec<OpenWindow>;
    fn activate_window(&self, id: &str);
    fn window_changes(&self) -> Option<WindowChangeListener> {
        None
    }
}

pub struct Fallback;

impl DesktopIntegration for Fallback {
    fn prepare_window(&self, window: &WindowHandle) {
        window.set_always_on_top(true);
    }
    fn windows(&self) -> Vec<OpenWindow> {
        Vec::new()
    }
    fn activate_window(&self, _id: &str) {}
}
