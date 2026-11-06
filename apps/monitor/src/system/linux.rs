use super::*;
use std::{fs, path::Path, process::Command};

pub const SERVICE_SCOPES: &[(bool, &str)] = &[(false, "System"), (true, "User")];
pub const SOCKET_ID_LABEL: &str = "Inode";

pub fn collect() -> Snapshot {
    Snapshot {
        os_name: crate::telemetry::os_name(),
        architecture: std::env::consts::ARCH.into(),
        drives: crate::telemetry::drives(),
        gpus: crate::telemetry::gpus(),
        hostname: text("/proc/sys/kernel/hostname").unwrap_or_else(|| "This computer".into()),
        kernel: text("/proc/sys/kernel/osrelease").unwrap_or_else(|| "Linux".into()),
        uptime: text("/proc/uptime")
            .and_then(|v| v.split_whitespace().next()?.parse().ok())
            .unwrap_or(0.0) as u64,
        cpu: cpu(),
        memory: memory(),
        processes: processes(),
        disks: disks(),
        network: network(),
        sockets: sockets(),
        temperatures: temperatures(),
        battery: battery(),
        services: services(false),
        user_services: services(true),
        ..Default::default()
    }
}

pub fn current_user() -> String {
    unsafe { libc::geteuid() }.to_string()
}

pub fn clock_ticks() -> u64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK).max(1) as u64 }
}

