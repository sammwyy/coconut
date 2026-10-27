//! Decode the D-Bus wire format before it reaches window or UI code.
use super::{FileFilter, FilterRule, PortalRequest, PortalType};
use crate::window::{WindowMode, WindowRequest};
use dbus::arg::{PropMap, RefArg};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

pub(super) type Error = (&'static str, String);
pub(super) fn invalid_arguments(error: impl std::fmt::Display) -> Error {
    ("org.freedesktop.DBus.Error.InvalidArgs", error.to_string())
}

pub(super) fn request(
    title: String,
    app_id: String,
    parent_window: String,
    save: bool,
    options: &PropMap,
) -> Result<WindowRequest, Error> {
    let directory = !save && boolean(options, "directory", false);
    let kind = if save {
        PortalType::SaveFile
    } else if directory {
        PortalType::SelectFolder
    } else {
        PortalType::OpenFile
    };
    let multiple = !save && boolean(options, "multiple", false);
    if options
        .get("choices")
        .and_then(|value| value.0.as_iter())
        .is_some_and(|mut values| values.next().is_some())
    {
        return Err((
            "org.freedesktop.DBus.Error.NotSupported",
            "Custom chooser options are not supported yet".into(),
        ));
    }
    let filters = match options.get("filters") {
        Some(value) => value
            .0
            .as_iter()
            .ok_or_else(|| invalid_arguments("Expected a filter array"))?
            .map(filter)
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let current_filter = options
        .get("current_filter")
        .map(|v| filter(v.0.as_ref()))
        .transpose()?;
    let current_file = path(options, "current_file");
    let suggested_name = string(options, "current_name")
        .or_else(|| {
            current_file
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| {
            if save {
                "untitled".into()
            } else {
                String::new()
            }
        });
    let request = PortalRequest {
        kind,
        app_id,
        parent_window,
        accept_label: string(options, "accept_label")
            .filter(|s| !s.is_empty())
            .map(|s| s.replace('_', "")),
        suggested_name,
        filters,
        current_filter,
        multiple,
        modal: boolean(options, "modal", true),
    };
    Ok(WindowRequest {
        title: if title.is_empty() {
            match kind {
                PortalType::SaveFile => "Save file",
                PortalType::SelectFolder => "Choose folder",
                PortalType::OpenFile => "Open file",
            }
            .into()
        } else {
            title
        },
        start_directory: path(options, "current_folder")
            .or_else(|| current_file.and_then(|p| p.parent().map(|p| p.to_path_buf()))),
        mode: WindowMode::Portal(request),
    })
}

fn filter(value: &dyn RefArg) -> Result<FileFilter, Error> {
    let mut parts = value
        .as_iter()
        .ok_or_else(|| invalid_arguments("Expected a filter tuple"))?;
    let name = parts
        .next()
        .and_then(RefArg::as_str)
        .ok_or_else(|| invalid_arguments("Expected a filter name"))?
        .to_owned();
    let patterns = parts
        .next()
        .and_then(RefArg::as_iter)
        .ok_or_else(|| invalid_arguments("Expected filter rules"))?;
    let mut rules = Vec::new();
    for pattern in patterns {
        let mut pair = pattern
            .as_iter()
            .ok_or_else(|| invalid_arguments("Expected a filter rule tuple"))?;
        let kind = pair
            .next()
            .and_then(RefArg::as_u64)
            .ok_or_else(|| invalid_arguments("Expected a filter rule kind"))?;
        let value = pair
            .next()
            .and_then(RefArg::as_str)
            .filter(|s| !s.contains('\0'))
            .ok_or_else(|| invalid_arguments("Expected a filter pattern"))?
            .to_owned();
        rules.push(match kind {
            0 => FilterRule::Glob(value),
            1 => FilterRule::MimeType(value),
            _ => return Err(invalid_arguments("Unknown filter rule kind")),
        });
    }
    if rules.is_empty() {
        return Err(invalid_arguments(
            "A file filter must have at least one rule",
        ));
    }
    Ok(FileFilter { name, rules })
}
fn boolean(options: &PropMap, key: &str, default: bool) -> bool {
    options
        .get(key)
        .and_then(|value| value.0.as_i64())
        .map(|v| v != 0)
        .unwrap_or(default)
}
fn string(options: &PropMap, key: &str) -> Option<String> {
    options
        .get(key)
        .and_then(|value| value.0.as_str())
        .map(str::to_owned)
}
// XDG paths are byte arrays; preserve non-UTF-8 paths and strip the final NUL.
pub(super) fn path(options: &PropMap, key: &str) -> Option<PathBuf> {
    let mut bytes = options
        .get(key)?
        .0
        .as_iter()?
        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect::<Option<Vec<_>>>()?;
    if bytes.last() == Some(&0) {
        bytes.pop();
    }
    if bytes.is_empty() || bytes.contains(&0) {
        return None;
    }
    let path = PathBuf::from(OsString::from_vec(bytes));
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::APPLICATION_NAMESPACE;
    use crate::portal::CAPABILITIES;
    use dbus::arg::Variant;

    #[test]
    fn request_keeps_filters_and_application_metadata() {
        let images = (
            "Images".to_string(),
            vec![(0_u32, "*.png".to_string()), (1, "image/jpeg".to_string())],
        );
        let mut options = PropMap::new();
        options.insert("filters".into(), Variant(Box::new(vec![images.clone()])));
        options.insert("current_filter".into(), Variant(Box::new(images.clone())));
        options.insert(
            "accept_label".into(),
            Variant(Box::new("_Attach".to_string())),
        );
        options.insert(
            "current_folder".into(),
            Variant(Box::new(b"/tmp/folder\xff\0".to_vec())),
        );
        // Round-trip through Message so tests exercise the real dynamic D-Bus types.
        let interface = format!("{APPLICATION_NAMESPACE}.Test");
        let message = dbus::Message::new_signal("/test", &interface, "Options")
            .unwrap()
            .append1(options);
        let options: PropMap = message.read1().unwrap();
        let decoded = request(
            "Pick a photo".into(),
            "org.test.App".into(),
            "wayland:parent".into(),
            false,
            &options,
        )
        .unwrap();
        assert_eq!(
            decoded.start_directory,
            Some(PathBuf::from(OsString::from_vec(
                b"/tmp/folder\xff".to_vec()
            )))
        );
        let WindowMode::Portal(portal) = decoded.mode else {
            panic!("expected chooser")
        };
        assert_eq!(portal.kind, PortalType::OpenFile);
        assert_eq!(portal.filters[0].wire(), images);
        assert_eq!(portal.current_filter, Some(portal.filters[0].clone()));
        assert_eq!(portal.app_id, "org.test.App");
        assert_eq!(portal.parent_window, "wayland:parent");
        assert_eq!(portal.accept_label.as_deref(), Some("Attach"));
    }
    #[test]
    fn accepts_multiple_requests_and_rejects_invalid_paths() {
        let mut options = PropMap::new();
        for bytes in [b"relative\0".to_vec(), b"/tmp/\0extra\0".to_vec(), vec![0]] {
            options.insert("current_folder".into(), Variant(Box::new(bytes)));
            assert!(path(&options, "current_folder").is_none());
        }
        options.insert("multiple".into(), Variant(Box::new(true)));
        let request = request("".into(), "".into(), "".into(), false, &options).unwrap();
        let WindowMode::Portal(portal) = request.mode else {
            panic!("expected chooser")
        };
        assert!(portal.multiple && CAPABILITIES.multiple);
    }
}
