use super::{Drive, Gpu};
use std::{
    collections::HashMap,
    fs,
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
    sync::OnceLock,
};

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
