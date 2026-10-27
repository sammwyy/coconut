use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterRule {
    Glob(String),
    MimeType(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFilter {
    pub name: String,
    pub rules: Vec<FilterRule>,
}

impl FileFilter {
    pub fn matches(&self, path: &Path) -> bool {
        self.rules.iter().any(|rule| match rule {
            FilterRule::Glob(pattern) => {
                let Some(name) = path.file_name() else {
                    return false;
                };
                let (Ok(pattern), Ok(name)) = (
                    CString::new(pattern.as_bytes()),
                    CString::new(name.as_bytes()),
                ) else {
                    return false;
                };
                // Both C strings are valid and live for the duration of fnmatch.
                unsafe { libc::fnmatch(pattern.as_ptr(), name.as_ptr(), 0) == 0 }
            }
            // Extension-based MIME matching is deterministic and does not spawn
            // a process or read the contents of every file while browsing.
            FilterRule::MimeType(expected) => mime_guess::from_path(path).iter().any(|mime| {
                mime.essence_str() == expected
                    || expected
                        .strip_suffix("/*")
                        .is_some_and(|kind| mime.type_().as_str() == kind)
            }),
        })
    }

    pub fn wire(&self) -> (String, Vec<(u32, String)>) {
        (
            self.name.clone(),
            self.rules
                .iter()
                .map(|rule| match rule {
                    FilterRule::Glob(value) => (0, value.clone()),
                    FilterRule::MimeType(value) => (1, value.clone()),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_match_globs_mime_and_unicode_names() {
        let filter = FileFilter {
            name: "Documents".into(),
            rules: vec![
                FilterRule::Glob("*.[tT][xX][tT]".into()),
                FilterRule::MimeType("application/pdf".into()),
            ],
        };
        assert!(filter.matches(Path::new("/tmp/ñ notes.TXT")));
        assert!(filter.matches(Path::new("/tmp/report.pdf")));
        assert!(!filter.matches(Path::new("/tmp/image.png")));
        let images = FileFilter {
            name: "Images".into(),
            rules: vec![FilterRule::MimeType("image/*".into())],
        };
        assert!(images.matches(Path::new("/tmp/image.png")));
        assert!(!images.matches(Path::new("/tmp/unknown")));
    }
}
