//! File chooser request types and the D-Bus backend, independent of the UI.
mod filters;
mod options;
pub mod service;
pub use filters::{FileFilter, FilterRule};
use std::path::PathBuf;

/// Stable portal identity. The binary is named `cfiles`, but this is the
/// backend name XDG Desktop Portal policy uses to select File Explorer.
pub const BACKEND_ID: &str = "cfiles";
/// The implementation-side service name prescribed by the XDG portal API.
/// Keep this in one place so an application identity change is deliberate.
pub const BACKEND_SERVICE: &str = "org.freedesktop.impl.portal.desktop.cfiles";
/// Namespace reserved for File Explorer's own D-Bus APIs, if it grows any.
pub const APPLICATION_NAMESPACE: &str = "org.cfiles";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalType {
    OpenFile,
    SelectFolder,
    SaveFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortalRequest {
    pub kind: PortalType,
    pub app_id: String,
    pub parent_window: String,
    pub accept_label: Option<String>,
    pub suggested_name: String,
    pub filters: Vec<FileFilter>,
    pub current_filter: Option<FileFilter>,
    pub multiple: bool,
    pub modal: bool,
}

impl PortalRequest {
    pub fn new(kind: PortalType) -> Self {
        Self {
            kind,
            app_id: String::new(),
            parent_window: String::new(),
            accept_label: None,
            suggested_name: String::new(),
            filters: Vec::new(),
            current_filter: None,
            multiple: false,
            modal: true,
        }
    }
    pub fn save(name: impl Into<String>) -> Self {
        Self {
            suggested_name: name.into(),
            ..Self::new(PortalType::SaveFile)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortalSelection {
    pub path: PathBuf,
    pub current_filter: Option<FileFilter>,
}

#[derive(Clone, Copy, Debug)]
pub struct PortalCapabilities {
    pub glob_filters: bool,
    pub mime_filters: bool,
    pub multiple: bool,
    pub choices: bool,
    pub parent_window: bool,
}
pub const CAPABILITIES: PortalCapabilities = PortalCapabilities {
    glob_filters: true,
    mime_filters: true,
    // The protocol permits a backend to return fewer files than requested.
    // Accept multiple-selection requests so callers such as DPlayer can use
    // the chooser; the current browser UI returns its chosen file.
    multiple: true,
    choices: false,
    parent_window: false,
};
