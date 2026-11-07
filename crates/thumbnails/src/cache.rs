#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: u64 = 8192;
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy)]
struct Policy {
    bytes: u64,
    entries: u64,
    age: Duration,
}

const POLICY: Policy = Policy {
    bytes: MAX_BYTES,
    entries: MAX_ENTRIES,
    age: MAX_AGE,
};

#[derive(Default)]
struct Usage {
    bytes: u64,
    entries: u64,
    swept: u64,
    modified: u128,
}

pub(super) fn maintain(directory: &Path) -> io::Result<()> {
    let mut lock = lock(directory)?;
    let usage = usage(directory, &mut lock, POLICY)?;
    save(directory, &mut lock, &usage)
}

pub(super) fn store(directory: &Path, destination: &Path, bytes: &[u8]) -> io::Result<()> {
    store_with_policy(directory, destination, bytes, POLICY)
}

fn store_with_policy(
    directory: &Path,
    destination: &Path,
    bytes: &[u8],
    policy: Policy,
) -> io::Result<()> {
    let mut lock = lock(directory)?;
    let mut usage = usage(directory, &mut lock, policy)?;
    lock.set_len(0)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            fs::remove_file(destination)?;
            usage = sweep(directory, policy)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let length = bytes.len() as u64;
    if length > policy.bytes || policy.entries == 0 {
        return Ok(());
    }
    if usage.bytes.saturating_add(length) > policy.bytes || usage.entries >= policy.entries {
        // Batch eviction avoids rescanning a full directory on every cache miss.
        usage = sweep(
            directory,
            Policy {
                bytes: (policy.bytes * 3 / 4).min(policy.bytes - length),
                entries: (policy.entries * 3 / 4).min(policy.entries - 1),
                ..policy
            },
        )?;
    }
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    usage.bytes += length;
    usage.entries += 1;
    save(directory, &mut lock, &usage)
}

fn lock(directory: &Path) -> io::Result<File> {
    let mut name = directory
        .file_name()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?
        .to_os_string();
    name.push(".lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(directory.with_file_name(name))?;
    file.lock()?;
    Ok(file)
}

fn directory_modified(directory: &Path) -> io::Result<u128> {
    Ok(fs::metadata(directory)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos())
}

fn usage(directory: &Path, lock: &mut File, policy: Policy) -> io::Result<Usage> {
    let mut bytes = [0; 40];
    if lock.read_exact(&mut bytes).is_ok() {
        let usage = Usage {
            bytes: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            entries: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            swept: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            modified: u128::from_le_bytes(bytes[24..].try_into().unwrap()),
        };
        let age = now().checked_sub(usage.swept);
        // Directory timestamps invalidate accounting after a crashed or external writer.
        if usage.modified == directory_modified(directory)?
            && age.is_some_and(|age| age < SWEEP_INTERVAL.as_secs())
            && usage.bytes <= policy.bytes
            && usage.entries <= policy.entries
        {
            return Ok(usage);
        }
    }
    lock.set_len(0)?;
    sweep(directory, policy)
}

fn save(directory: &Path, lock: &mut File, usage: &Usage) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(40);
    bytes.extend_from_slice(&usage.bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.entries.to_le_bytes());
    bytes.extend_from_slice(&usage.swept.to_le_bytes());
    bytes.extend_from_slice(&directory_modified(directory)?.to_le_bytes());
    lock.rewind()?;
    lock.write_all(&bytes)?;
    lock.set_len(bytes.len() as u64)
}

