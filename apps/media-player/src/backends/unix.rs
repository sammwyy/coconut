use super::SpawnedDecoder;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

const VIDEO_FD: i32 = 3;
const AUDIO_FD: i32 = 4;

pub struct ProcessControl {
    pid: u32,
}

impl ProcessControl {
    pub fn suspend(&self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGSTOP);
        }
    }

    pub fn resume(&self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGCONT);
        }
    }

    pub fn kill(&self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGKILL);
        }
    }
}

pub fn configure_command(_cmd: &mut Command) {}

pub fn init_env() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some() {
        return;
    }
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    use std::os::unix::fs::FileTypeExt;
    let mut sockets = std::fs::read_dir(runtime)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_socket()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("wayland-"))
        .collect::<Vec<_>>();
    sockets.sort();
    if let Some(socket) = sockets.first() {
        std::env::set_var("WAYLAND_DISPLAY", socket);
    }
}

fn pipe() -> io::Result<(File, File)> {
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}

pub fn spawn_ffmpeg<F>(
    has_video: bool,
    has_audio: bool,
    build_args: F,
) -> io::Result<SpawnedDecoder>
where
    F: FnOnce(&str, &str) -> Vec<String>,
{
    let (video_read, video_write) = pipe()?;
    let (audio_read, audio_write) = pipe()?;
    let video_w = video_write.into_raw_fd();
    let audio_w = audio_write.into_raw_fd();

    let video_dest = format!("pipe:{VIDEO_FD}");
    let audio_dest = format!("pipe:{AUDIO_FD}");
    let args = build_args(&video_dest, &audio_dest);

    let mut command = Command::new("ffmpeg");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            // Avoid clobbering either pipe write end if its fd already
            // matches the other stream's target descriptor.
            let mut v = video_w;
            let mut a = audio_w;
            if v == AUDIO_FD {
                v = libc::fcntl(v, libc::F_DUPFD, AUDIO_FD + 1);
            }
            if a == VIDEO_FD {
                a = libc::fcntl(a, libc::F_DUPFD, AUDIO_FD + 1);
            }
            if has_video {
                libc::dup2(v, VIDEO_FD);
            }
            if has_audio {
                libc::dup2(a, AUDIO_FD);
            }
            Ok(())
        });
    }

    let child = command.spawn().inspect_err(|_| unsafe {
        libc::close(video_w);
        libc::close(audio_w);
    })?;

    // Close parent copies of the write descriptors so reader threads observe EOF when ffmpeg exits.
    unsafe {
        libc::close(video_w);
        libc::close(audio_w);
    }

    let pid = child.id();
    Ok(SpawnedDecoder {
        child,
        control: ProcessControl { pid },
        video_reader: has_video.then(|| Box::new(video_read) as Box<dyn Read + Send>),
        audio_reader: has_audio.then(|| Box::new(audio_read) as Box<dyn Read + Send>),
    })
}
