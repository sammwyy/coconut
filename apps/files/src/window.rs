//! Launch configuration shared by regular windows and portal dialogs.
use crate::portal::{PortalRequest, PortalType};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowMode {
    FileManager,
    Portal(PortalRequest),
}

#[derive(Clone, Debug)]
pub struct WindowRequest {
    pub title: String,
    pub start_directory: Option<PathBuf>,
    pub mode: WindowMode,
}

impl WindowRequest {
    pub fn manager(start_directory: Option<PathBuf>) -> Self {
        Self {
            title: "File Explorer".into(),
            start_directory,
            mode: WindowMode::FileManager,
        }
    }
    pub fn open(title: String, directories_only: bool) -> Self {
        Self {
            title,
            start_directory: None,
            mode: WindowMode::Portal(PortalRequest::new(if directories_only {
                PortalType::SelectFolder
            } else {
                PortalType::OpenFile
            })),
        }
    }
    pub fn save(title: String, name: String) -> Self {
        Self {
            title,
            start_directory: None,
            mode: WindowMode::Portal(PortalRequest::save(name)),
        }
    }
}