fn sweep(directory: &Path, policy: Policy) -> io::Result<Usage> {
    let mut entries = BinaryHeap::new();
    let now = SystemTime::now();
    let mut usage = Usage::default();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.as_encoded_bytes();
        if name.len() != 64 || !name.iter().all(u8::is_ascii_hexdigit) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) if entry.file_type()?.is_file() => metadata,
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let modified = metadata.modified()?;
        if now.duration_since(modified).unwrap_or_default() >= policy.age
            || metadata.len() > policy.bytes
        {
            remove(&entry.path())?;
            continue;
        }
        usage.bytes = usage.bytes.saturating_add(metadata.len());
        usage.entries += 1;
        entries.push(Reverse((modified, entry.path(), metadata.len())));
        while usage.bytes > policy.bytes || usage.entries > policy.entries {
            let Reverse((_, path, length)) = entries.pop().expect("over-quota cache entry");
            remove(&path)?;
            usage.bytes = usage.bytes.saturating_sub(length);
            usage.entries -= 1;
        }
    }
    usage.swept = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    Ok(usage)
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::{fs::FileTimes, process::Command};

    fn directory() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("cache");
        fs::create_dir(&path).unwrap();
        (root, path)
    }

    fn policy(bytes: u64, entries: u64) -> Policy {
        Policy {
            bytes,
            entries,
            ..POLICY
        }
    }

    fn entry(directory: &Path, id: u32, bytes: usize, age: Duration) -> std::path::PathBuf {
        let path = directory.join(format!("{id:064x}"));
        fs::write(&path, vec![0; bytes]).unwrap();
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(SystemTime::now() - age))
            .unwrap();
        path
    }

    fn totals(directory: &Path) -> (u64, u64) {
        fs::read_dir(directory)
            .unwrap()
            .fold((0, 0), |(bytes, count), entry| {
                let metadata = entry.unwrap().metadata().unwrap();
                (bytes + metadata.len(), count + 1)
            })
    }

    #[test]
    fn byte_quota_keeps_recent_entries_and_leaves_room_for_new_previews() {
        let (_root, directory) = directory();
        let old = entry(directory.as_path(), 1, 60, Duration::from_secs(300));
        let recent = entry(directory.as_path(), 2, 60, Duration::from_secs(60));
        let new = directory.as_path().join(format!("{:064x}", 3));
        store_with_policy(directory.as_path(), &new, &[1; 60], policy(160, 8)).unwrap();
        assert!(!old.exists());
        assert!(recent.exists());
        assert_eq!(fs::read(&new).unwrap(), [1; 60]);
        assert_eq!(totals(directory.as_path()), (120, 2));
        store_with_policy(directory.as_path(), &new, &[2; 40], policy(160, 8)).unwrap();
        assert_eq!(totals(directory.as_path()), (100, 2));
    }

    #[test]
    fn count_quota_bounds_tiny_entries() {
        let (_root, directory) = directory();
        for id in 0..20 {
            let destination = directory.as_path().join(format!("{id:064x}"));
            store_with_policy(directory.as_path(), &destination, &[0], policy(4096, 4)).unwrap();
            assert!(totals(directory.as_path()).1 <= 4);
        }
    }

    #[test]
    fn legacy_cache_over_quota_retains_the_newest_previews() {
        let (_root, directory) = directory();
        for id in 0..120 {
            entry(
                directory.as_path(),
                id,
                40,
                Duration::from_secs(120 - id as u64),
            );
        }
        let destination = directory.join(format!("{:064x}", 120));
        store_with_policy(directory.as_path(), &destination, &[0; 40], policy(160, 4)).unwrap();
        assert_eq!(totals(directory.as_path()), (160, 4));
        for id in 117..=120 {
            assert!(directory.join(format!("{id:064x}")).exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn expiration_does_not_remove_unmanaged_files_or_follow_symlinks() {
        let (_root, directory) = directory();
        let expired = entry(
            directory.as_path(),
            1,
            60,
            MAX_AGE + Duration::from_secs(10),
        );
        let recent = entry(directory.as_path(), 2, 60, Duration::ZERO);
        let temporary = directory.as_path().join(".tmp-active");
        fs::write(&temporary, b"active writer").unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), b"keep").unwrap();
        let link = directory.as_path().join(format!("{:064x}", 3));
        symlink(outside.path(), &link).unwrap();
        let nested = directory.as_path().join(format!("{:064x}", 4));
        fs::create_dir(&nested).unwrap();
        maintain(directory.as_path()).unwrap();
        assert!(!expired.exists());
        assert!(recent.exists());
        assert!(temporary.exists());
        assert!(nested.is_dir());
        assert!(link.is_symlink());
        assert_eq!(fs::read(outside.path()).unwrap(), b"keep");
    }

    #[test]
    fn external_writes_and_corrupt_accounting_are_reconciled() {
        let (_root, directory) = directory();
        let destination = directory.as_path().join(format!("{:064x}", 1));
        store_with_policy(directory.as_path(), &destination, &[0; 60], policy(160, 8)).unwrap();
        entry(directory.as_path(), 2, 100, Duration::from_secs(60));
        let new = directory.as_path().join(format!("{:064x}", 3));
        store_with_policy(directory.as_path(), &new, &[0; 60], policy(160, 8)).unwrap();
        assert!(totals(directory.as_path()).0 <= 160);
        let mut lock = lock(directory.as_path()).unwrap();
        lock.set_len(0).unwrap();
        lock.write_all(b"partial write").unwrap();
        drop(lock);
        store_with_policy(directory.as_path(), &new, &[0; 60], policy(160, 8)).unwrap();
        assert!(totals(directory.as_path()).0 <= 160);
    }

    #[test]
    fn repairing_corrupt_entries_preserves_the_shared_byte_quota() {
        let (_root, directory) = directory();
        let first = directory.join(format!("{:064x}", 1));
        let second = directory.join(format!("{:064x}", 2));
        let third = directory.join(format!("{:064x}", 3));
        let fourth = directory.join(format!("{:064x}", 4));
        let policy = policy(160, 8);
        store_with_policy(directory.as_path(), &first, &[0; 40], policy).unwrap();
        store_with_policy(directory.as_path(), &second, &[0; 40], policy).unwrap();
        fs::write(&first, [0; 160]).unwrap();
        store_with_policy(directory.as_path(), &first, &[0; 40], policy).unwrap();
        store_with_policy(directory.as_path(), &third, &[0; 80], policy).unwrap();
        store_with_policy(directory.as_path(), &fourth, &[0; 40], policy).unwrap();
        assert!(totals(directory.as_path()).0 <= 160);
        assert_eq!(fs::read(fourth).unwrap(), [0; 40]);
    }

    #[test]
    fn independent_processes_share_one_quota() {
        let (_root, directory) = directory();
        let mut children = (0..4)
            .map(|id| {
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "cache::tests::cache_writer_process", "--ignored"])
                    .env("COCONUT_TEST_THUMBNAIL_CACHE", directory.as_path())
                    .env("COCONUT_TEST_THUMBNAIL_WRITER", id.to_string())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for child in &mut children {
            assert!(child.wait().unwrap().success());
        }
        let (bytes, count) = totals(directory.as_path());
        assert!(bytes <= 160);
        assert!(count <= 4);
    }

    #[test]
    #[ignore = "subprocess fixture"]
    fn cache_writer_process() {
        let directory = std::env::var_os("COCONUT_TEST_THUMBNAIL_CACHE").unwrap();
        let writer: u32 = std::env::var("COCONUT_TEST_THUMBNAIL_WRITER")
            .unwrap()
            .parse()
            .unwrap();
        for index in 0..20 {
            let directory = Path::new(&directory);
            let destination = directory.join(format!("{:064x}", writer * 20 + index));
            store_with_policy(directory, &destination, &[0; 40], policy(160, 4)).unwrap();
        }
    }
}
