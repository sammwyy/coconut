use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};
use url::Url;

static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

fn clipboard<T>(
    action: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, String> {
    let mut clipboard = CLIPBOARD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|error| error.to_string())?;
    if clipboard.is_none() {
        *clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
    }
    action(clipboard.as_mut().expect("initialized clipboard")).map_err(|error| error.to_string())
}
pub(crate) fn copied_paths() -> Vec<PathBuf> {
    clipboard(|clipboard| clipboard.get_text())
        .ok()
        .and_then(|text| parse_paths(&text))
        .unwrap_or_default()
}
fn parse_paths(text: &str) -> Option<Vec<PathBuf>> {
    let paths: Option<Vec<_>> = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| Url::parse(line).ok()?.to_file_path().ok())
        .collect();
    paths.filter(|paths| !paths.is_empty())
}
pub(crate) fn copy_path(path: &Path) -> Result<(), String> {
    let uri = Url::from_file_path(path).map_err(|_| "This path cannot be copied.".to_string())?;
    clipboard(|clipboard| clipboard.set_text(format!("{uri}\r\n")))
}
pub(crate) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && !matches!(name, "." | "..") && !name.contains(['/', '\0'])
}
pub(crate) fn create(directory: &Path, name: &str, folder: bool) -> io::Result<()> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Enter a file name without /, NUL, . or ..",
        ));
    }
    let path = directory.join(name);
    if folder {
        fs::create_dir(path)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ())
    }
}
pub(crate) fn rename(path: &Path, name: &str) -> io::Result<()> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Enter a valid file name",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot rename the filesystem root",
        )
    })?;
    let destination = parent.join(name);
    if path == destination {
        return Ok(());
    }
    rename_exclusive(path, &destination)
}
#[cfg(target_os = "linux")]
fn rename_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // RENAME_NOREPLACE keeps collision handling atomic with the rename.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(target_os = "linux"))]
fn rename_exclusive(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Exclusive rename is unavailable on this platform",
    ))
}
pub(crate) fn paste(sources: &[PathBuf], directory: &Path) -> io::Result<()> {
    let directory = fs::canonicalize(directory)?;
    for source in sources {
        let metadata = fs::symlink_metadata(source)?;
        if metadata.is_dir() && directory.starts_with(fs::canonicalize(source)?) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot copy a folder into itself",
            ));
        }
        let name = source.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot copy the filesystem root",
            )
        })?;
        copy_entry(source, &directory.join(name))?;
    }
    Ok(())
}
fn copy_entry(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_symlink() {
        #[cfg(unix)]
        {
            return std::os::unix::fs::symlink(fs::read_link(source)?, destination);
        }
        #[cfg(not(unix))]
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Copying symbolic links is unavailable",
            ));
        }
    }
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions())
    } else if metadata.is_file() {
        let mut input = fs::File::open(source)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        io::copy(&mut input, &mut output)?;
        fs::set_permissions(destination, metadata.permissions())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Only regular files, folders and symbolic links can be copied",
        ))
    }
}
pub(crate) fn launch(mut command: Command) -> Result<(), String> {
    let mut child =
        coconut_core::spawn_detached(&mut command).map_err(|error| error.to_string())?;
    std::thread::spawn(move || {
        if let Err(error) = child.wait() {
            eprintln!("file-menu: could not reap application: {error}");
        }
    });
    Ok(())
}
pub(crate) fn launch_application(
    entry: &delib::desktop_entry::DesktopEntry,
    path: &Path,
) -> Result<(), String> {
    let argv = entry
        .command_for_files(&[path.to_path_buf()])
        .ok_or("The application has no launch command")?;
    let (program, args) = argv
        .split_first()
        .ok_or("The application has an empty launch command")?;
    let mut command = if entry.terminal() {
        coconut_core::terminal_command(program, args)
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    if let Some(directory) = entry.value("Path").filter(|value| !value.is_empty()) {
        command.current_dir(directory);
    }
    launch(command)
}
pub(crate) fn open_terminal(directory: &Path) -> Result<(), String> {
    let mut command = Command::new("cterm");
    command.arg("--working-directory").arg(directory);
    launch(command)
}
pub fn open_in_explorer(path: &Path) -> Result<(), String> {
    let command = coconut_core::files_command(path).ok_or("File Explorer is not installed")?;
    launch(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_and_rename_preserve_existing_entries() {
        let root = tempfile::tempdir().unwrap();
        create(root.path(), "notes", false).unwrap();
        fs::write(root.path().join("notes"), b"original").unwrap();
        assert!(create(root.path(), "notes", false).is_err());
        create(root.path(), "other", false).unwrap();
        assert!(rename(&root.path().join("other"), "notes").is_err());
        assert_eq!(fs::read(root.path().join("notes")).unwrap(), b"original");
        for name in ["../escape", ".", "..", "", "a\0b"] {
            assert!(create(root.path(), name, false).is_err());
        }
    }
    #[test]
    fn paste_refuses_overwrites_and_recursive_destinations() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        fs::create_dir(&folder).unwrap();
        let child = folder.join("child");
        fs::create_dir(&child).unwrap();
        assert!(paste(&[folder.clone()], &child).is_err());
        assert!(paste(&[folder.clone()], root.path()).is_err());
        let target = root.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(folder.join("notes"), b"text").unwrap();
        paste(&[folder], &target).unwrap();
        assert_eq!(fs::read(target.join("folder/notes")).unwrap(), b"text");
    }
    #[test]
    fn clipboard_accepts_local_uris_without_interpreting_commands() {
        assert_eq!(
            parse_paths("file:///tmp/hello%20world\r\n"),
            Some(vec![PathBuf::from("/tmp/hello world")])
        );
        assert!(parse_paths("https://example.com/file").is_none());
        assert!(parse_paths("$(touch /tmp/unsafe)").is_none());
    }
    #[cfg(unix)]
    #[test]
    fn paste_preserves_links_without_following_them() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("link");
        std::os::unix::fs::symlink("missing", &source).unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        paste(&[source], &destination).unwrap();
        assert_eq!(
            fs::read_link(destination.join("link")).unwrap(),
            PathBuf::from("missing")
        );
    }
}
