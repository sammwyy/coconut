//! Per-window hooks. A manager and a chooser can share every browser component.
use crate::{
    portal::{FileFilter, PortalCapabilities, PortalRequest, PortalType, CAPABILITIES},
    window::{WindowMode, WindowRequest},
};
use creamui_widgets::SelectController;
use std::path::Path;

#[derive(Clone)]
pub struct WindowContext {
    pub request: WindowRequest,
    pub filter: SelectController,
}
impl WindowContext {
    pub fn new(mut request: WindowRequest) -> Self {
        let mut index = 0;
        if let WindowMode::Portal(portal) = &mut request.mode {
            if let Some(current) = &portal.current_filter {
                index = portal
                    .filters
                    .iter()
                    .position(|filter| filter == current)
                    .unwrap_or_else(|| {
                        portal.filters.push(current.clone());
                        portal.filters.len() - 1
                    });
            }
        }
        Self {
            request,
            filter: SelectController::new(index),
        }
    }
    pub fn portal(&self) -> Option<PortalContext> {
        match &self.request.mode {
            WindowMode::FileManager => None,
            WindowMode::Portal(request) => Some(PortalContext {
                request: request.clone(),
                filter: self.filter.clone(),
            }),
        }
    }
    pub fn portal_type(&self) -> Option<PortalType> {
        self.portal().map(|p| p.request.kind)
    }
    pub fn is_manager(&self) -> bool {
        self.portal_type().is_none()
    }
    pub fn is_save(&self) -> bool {
        self.portal_type() == Some(PortalType::SaveFile)
    }
    pub fn directories_only(&self) -> bool {
        self.portal_type() == Some(PortalType::SelectFolder)
    }
    pub fn accepts(&self, path: &Path, directory: bool) -> bool {
        self.portal().is_none_or(|p| p.accepts(path, directory))
    }
}

#[derive(Clone)]
pub struct PortalContext {
    pub request: PortalRequest,
    pub filter: SelectController,
}
impl PortalContext {
    pub fn capabilities(&self) -> PortalCapabilities {
        CAPABILITIES
    }
    pub fn current_filter(&self) -> Option<FileFilter> {
        if self.request.kind == PortalType::SelectFolder {
            return None;
        }
        self.request.filters.get(self.filter.selected()).cloned()
    }
    pub fn accepts(&self, path: &Path, directory: bool) -> bool {
        directory
            || (self.request.kind != PortalType::SelectFolder
                && self.current_filter().is_none_or(|f| f.matches(path)))
    }
    pub fn accept_label(&self) -> String {
        self.request.accept_label.clone().unwrap_or_else(|| {
            match self.request.kind {
                PortalType::OpenFile => "Open",
                PortalType::SelectFolder => "Choose folder",
                PortalType::SaveFile => "Save",
            }
            .into()
        })
    }
}

/// Returns the chooser's type, filters, selected filter and request metadata,
/// or None when the current window is the regular file manager.
pub fn use_is_portal() -> Option<PortalContext> {
    creamui_reactive::try_use_context::<WindowContext>().and_then(|context| context.portal())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::FilterRule;
    #[test]
    fn hook_is_window_scoped_and_exposes_live_filter_state() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(WindowContext::new(WindowRequest::manager(None)));
            assert!(use_is_portal().is_none());
            creamui_reactive::with_context_scope(|| {
                let images = FileFilter {
                    name: "Images".into(),
                    rules: vec![FilterRule::Glob("*.png".into())],
                };
                let text = FileFilter {
                    name: "Text".into(),
                    rules: vec![FilterRule::MimeType("text/plain".into())],
                };
                let mut request = PortalRequest::new(PortalType::OpenFile);
                request.filters = vec![images.clone(), text.clone()];
                request.current_filter = Some(text.clone());
                request.accept_label = Some("Attach".into());
                let context = WindowContext::new(WindowRequest {
                    title: "Choose".into(),
                    start_directory: None,
                    mode: WindowMode::Portal(request),
                });
                creamui_reactive::provide_context(context);
                let portal = use_is_portal().unwrap();
                assert_eq!(portal.request.kind, PortalType::OpenFile);
                assert_eq!(portal.current_filter(), Some(text));
                assert_eq!(portal.accept_label(), "Attach");
                assert!(portal.accepts(Path::new("notes.txt"), false));
                assert!(!portal.accepts(Path::new("photo.png"), false));
                assert!(portal.accepts(Path::new("folder"), true));
                portal.filter.select(0);
                assert_eq!(use_is_portal().unwrap().current_filter(), Some(images));
                assert!(portal.accepts(Path::new("photo.png"), false));
            });
            assert!(
                use_is_portal().is_none(),
                "a chooser must not leak into another window"
            );
        });
    }
    #[test]
    fn standalone_current_filter_is_available_in_the_hud() {
        let filter = FileFilter {
            name: "Text".into(),
            rules: vec![FilterRule::Glob("*.txt".into())],
        };
        let mut portal = PortalRequest::save("report.txt");
        portal.current_filter = Some(filter.clone());
        let context = WindowContext::new(WindowRequest {
            title: "Save".into(),
            start_directory: None,
            mode: WindowMode::Portal(portal),
        });
        assert_eq!(context.portal().unwrap().current_filter(), Some(filter));
        assert_eq!(context.portal().unwrap().request.filters.len(), 1);
    }
}
