use super::*;
use std::fs;

#[derive(Default)]
pub(super) struct Snapshot {
    pub(super) applications: Vec<DesktopEntry>,
    pub(super) copied: Vec<PathBuf>,
    pub(super) can_paste: bool,
}
pub(super) fn inspect(target: &FileMenuTarget) -> Result<Snapshot, String> {
    let metadata = fs::symlink_metadata(target.path()).map_err(|error| error.to_string())?;
    let applications = if matches!(
        target,
        FileMenuTarget::Item {
            directory: false,
            ..
        }
    ) {
        let mime = mime_guess::from_path(target.path()).first_or_octet_stream();
        MimeApps::default().applications_for(mime.essence_str())
    } else {
        Vec::new()
    };
    let copied = operations::copied_paths();
    let can_paste = !copied.is_empty() && metadata.is_dir() && !metadata.permissions().readonly();
    Ok(Snapshot {
        applications,
        copied,
        can_paste,
    })
}
pub(super) fn properties(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let kind = if metadata.is_symlink() {
        "Symbolic link"
    } else if metadata.is_dir() {
        "Folder"
    } else {
        "File"
    };
    let mut text = format!(
        "{}\n\nType: {kind}\nSize: {} bytes\nRead only: {}",
        path.display(),
        metadata.len(),
        metadata.permissions().readonly()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        text.push_str(&format!(
            "\nPermissions: {:03o}",
            metadata.permissions().mode() & 0o777
        ));
    }
    if let Ok(target) = fs::read_link(path) {
        text.push_str(&format!("\nTarget: {}", target.display()));
    }
    Ok(text)
}
