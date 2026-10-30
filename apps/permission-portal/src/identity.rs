use std::path::{Path, PathBuf};
use zbus::{fdo, message::Header, Connection};

pub fn sibling(name: &str) -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|dir| dir.join(name))
}

pub async fn caller_pid(header: &Header<'_>, connection: &Connection) -> fdo::Result<u32> {
    let sender = header
        .sender()
        .ok_or_else(|| fdo::Error::AccessDenied("Missing D-Bus sender".into()))?;
    connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &sender.as_str(),
        )
        .await
        .and_then(|reply| reply.body().deserialize())
        .map_err(|error| fdo::Error::AccessDenied(format!("Cannot identify caller: {error}")))
}

pub fn executable_matches(pid: u32, expected: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = std::fs::metadata(format!("/proc/{pid}/exe"));
        let expected = std::fs::metadata(expected);
        matches!((actual, expected), (Ok(a), Ok(b)) if a.is_file() && a.dev() == b.dev() && a.ino() == b.ino())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, expected);
        false
    }
}

pub async fn require_executable(
    expected: &Path,
    header: &Header<'_>,
    connection: &Connection,
) -> fdo::Result<()> {
    let pid = caller_pid(header, connection).await?;
    if executable_matches(pid, expected) {
        Ok(())
    } else {
        Err(fdo::Error::AccessDenied(format!(
            "Only {} may call this method",
            expected.display()
        )))
    }
}

pub async fn require_owner(
    name: &str,
    header: &Header<'_>,
    connection: &Connection,
) -> fdo::Result<()> {
    let reply = connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &name,
        )
        .await
        .map_err(|error| fdo::Error::AccessDenied(error.to_string()))?;
    let owner: String = reply
        .body()
        .deserialize()
        .map_err(|error| fdo::Error::AccessDenied(error.to_string()))?;
    if header
        .sender()
        .is_some_and(|sender| sender.as_str() == owner)
    {
        Ok(())
    } else {
        Err(fdo::Error::AccessDenied(format!(
            "Only the owner of {name} may call this method"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_identity_rejects_a_copy_with_the_same_name() {
        let exe = std::env::current_exe().unwrap();
        assert!(executable_matches(std::process::id(), &exe));
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join(exe.file_name().unwrap());
        std::fs::copy(exe, &copy).unwrap();
        assert!(!executable_matches(std::process::id(), &copy));
    }
}
