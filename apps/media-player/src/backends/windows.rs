use super::SpawnedDecoder;
use std::ffi::c_void;
use std::io::{self, Read};
use std::net::TcpListener;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PROCESS_TERMINATE: u32 = 0x0001;
const PROCESS_SUSPEND_RESUME: u32 = 0x0800;

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
    fn TerminateProcess(hProcess: *mut c_void, uExitCode: u32) -> i32;
    fn CloseHandle(hObject: *mut c_void) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(ProcessHandle: *mut c_void) -> i32;
    fn NtResumeProcess(ProcessHandle: *mut c_void) -> i32;
}

pub struct ProcessControl {
    pid: u32,
}

impl ProcessControl {
    fn with_handle(&self, access: u32, f: impl FnOnce(*mut c_void)) {
        unsafe {
            let handle = OpenProcess(access, 0, self.pid);
            if !handle.is_null() {
                f(handle);
                CloseHandle(handle);
            }
        }
    }

    pub fn suspend(&self) {
        self.with_handle(PROCESS_SUSPEND_RESUME, |handle| unsafe {
            NtSuspendProcess(handle);
        });
    }

    pub fn resume(&self) {
        self.with_handle(PROCESS_SUSPEND_RESUME, |handle| unsafe {
            NtResumeProcess(handle);
        });
    }

    pub fn kill(&self) {
        self.with_handle(PROCESS_TERMINATE, |handle| unsafe {
            TerminateProcess(handle, 1);
        });
    }
}

pub fn configure_command(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

pub fn init_env() {}

/// Lazily accepts the single loopback TCP connection opened by `ffmpeg.exe`
/// on the first `read()` call so video and audio listeners do not serialize startup.
struct DeferredTcpReader {
    accept: Option<thread::JoinHandle<io::Result<std::net::TcpStream>>>,
    stream: Option<std::net::TcpStream>,
}

impl DeferredTcpReader {
    fn new(listener: TcpListener) -> Self {
        let accept = thread::spawn(move || listener.accept().map(|(stream, _)| stream));
        Self {
            accept: Some(accept),
            stream: None,
        }
    }
}

impl Read for DeferredTcpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.stream.is_none() {
            let join = self.accept.take().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotConnected, "stream already closed")
            })?;
            let stream = join
                .join()
                .map_err(|_| io::Error::other("accept thread panicked"))??;
            self.stream = Some(stream);
        }
        self.stream.as_mut().unwrap().read(buf)
    }
}

pub fn spawn_ffmpeg<F>(
    has_video: bool,
    has_audio: bool,
    build_args: F,
) -> io::Result<SpawnedDecoder>
where
    F: FnOnce(&str, &str) -> Vec<String>,
{
    let video_listener = if has_video {
        Some(TcpListener::bind("127.0.0.1:0")?)
    } else {
        None
    };
    let audio_listener = if has_audio {
        Some(TcpListener::bind("127.0.0.1:0")?)
    } else {
        None
    };

    let video_dest = match &video_listener {
        Some(l) => format!("tcp://{}", l.local_addr()?),
        None => String::new(),
    };
    let audio_dest = match &audio_listener {
        Some(l) => format!("tcp://{}", l.local_addr()?),
        None => String::new(),
    };

    let args = build_args(&video_dest, &audio_dest);
    let mut command = Command::new("ffmpeg");
    configure_command(&mut command);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let child = command.spawn()?;
    let pid = child.id();

    Ok(SpawnedDecoder {
        child,
        control: ProcessControl { pid },
        video_reader: video_listener
            .map(|l| Box::new(DeferredTcpReader::new(l)) as Box<dyn Read + Send>),
        audio_reader: audio_listener
            .map(|l| Box::new(DeferredTcpReader::new(l)) as Box<dyn Read + Send>),
    })
}