fn text(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}
fn cpu() -> Cpu {
    let info = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = info
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name\t: ")
                .or_else(|| l.strip_prefix("Hardware\t: "))
        })
        .unwrap_or("Processor")
        .to_string();
    let cores = info
        .lines()
        .filter(|l| l.starts_with("processor\t"))
        .count()
        .max(1);
    let loads = text("/proc/loadavg")
        .unwrap_or_default()
        .split_whitespace()
        .take(3)
        .filter_map(|v| v.parse().ok())
        .collect::<Vec<f32>>();
    let load = (
        loads.first().copied().unwrap_or(0.0),
        loads.get(1).copied().unwrap_or(0.0),
        loads.get(2).copied().unwrap_or(0.0),
    );
    let stat = text("/proc/stat").unwrap_or_default();
    // Guest time is already included in user/nice; summing it again inflates CPU.
    let counters: Vec<u64> = stat
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .skip(1)
        .take(8)
        .filter_map(|v| v.parse().ok())
        .collect();
    Cpu {
        model,
        cores,
        load,
        frequency_mhz: info
            .lines()
            .filter_map(|line| line.strip_prefix("cpu MHz\t\t: "))
            .filter_map(|v| v.parse::<f32>().ok())
            .fold(None, |mean, value| {
                let (sum, count) = mean.unwrap_or((0.0, 0));
                Some((sum + value, count + 1))
            })
            .map(|(sum, count)| sum / count as f32),
        governor: text("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .unwrap_or_else(|| "—".into()),
        driver: text("/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver")
            .unwrap_or_else(|| "—".into()),
        core_ticks: stat
            .lines()
            .skip(1)
            .take_while(|line| line.starts_with("cpu"))
            .map(|line| {
                let values: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .take(8)
                    .filter_map(|v| v.parse().ok())
                    .collect();
                (
                    values.iter().sum(),
                    values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0),
                )
            })
            .collect(),
        total_ticks: counters.iter().sum(),
        idle_ticks: counters.get(3).copied().unwrap_or(0) + counters.get(4).copied().unwrap_or(0),
        ..Default::default()
    }
}
fn memory() -> Memory {
    let values: HashMap<_, _> = fs::read_to_string("/proc/meminfo")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut p = line.split_whitespace();
            Some((
                p.next()?.trim_end_matches(':').to_string(),
                p.next()?.parse::<u64>().ok()? * 1024,
            ))
        })
        .collect();
    Memory {
        total: *values.get("MemTotal").unwrap_or(&0),
        available: *values
            .get("MemAvailable")
            .or(values.get("MemFree"))
            .unwrap_or(&0),
        swap_total: *values.get("SwapTotal").unwrap_or(&0),
        swap_free: *values.get("SwapFree").unwrap_or(&0),
    }
}
fn processes() -> Vec<Process> {
    let page = page_size();
    let mut out = Vec::new();
    for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let base = entry.path();
        let stat = fs::read_to_string(base.join("stat")).unwrap_or_default();
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let Some(rest) = stat.get(close + 2..) else {
            continue;
        };
        let fields: Vec<_> = rest.split_whitespace().collect();
        if fields.len() < 22 {
            continue;
        }
        let name = stat
            .find('(')
            .and_then(|open| stat.get(open + 1..close))
            .unwrap_or("process")
            .to_string();
        let state = fields[0].to_string();
        let cpu_ticks = fields
            .get(11)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            + fields
                .get(12)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
        let rss = fields
            .get(21)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * page;
        let arguments: Vec<String> = fs::read(base.join("cmdline"))
            .ok()
            .map(|bytes| {
                bytes
                    .split(|b| *b == 0)
                    .filter(|part| !part.is_empty())
                    .map(|part| String::from_utf8_lossy(part).into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let command = (!arguments.is_empty())
            .then(|| arguments.join(" "))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("[{name}]"));
        let user = fs::read_to_string(base.join("status"))
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("Uid:\t"))
                    .and_then(|v| v.split_whitespace().next())
                    .map(|v| v.to_string())
            })
            .unwrap_or_else(|| "—".into());
        let io_file = fs::read_to_string(base.join("io"));
        let io_available = io_file.is_ok();
        let io = io_file.unwrap_or_default();
        let counter = |key: &str| {
            io.lines()
                .find_map(|line| line.strip_prefix(key))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0)
        };
        out.push(Process {
            pid,
            name,
            user,
            state,
            cpu: 0.0,
            memory: rss,
            command,
            parent_pid: fields.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
            executable: fs::read_link(base.join("exe"))
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            arguments,
            cgroup: text(base.join("cgroup")).unwrap_or_default(),
            threads: fields.get(17).and_then(|v| v.parse().ok()).unwrap_or(1),
            read_rate: 0.0,
            write_rate: 0.0,
            io_available,
            io_read: counter("read_bytes:"),
            io_write: counter("write_bytes:"),
            cpu_ticks,
            start_ticks: fields.get(19).and_then(|v| v.parse().ok()).unwrap_or(0),
        });
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.memory));
    out
}
fn page_size() -> u64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE).max(1) as u64 }
}
fn disks() -> Vec<Disk> {
    let mut out = Vec::new();
    for line in fs::read_to_string("/proc/mounts")
        .unwrap_or_default()
        .lines()
    {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() < 3 || !p[0].starts_with('/') {
            continue;
        }
        let mount = p[1].replace("\\040", " ");
        if ["proc", "sysfs", "tmpfs", "devtmpfs", "cgroup"]
            .iter()
            .any(|f| p[2].starts_with(f))
        {
            continue;
        }
        if let Some((total, available)) = space(&mount) {
            out.push(Disk {
                mount,
                device: p[0].into(),
                total,
                available,
                filesystem: p[2].into(),
            });
        }
    }
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out
}
fn space(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0 {
        Some((s.f_blocks * s.f_frsize, s.f_bavail * s.f_frsize))
    } else {
        None
    }
}
fn network() -> Vec<Network> {
    let addresses = crate::telemetry::interface_addresses();
    fs::read_to_string("/proc/net/dev")
        .unwrap_or_default()
        .lines()
        .skip(2)
        .filter_map(|line| {
            let (name, values) = line.split_once(':')?;
            let v: Vec<_> = values.split_whitespace().collect();
            Some(Network {
                name: name.trim().into(),
                received: v.first()?.parse().ok()?,
                transmitted: v.get(8)?.parse().ok()?,
                state: text(format!("/sys/class/net/{}/operstate", name.trim()))
                    .unwrap_or_else(|| "unknown".into()),
                addresses: addresses.get(name.trim()).cloned().unwrap_or_default(),
                mac: text(format!("/sys/class/net/{}/address", name.trim())).unwrap_or_default(),
                speed_mbps: text(format!("/sys/class/net/{}/speed", name.trim()))
                    .and_then(|v| v.parse::<u64>().ok()),
                ..Default::default()
            })
        })
        .collect()
}
fn sockets() -> Vec<Socket> {
    let mut out = parse_sockets("/proc/net/tcp", "TCP", false);
    out.extend(parse_sockets("/proc/net/tcp6", "TCP6", true));
    out.extend(parse_sockets("/proc/net/udp", "UDP", false));
    out.extend(parse_sockets("/proc/net/udp6", "UDP6", true));
    out
}
fn parse_sockets(path: &str, protocol: &str, v6: bool) -> Vec<Socket> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let p: Vec<_> = line.split_whitespace().collect();
            Some(Socket {
                protocol: protocol.into(),
                local: address(p.get(1)?, v6),
                remote: address(p.get(2)?, v6),
                state: tcp_state(p.get(3).copied().unwrap_or("")),
                inode: p.get(9).unwrap_or(&"—").to_string(),
            })
        })
        .collect()
}
fn address(s: &str, v6: bool) -> String {
    let (ip, port) = s.split_once(':').unwrap_or((s, "0"));
    let port = u16::from_str_radix(port, 16).unwrap_or(0);
    if !v6 {
        let bytes = u32::from_str_radix(ip, 16).unwrap_or(0).to_ne_bytes();
        format!("{}:{port}", std::net::Ipv4Addr::from(bytes))
    } else {
        let mut bytes = [0_u8; 16];
        if ip.len() == 32 {
            for (index, chunk) in bytes.chunks_mut(4).enumerate() {
                chunk.copy_from_slice(
                    &u32::from_str_radix(&ip[index * 8..index * 8 + 8], 16)
                        .unwrap_or(0)
                        .to_ne_bytes(),
                );
            }
        }
        format!("[{}]:{port}", std::net::Ipv6Addr::from(bytes))
    }
}
fn tcp_state(s: &str) -> String {
    match s {
        "01" => "ESTABLISHED",
        "0A" => "LISTEN",
        "07" => "CLOSE",
        "06" => "TIME_WAIT",
        _ => s,
    }
    .into()
}
fn temperatures() -> Vec<Temperature> {
    let mut readings = vec![];
    for device in fs::read_dir("/sys/class/hwmon")
        .into_iter()
        .flatten()
        .flatten()
    {
        let base = device.path();
        let chip = text(base.join("name")).unwrap_or_else(|| "Sensor".into());
        for entry in fs::read_dir(&base).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(index) = name
                .strip_prefix("temp")
                .and_then(|name| name.strip_suffix("_input"))
            else {
                continue;
            };
            let Some(value) = text(entry.path())
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
            else {
                continue;
            };
            let label = text(base.join(format!("temp{index}_label")))
                .map(|label| format!("{chip} · {label}"))
                .unwrap_or_else(|| format!("{chip} · Sensor {index}"));
            readings.push(Temperature {
                label,
                celsius: value / 1000.0,
            });
        }
    }
    if readings.is_empty() {
        readings = fs::read_dir("/sys/class/thermal")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let base = entry.path();
                let value = text(base.join("temp"))?.parse::<f32>().ok()?;
                value.is_finite().then(|| Temperature {
                    label: text(base.join("type")).unwrap_or_else(|| "Thermal sensor".into()),
                    celsius: value / 1000.0,
                })
            })
            .collect();
    }
    readings.sort_by(|a, b| a.label.cmp(&b.label));
    readings
}
fn battery() -> Option<Battery> {
    let p = fs::read_dir("/sys/class/power_supply")
        .ok()?
        .flatten()
        .find(|e| text(e.path().join("type")).as_deref() == Some("Battery"))?
        .path();
    Some(Battery {
        capacity: text(p.join("capacity"))
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "—".into()),
        status: text(p.join("status")).unwrap_or_else(|| "Unknown".into()),
        energy: text(p.join("energy_now"))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|value| format!("{:.1} Wh", value / 1_000_000.0))
            .or_else(|| {
                text(p.join("charge_now"))
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|value| format!("{:.1} Ah", value / 1_000_000.0))
            })
            .unwrap_or_else(|| "—".into()),
    })
}
fn services(user: bool) -> Vec<Service> {
    let mut command = Command::new("systemctl");
    if user {
        command.arg("--user");
    }
    let Ok(output) = crate::telemetry::output_timeout(
        command.args([
            "list-units",
            "--type=service",
            "--all",
            "--no-legend",
            "--no-pager",
            "--plain",
            "--no-ask-password",
        ]),
        Duration::from_secs(3),
    ) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| {
            let p: Vec<_> = l.split_whitespace().collect();
            if p.len() < 4 {
                return None;
            };
            Some(Service {
                unit: p[0].into(),
                state: format!("{} / {}", p[2], p[3]),
                description: p[4..].join(" "),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_name_does_not_include_pid_or_parentheses() {
        let current = processes()
            .into_iter()
            .find(|p| p.pid == std::process::id())
            .expect("the running monitor is visible");
        assert_eq!(current.name, text("/proc/self/comm").unwrap());
    }
}
