use crate::state::PlayerState;
use creamui_render::AppHandle;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub(crate) enum Request {
    Open(Vec<PathBuf>),
    Quit,
}

pub(crate) struct Listener {
    socket: UnixListener,
    path: PathBuf,
    _lock: File,
}

impl Listener {
    pub(crate) fn cleanup(&self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn socket_path() -> io::Result<PathBuf> {
    let uid = unsafe { libc::geteuid() };
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| {
            fs::metadata(path).is_ok_and(|meta| {
                meta.is_dir() && meta.uid() == uid && meta.permissions().mode() & 0o077 == 0
            })
        });
    let dir = if let Some(runtime) = runtime {
        runtime
    } else {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| io::Error::other("HOME and XDG_RUNTIME_DIR are unavailable"))?;
        let dir = PathBuf::from(home).join(".cache/cplayer");
        let mut builder = fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.recursive(true).create(&dir)?;
        let meta = fs::metadata(&dir)?;
        if meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "insecure IPC directory",
            ));
        }
        dir
    };
    Ok(dir.join("cplayer-player.sock"))
}

fn send_to(path: &Path, request: &Request) -> io::Result<()> {
    let mut stream = UnixStream::connect(path)?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.write_all(&serde_json::to_vec(request)?)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut ack = [0];
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.read_exact(&mut ack)?;
    if ack[0] != 1 {
        return Err(io::Error::other("player rejected the request"));
    }
    Ok(())
}

pub(crate) fn send(request: Request) -> io::Result<()> {
    send_to(&socket_path()?, &request)
}

pub(crate) fn start(paths: &[PathBuf]) -> io::Result<Option<Arc<Listener>>> {
    let path = socket_path()?;
    let request = Request::Open(paths.to_vec());
    if send_to(&path, &request).is_ok() {
        return Ok(None);
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path.with_extension("lock"))?;
    let meta = lock.metadata()?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "insecure IPC lock",
        ));
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return send_to(&path, &request).map(|()| None);
    }
    if send_to(&path, &request).is_ok() {
        return Ok(None);
    }
    match UnixListener::bind(&path) {
        Ok(socket) => Ok(Some(Arc::new(Listener {
            socket,
            path,
            _lock: lock,
        }))),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            let meta = fs::symlink_metadata(&path)?;
            if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
                return Err(error);
            }
            fs::remove_file(&path)?;
            let socket = UnixListener::bind(&path)?;
            Ok(Some(Arc::new(Listener {
                socket,
                path,
                _lock: lock,
            })))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn listen(app: AppHandle, state: PlayerState, listener: Arc<Listener>) {
    let next = listener.clone();
    app.clone().spawn_background(
        move || {
            let (mut stream, _) = listener.socket.accept().ok()?;
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut bytes = Vec::new();
            let read = (&mut stream)
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .is_ok();
            let request = if read && bytes.len() <= 1_048_576 {
                serde_json::from_slice::<Request>(&bytes).ok()
            } else {
                None
            };
            let _ = stream.write_all(&[u8::from(request.is_some())]);
            Some(request)
        },
        move |request| {
            if let Some(request) = request {
                match request {
                    Some(Request::Open(paths)) => {
                        if !paths.is_empty() {
                            state.play_now(paths);
                        }
                        super::player::open_window(&state);
                    }
                    Some(Request::Quit) => state.app.exit(),
                    None => {}
                }
                listen(app, state, next);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_request_round_trips_over_local_socket() {
        let dir = std::env::temp_dir().join(format!("cplayer-ipc-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("player.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            let Request::Open(paths) = serde_json::from_slice(&bytes).unwrap() else {
                panic!("expected open request");
            };
            stream.write_all(&[1]).unwrap();
            paths
        });
        let paths = vec![PathBuf::from("/tmp/example.mp3")];
        send_to(&path, &Request::Open(paths.clone())).unwrap();
        assert_eq!(server.join().unwrap(), paths);
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
