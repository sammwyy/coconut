use std::path::Path;
#[cfg(target_os = "linux")]
use std::{ffi::CString, os::unix::ffi::OsStrExt};

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
            FilterRule::Glob(pattern) => glob_matches(pattern, path),
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

#[cfg(target_os = "linux")]
fn glob_matches(pattern: &str, path: &Path) -> bool {
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

#[cfg(not(target_os = "linux"))]
fn glob_matches(pattern: &str, path: &Path) -> bool {
    fn class(pattern: &[u8], start: usize, value: u8) -> Option<(bool, usize)> {
        let mut index = start + 1;
        let negated = pattern.get(index).is_some_and(|c| matches!(c, b'!' | b'^'));
        index += usize::from(negated);
        let mut matched = false;
        while index < pattern.len() && pattern[index] != b']' {
            if index + 2 < pattern.len() && pattern[index + 1] == b'-' && pattern[index + 2] != b']'
            {
                matched |= (pattern[index]..=pattern[index + 2]).contains(&value);
                index += 3;
            } else {
                matched |= pattern[index] == value;
                index += 1;
            }
        }
        (index < pattern.len()).then_some((if negated { !matched } else { matched }, index + 1))
    }

    fn matches(pattern: &[u8], text: &[u8], p: usize, t: usize, memo: &mut [Option<bool>]) -> bool {
        let index = p * (text.len() + 1) + t;
        if let Some(result) = memo[index] {
            return result;
        }
        let result = match pattern.get(p) {
            None => t == text.len(),
            Some(b'*') => {
                matches(pattern, text, p + 1, t, memo)
                    || t < text.len() && matches(pattern, text, p, t + 1, memo)
            }
            Some(b'?') => t < text.len() && matches(pattern, text, p + 1, t + 1, memo),
            Some(b'[') if t < text.len() => {
                class(pattern, p, text[t]).is_some_and(|(matched, next)| {
                    matched && matches(pattern, text, next, t + 1, memo)
                })
            }
            Some(b'\\') if p + 1 < pattern.len() && t < text.len() => {
                pattern[p + 1] == text[t] && matches(pattern, text, p + 2, t + 1, memo)
            }
            Some(value) => {
                t < text.len() && *value == text[t] && matches(pattern, text, p + 1, t + 1, memo)
            }
        };
        memo[index] = Some(result);
        result
    }

    let Some(name) = path.file_name() else {
        return false;
    };
    let pattern = pattern.as_bytes();
    let name = name.as_encoded_bytes();
    let mut memo = vec![None; (pattern.len() + 1) * (name.len() + 1)];
    matches(pattern, name, 0, 0, &mut memo)
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
