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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub window_count: usize,
    /// Output currently displaying this workspace, if the compositor maps
    /// workspaces to outputs individually.
    pub output: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceCapabilities {
    pub create: bool,
    pub destroy: bool,
}

pub trait DesktopIntegration {
    fn prepare_window(&self, window: &WindowHandle);
    fn windows(&self) -> Vec<OpenWindow>;
    fn activate_window(&self, id: &str);
    fn close_window(&self, _id: &str) {}
    fn window_changes(&self) -> Option<WindowChangeListener> {
        None
    }
    fn work_area(&self) -> Option<DesktopWorkArea> {
        None
    }
    fn workspaces(&self) -> Vec<Workspace> {
        Vec::new()
    }
    fn activate_workspace(&self, _id: &str) {}
    /// Creates a workspace and returns its id so the caller can activate it.
    fn create_workspace(&self) -> Option<String> {
        None
    }
    fn destroy_workspace(&self, _id: &str) {}
    fn workspace_capabilities(&self) -> WorkspaceCapabilities {
        WorkspaceCapabilities::default()
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
