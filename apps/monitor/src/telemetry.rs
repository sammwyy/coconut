//! Optional Linux hardware telemetry; missing driver counters stay unavailable.
use std::{
    collections::HashMap,
    fs, io,
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
    process::{Command, Output, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

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
fn text(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().into())
}
fn number(path: impl AsRef<Path>) -> Option<u64> {
    text(path)?.parse().ok()
}

pub fn os_name() -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        text("/etc/os-release")
            .and_then(|s| {
                s.lines().find_map(|line| {
                    line.strip_prefix("PRETTY_NAME=")
                        .map(|s| s.trim_matches('"').to_string())
                })
            })
            .unwrap_or_else(|| "Linux".into())
    })
    .clone()
}
pub fn drives() -> Vec<Drive> {
    let mut result = vec![];
    for entry in fs::read_dir("/sys/block").into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if ["loop", "ram", "zram"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        let base = entry.path();
        let counters: Vec<u64> = text(base.join("stat"))
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        if counters.len() < 10 {
            continue;
        }
        result.push(Drive {
            name,
            model: text(base.join("device/model")).unwrap_or_else(|| "Block device".into()),
            kind: if number(base.join("queue/rotational")) == Some(1) {
                "HDD"
            } else {
                "SSD / virtual"
            }
            .into(),
            size: number(base.join("size")).unwrap_or(0).saturating_mul(512),
            read_bytes: counters[2].saturating_mul(512),
            write_bytes: counters[6].saturating_mul(512),
            busy_ms: counters[9],
            ..Default::default()
        });
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}
pub fn sample_drives(previous: &[Drive], next: &mut [Drive], elapsed: f32) {
    if elapsed <= 0.0 {
        return;
    }
    for drive in next {
        if let Some(old) = previous.iter().find(|old| old.name == drive.name) {
            drive.read_rate = drive.read_bytes.saturating_sub(old.read_bytes) as f32 / elapsed;
            drive.write_rate = drive.write_bytes.saturating_sub(old.write_bytes) as f32 / elapsed;
            drive.usage = (drive.busy_ms.saturating_sub(old.busy_ms) as f32 / (elapsed * 10.0))
                .clamp(0.0, 100.0);
            drive.sampled = true;
        }
    }
}
pub fn gpus() -> Vec<Gpu> {
    let mut result = vec![];
    for entry in fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
    {
        let id = entry.file_name().to_string_lossy().into_owned();
        if !id
            .strip_prefix("card")
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let base = entry.path().join("device");
        if !base.exists() {
            continue;
        }
        let driver = fs::read_link(base.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Unknown driver".into());
        let vendor = text(base.join("vendor")).unwrap_or_default();
        let device = text(base.join("device")).unwrap_or_default();
        let name = pci_name(&vendor, &device).unwrap_or_else(|| {
            format!(
                "{} graphics",
                match vendor.as_str() {
                    "0x1002" => "AMD",
                    "0x8086" => "Intel",
                    "0x10de" => "NVIDIA",
                    _ => "GPU",
                }
            )
        });
        let temperature = fs::read_dir(base.join("hwmon"))
            .into_iter()
            .flatten()
            .flatten()
            .find_map(|entry| number(entry.path().join("temp1_input")))
            .map(|v| v as f32 / 1000.0);
        result.push(Gpu {
            id,
            name,
            driver,
            usage: number(base.join("gpu_busy_percent")).map(|n| (n as f32).clamp(0.0, 100.0)),
            memory_used: number(base.join("mem_info_vram_used")),
            memory_total: number(base.join("mem_info_vram_total")),
            temperature,
        });
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}
fn pci_name(vendor: &str, device: &str) -> Option<String> {
    static IDS: OnceLock<String> = OnceLock::new();
    let ids = IDS.get_or_init(|| {
        text("/usr/share/hwdata/pci.ids")
            .or_else(|| text("/usr/share/misc/pci.ids"))
            .unwrap_or_default()
    });
    let vendor = vendor.trim_start_matches("0x");
    let device = device.trim_start_matches("0x");
    let mut found = false;
    for line in ids.lines() {
        if !line.starts_with(['\t', '#']) && !line.is_empty() {
            found = line.get(..4) == Some(vendor);
        }
        if found
            && line.starts_with('\t')
            && !line.starts_with("\t\t")
            && line.get(1..5) == Some(device)
        {
            return Some(line[5..].trim().into());
        }
    }
    None
}
pub fn interface_addresses() -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    let mut first = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut first) } != 0 {
        return result;
    }
    let mut cursor = first;
    while !cursor.is_null() {
        let entry = unsafe { &*cursor };
        if !entry.ifa_addr.is_null() {
            let address = unsafe {
                match (*entry.ifa_addr).sa_family as i32 {
                    libc::AF_INET => Some(
                        Ipv4Addr::from(
                            (*(entry.ifa_addr as *const libc::sockaddr_in))
                                .sin_addr
                                .s_addr
                                .to_ne_bytes(),
                        )
                        .to_string(),
                    ),
                    libc::AF_INET6 => Some(
                        Ipv6Addr::from(
                            (*(entry.ifa_addr as *const libc::sockaddr_in6))
                                .sin6_addr
                                .s6_addr,
                        )
                        .to_string(),
                    ),
                    _ => None,
                }
            };
            if let Some(address) = address {
                let name = unsafe { std::ffi::CStr::from_ptr(entry.ifa_name) }
                    .to_string_lossy()
                    .into_owned();
                result.entry(name).or_default().push(address);
            }
        }
        cursor = entry.ifa_next;
    }
    unsafe { libc::freeifaddrs(first) };
    result
}

/// Bound background subprocesses so a failed service manager cannot stall polling.
pub fn output_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
}
