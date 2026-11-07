//! Optional hardware telemetry; missing driver counters stay unavailable.
use std::{
    io,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "linux")]
pub use platform::interface_addresses;
#[cfg(target_os = "windows")]
pub(crate) use platform::powershell;
#[cfg(target_os = "windows")]
use windows as platform;

#[derive(Clone, Default)]
pub struct Drive {
    pub name: String,
    pub model: String,
    pub kind: String,
    pub size: u64,
    pub read_rate: f32,
    pub write_rate: f32,
    pub usage: f32,
    pub sampled: bool,
    read_bytes: u64,
    write_bytes: u64,
    busy_ms: u64,
    direct_rates: bool,
}
#[derive(Clone, Default)]
pub struct Gpu {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub usage: Option<f32>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub temperature: Option<f32>,
}

pub fn os_name() -> String {
    platform::os_name()
}
pub fn drives() -> Vec<Drive> {
    platform::drives()
}
pub fn gpus() -> Vec<Gpu> {
    platform::gpus()
}

pub fn sample_drives(previous: &[Drive], next: &mut [Drive], elapsed: f32) {
    if elapsed <= 0.0 {
        return;
    }
    for drive in next {
        if drive.direct_rates {
            continue;
        }
        if let Some(old) = previous.iter().find(|old| old.name == drive.name) {
            drive.read_rate = drive.read_bytes.saturating_sub(old.read_bytes) as f32 / elapsed;
            drive.write_rate = drive.write_bytes.saturating_sub(old.write_bytes) as f32 / elapsed;
            drive.usage = (drive.busy_ms.saturating_sub(old.busy_ms) as f32 / (elapsed * 10.0))
                .clamp(0.0, 100.0);
            drive.sampled = true;
        }
    }
}

/// Bound background subprocesses so a failed service manager cannot stall polling.
pub fn output_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let read = |pipe: Option<Box<dyn io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = read(stdout.map(|pipe| Box::new(pipe) as Box<dyn io::Read + Send>));
    let stderr = read(stderr.map(|pipe| Box::new(pipe) as Box<dyn io::Read + Send>));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(15))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "The operation timed out",
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
        }
    };
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_rates_use_counter_deltas() {
        let old = Drive {
            name: "disk".into(),
            read_bytes: 1024,
            write_bytes: 2048,
            busy_ms: 100,
            ..Default::default()
        };
        let mut next = vec![Drive {
            name: "disk".into(),
            read_bytes: 5120,
            write_bytes: 4096,
            busy_ms: 600,
            ..Default::default()
        }];
        sample_drives(&[old], &mut next, 2.0);
        assert_eq!(next[0].read_rate, 2048.0);
        assert_eq!(next[0].write_rate, 1024.0);
        assert_eq!(next[0].usage, 25.0);
    }

    #[test]
    fn formatted_windows_rates_are_not_sampled_twice() {
        let previous = Drive {
            name: "disk".into(),
            read_bytes: 1024,
            ..Default::default()
        };
        let mut next = vec![Drive {
            name: "disk".into(),
            read_rate: 4096.0,
            write_rate: 2048.0,
            usage: 42.0,
            sampled: true,
            direct_rates: true,
            ..Default::default()
        }];
        sample_drives(&[previous], &mut next, 2.0);
        assert_eq!(next[0].read_rate, 4096.0);
        assert_eq!(next[0].write_rate, 2048.0);
        assert_eq!(next[0].usage, 42.0);
    }
}
