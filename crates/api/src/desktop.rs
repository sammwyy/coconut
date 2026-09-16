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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesktopWorkArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait DesktopIntegration {
    fn prepare_window(&self, window: &WindowHandle);
    fn windows(&self) -> Vec<OpenWindow>;
    fn activate_window(&self, id: &str);
    fn window_changes(&self) -> Option<WindowChangeListener> {
        None
    }
    fn work_area(&self) -> Option<DesktopWorkArea> {
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
