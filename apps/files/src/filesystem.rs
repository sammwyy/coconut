use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::CString,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::SystemTime,
};
use url::Url;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub directory: bool,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub permissions: u32,
}

impl Entry {
    pub fn name(&self) -> String {
        name(&self.path)
    }
    pub fn kind(&self) -> String {
        if self.directory {
            "Folder".into()
        } else {
            self.path
                .extension()
                .map(|ext| format!("{} file", ext.to_string_lossy().to_uppercase()))
                .unwrap_or_else(|| "File".into())
        }
    }
}

pub(crate) fn name(path: &Path) -> String {
    path.file_name()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub(crate) fn entries(path: &Path, show_hidden: bool) -> io::Result<Vec<Entry>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !show_hidden && entry.file_name().as_bytes().starts_with(b".") {
            continue;
        }
        // Follow symlinks so linked folders remain navigable.
        let metadata =
            fs::metadata(entry.path()).or_else(|_| fs::symlink_metadata(entry.path()))?;
        items.push(Entry {
            path: entry.path(),
            directory: metadata.is_dir(),
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            permissions: metadata.permissions().mode() & 0o777,
        });
    }
    items.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
    Ok(items)
}

#[derive(Clone)]
pub(crate) struct Place {
    pub label: String,
    pub icon: &'static str,
    pub path: PathBuf,
}

pub(crate) fn places() -> Vec<Place> {
    let home = home();
    let config = delib::xdg::XdgPaths::default().config_home;
    let dirs = fs::read_to_string(config.join("user-dirs.dirs")).unwrap_or_default();
    let mut result = vec![Place {
        label: "Home folder".into(),
        icon: "home",
        path: home.clone(),
    }];
    for (key, label, icon) in [
        ("DESKTOP", "Desktop", "desktop"),
        ("DOWNLOAD", "Downloads", "download"),
        ("DOCUMENTS", "Documents", "file"),
        ("PICTURES", "Pictures", "image"),
        ("VIDEOS", "Videos", "video"),
        ("MUSIC", "Music", "music"),
    ] {
        let prefix = format!("XDG_{key}_DIR=");
        let configured = dirs
            .lines()
            .find_map(|line| line.trim().strip_prefix(&prefix))
            .and_then(|v| parse_user_dir(v, &home));
        let path = configured.unwrap_or_else(|| home.join(label));
        if path != home && path.is_dir() {
            result.push(Place {
                label: label.into(),
                icon,
                path,
            });
        }
    }
    result
}

fn parse_user_dir(value: &str, home: &Path) -> Option<PathBuf> {
    let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    let value = value.replace("\\\"", "\"").replace("\\\\", "\\");
    let path = if value == "$HOME" {
        home.to_path_buf()
    } else if let Some(tail) = value.strip_prefix("$HOME/") {
        home.join(tail)
    } else {
        PathBuf::from(value)
    };
    path.is_absolute().then_some(path)
}

#[derive(Clone)]
pub(crate) struct Drive {
    pub label: String,
    pub path: PathBuf,
    pub space: Option<(u64, u64)>,
}

pub(crate) fn drives() -> Vec<Drive> {
    let mut paths = vec![PathBuf::from("/")];
    if let Ok(mounts) = fs::read_to_string("/proc/self/mountinfo") {
        for line in mounts.lines() {
            let Some((before, after)) = line.split_once(" - ") else {
                continue;
            };
            let fields: Vec<_> = before.split_whitespace().collect();
            let fs_fields: Vec<_> = after.split_whitespace().collect();
            if fields.len() < 5 || fs_fields.len() < 2 || !fs_fields[1].starts_with("/dev/") {
                continue;
            }
            let mount = fields[4]
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n")
                .replace("\\134", "\\");
            let path = PathBuf::from(mount);
            if ["/media", "/run/media", "/mnt"]
                .iter()
                .any(|base| path.starts_with(base))
                && !paths.contains(&path)
            {
                paths.push(path);
            }
        }
    }
    paths
        .into_iter()
        .map(|path| Drive {
            label: if path == Path::new("/") {
                "File system".into()
            } else {
                name(&path)
            },
            space: disk_space(&path),
            path,
        })
        .collect()
}

fn disk_space(path: &Path) -> Option<(u64, u64)> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // statvfs initializes the structure on success; the C string is alive throughout.
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some((
        stat.f_bavail.saturating_mul(stat.f_frsize),
        stat.f_blocks.saturating_mul(stat.f_frsize),
    ))
}

pub(crate) fn size(bytes: u64) -> String {
    for (divisor, suffix) in [
        (1_u64 << 40, "TB"),
        (1 << 30, "GB"),
        (1 << 20, "MB"),
        (1 << 10, "KB"),
    ] {
        if bytes >= divisor {
            return format!("{:.1} {suffix}", bytes as f64 / divisor as f64);
        }
    }
    format!("{bytes} B")
}

pub(crate) fn state_file(file: &str) -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("coconut")
        .join("cfiles")
        .join(file)
}

pub(crate) fn read_paths(file: &Path) -> Option<Vec<PathBuf>> {
    fs::read_to_string(file).ok().map(|text| {
        text.lines()
            .filter_map(|line| Url::parse(line).ok()?.to_file_path().ok())
            .collect()
    })
}

pub(crate) fn write_paths(file: &Path, paths: &[PathBuf]) -> io::Result<()> {
    fs::create_dir_all(file.parent().expect("state file parent"))?;
    let text: String = paths
        .iter()
        .filter_map(|path| Url::from_file_path(path).ok())
        .map(|url| format!("{url}\n"))
        .collect();
    let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, text)?;
    fs::rename(temporary, file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xdg_paths_are_parsed_without_executing_shell() {
        assert_eq!(
            parse_user_dir("\"$HOME/My Documents\"", Path::new("/users/me")),
            Some(PathBuf::from("/users/me/My Documents"))
        );
        assert_eq!(
            parse_user_dir("\"/data/photos\"", Path::new("/home/me")),
            Some(PathBuf::from("/data/photos"))
        );
        assert_eq!(
            parse_user_dir("\"$(touch /tmp/unsafe)\"", Path::new("/home/me")),
            None
        );
    }
}
