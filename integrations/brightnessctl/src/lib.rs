use coconut_api::brightness::BrightnessIntegration;
use coconut_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
enum Backend {
    BrightnessCtl,
    Ddc,
}

pub struct BrightnessCtl {
    backend: Backend,
    level: Arc<Mutex<f32>>,
    changes: Option<ChangeListener>,
    _bridge: Option<EventBridgeGuard>,
}

impl BrightnessCtl {
    pub fn detect() -> Option<Self> {
        if output("brightnessctl", &["get"]).is_some() {
            return Some(Self::new(Backend::BrightnessCtl));
        }
        // DDC is useful for external displays where the kernel backlight does
        // not exist.  `getvcp` also verifies that a monitor is reachable.
        output("ddcutil", &["getvcp", "10", "--terse"])
            .is_some()
            .then(|| Self::new(Backend::Ddc))
    }

    fn new(backend: Backend) -> Self {
        let level = Arc::new(Mutex::new(level_for(&backend).unwrap_or(1.0)));
        let sysfs_path = matches!(backend, Backend::BrightnessCtl).then(backlight_sysfs_path);
        let Some(Some(path)) = sysfs_path else {
            spawn_poll(level.clone(), backend.clone());
            return Self {
                backend,
                level,
                changes: None,
                _bridge: None,
            };
        };
        // The kernel backlight driver posts a sysfs change notification on
        // every write to `brightness` (its own or another program's), so an
        // inotify watch on that file is a genuine hardware-level hook rather
        // than a timer guessing when to re-read it.
        let cache = level.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = watch_backlight(&path, &cache, &change_tx, &shutdown_rx) {
                eprintln!("brightness: {error}");
            }
        });
        Self {
            backend,
            level,
            changes: Some(changes),
            _bridge: Some(bridge),
        }
    }

    fn ddc_value() -> Option<(f32, f32)> {
        let line = output("ddcutil", &["getvcp", "10", "--terse"])?;
        let numbers: Vec<f32> = line
            .split_whitespace()
            .filter_map(|item| item.trim_matches(',').parse().ok())
            .collect();
        // `VCP 10 C current max` is the terse format.
        (numbers.len() >= 2).then(|| (numbers[numbers.len() - 2], numbers[numbers.len() - 1]))
    }
}

impl BrightnessIntegration for BrightnessCtl {
    fn level(&self) -> f32 {
        self.level.lock().map(|level| *level).unwrap_or(1.0)
    }

    fn set_level(&self, level: f32) {
        let level = level.clamp(0.0, 1.0);
        if let Ok(mut current) = self.level.lock() {
            *current = level;
        }
        let backend = self.backend.clone();
        thread::spawn(move || match backend {
            Backend::BrightnessCtl => {
                let _ = output(
                    "brightnessctl",
                    &["set", &format!("{}%", (level * 100.0).round())],
                );
            }
            Backend::Ddc => {
                if let Some((_, max)) = Self::ddc_value() {
                    let _ = output(
                        "ddcutil",
                        &["setvcp", "10", &((level * max).round() as u32).to_string()],
                    );
                }
            }
        });
    }

    fn changes(&self) -> Option<ChangeListener> {
        self.changes.clone()
    }
}

fn spawn_poll(level: Arc<Mutex<f32>>, backend: Backend) {
    thread::spawn(move || loop {
        if let Some(next) = level_for(&backend) {
            if let Ok(mut current) = level.lock() {
                *current = next;
            }
        }
        thread::sleep(Duration::from_secs(2));
    });
}

/// Resolves the sysfs directory `brightnessctl` is reading, e.g.
/// `/sys/class/backlight/amdgpu_bl1`, so it can be watched directly.
fn backlight_sysfs_path() -> Option<PathBuf> {
    let info = output("brightnessctl", &["-m", "i"])?;
    let mut fields = info.split(',');
    let device = fields.next()?;
    let class = fields.next()?;
    let path = PathBuf::from("/sys/class").join(class).join(device);
    path.join("brightness").is_file().then_some(path)
}

fn watch_backlight(
    path: &Path,
    level: &Arc<Mutex<f32>>,
    changes: &SyncSender<()>,
    shutdown: &Receiver<()>,
) -> Result<(), String> {
    let brightness_file = path.join("brightness");
    let watched_path =
        CString::new(brightness_file.as_os_str().as_bytes()).map_err(|error| error.to_string())?;

    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let watch = unsafe {
        libc::inotify_add_watch(
            fd,
            watched_path.as_ptr(),
            libc::IN_MODIFY | libc::IN_CLOSE_WRITE,
        )
    };
    if watch < 0 {
        let error = std::io::Error::last_os_error().to_string();
        unsafe { libc::close(fd) };
        return Err(error);
    }

    let mut buffer = [0u8; 512];
    loop {
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll_fd, 1, 500) };
        if ready > 0 && poll_fd.revents & libc::POLLIN != 0 {
            let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
            if read > 0 {
                if let Some(next) = level_for(&Backend::BrightnessCtl) {
                    if let Ok(mut current) = level.lock() {
                        *current = next;
                    }
                }
                let _ = changes.try_send(());
            }
        }
    }
    unsafe { libc::close(fd) };
    Ok(())
}

fn level_for(backend: &Backend) -> Option<f32> {
    match backend {
        Backend::BrightnessCtl => {
            let current = output("brightnessctl", &["get"]).and_then(|v| v.parse::<f32>().ok());
            let maximum = output("brightnessctl", &["max"]).and_then(|v| v.parse::<f32>().ok());
            current.zip(maximum).map(|(current, max)| current / max)
        }
        Backend::Ddc => BrightnessCtl::ddc_value().map(|(current, max)| current / max),
    }
    .map(|level| level.clamp(0.0, 1.0))
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    Command::new("timeout")
        .args(["2s", program])
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{BrightnessCtl, BrightnessIntegration};
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a kernel-backlight device controlled by brightnessctl"]
    fn reacts_to_an_external_brightness_change() {
        let brightness = BrightnessCtl::detect().expect("a brightness backend");
        sleep(Duration::from_millis(200));
        let listener = brightness
            .changes()
            .expect("a native inotify hook for the kernel backlight");
        let initial = brightness.level();
        let target = if initial > 0.5 {
            initial - 0.1
        } else {
            initial + 0.1
        };

        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = result_tx.send(listener.wait());
        });
        let _ = Command::new("brightnessctl")
            .args(["set", &format!("{}%", (target * 100.0).round())])
            .status();
        assert_eq!(result_rx.recv_timeout(Duration::from_secs(5)), Ok(true));

        let _ = Command::new("brightnessctl")
            .args(["set", &format!("{}%", (initial * 100.0).round())])
            .status();
    }
}
