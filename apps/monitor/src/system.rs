use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[derive(Clone, Default)]
pub struct Snapshot {
    pub hostname: String,
    pub kernel: String,
    pub uptime: u64,
    pub cpu: Cpu,
    pub os_name: String,
    pub architecture: String,
    pub drives: Vec<crate::telemetry::Drive>,
    pub gpus: Vec<crate::telemetry::Gpu>,
    pub memory: Memory,
    pub processes: Vec<Process>,
    pub applications: Vec<crate::applications::Application>,
    pub disks: Vec<Disk>,
    pub network: Vec<Network>,
    pub sockets: Vec<Socket>,
    pub temperatures: Vec<Temperature>,
    pub battery: Option<Battery>,
    pub services: Vec<Service>,
    pub user_services: Vec<Service>,
    sampled_at: Option<Instant>,
}
#[derive(Clone, Default)]
pub struct Cpu {
    pub model: String,
    pub cores: usize,
    pub usage: f32,
    #[cfg(target_os = "linux")]
    pub load: (f32, f32, f32),
    pub sampled: bool,
    pub per_core: Vec<f32>,
    pub frequency_mhz: Option<f32>,
    #[cfg(target_os = "linux")]
    pub governor: String,
    #[cfg(target_os = "linux")]
    pub driver: String,
    core_ticks: Vec<(u64, u64)>,
    total_ticks: u64,
    idle_ticks: u64,
}
#[derive(Clone, Default)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}
#[derive(Clone, Default)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub state: String,
    pub cpu: f32,
    pub memory: u64,
    pub command: String,
    pub parent_pid: u32,
    pub executable: String,
    pub arguments: Vec<String>,
    pub cgroup: String,
    pub threads: u64,
    pub read_rate: f32,
    pub write_rate: f32,
    pub io_available: bool,
    io_read: u64,
    io_write: u64,
    cpu_ticks: u64,
    pub(crate) start_ticks: u64,
}
#[derive(Clone, Default)]
pub struct Disk {
    pub mount: String,
    pub device: String,
    pub total: u64,
    pub available: u64,
    pub filesystem: String,
}
#[derive(Clone, Default)]
pub struct Network {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
    pub state: String,
    pub addresses: Vec<String>,
    pub mac: String,
    pub speed_mbps: Option<u64>,
    pub receive_rate: f32,
    pub transmit_rate: f32,
    pub sampled: bool,
}
#[derive(Clone, Default)]
pub struct Socket {
    pub protocol: String,
    pub local: String,
    pub remote: String,
    pub state: String,
    pub inode: String,
}
#[derive(Clone, Default)]
pub struct Temperature {
    pub label: String,
    pub celsius: f32,
}
#[derive(Clone, Default)]
pub struct Battery {
    pub capacity: String,
    pub status: String,
    pub energy: String,
}
#[derive(Clone, Default)]
pub struct Service {
    pub unit: String,
    pub state: String,
    pub description: String,
}

pub const SERVICE_SCOPES: &[(bool, &str)] = platform::SERVICE_SCOPES;
pub const SOCKET_ID_LABEL: &str = platform::SOCKET_ID_LABEL;

pub fn collect() -> Snapshot {
    let mut snapshot = platform::collect();
    snapshot.sampled_at = Some(Instant::now());
    snapshot.applications = crate::applications::collect(&snapshot.processes);
    snapshot
}

/// Resource percentages describe work between refreshes, rather than load
/// average or a process's CPU time divided by the machine's entire uptime.
pub fn collect_since(previous: &Snapshot) -> Snapshot {
    let mut next = collect();
    apply_samples(previous, &mut next);
    next.applications = crate::applications::collect(&next.processes);
    next
}

pub fn current_user() -> String {
    platform::current_user()
}

#[cfg(target_os = "windows")]
pub fn mark_process_suspended(process: &Process, suspended: bool) {
    platform::mark_process_suspended(process, suspended);
}

fn apply_samples(previous: &Snapshot, next: &mut Snapshot) {
    let total = next
        .cpu
        .total_ticks
        .saturating_sub(previous.cpu.total_ticks);
    let idle = next.cpu.idle_ticks.saturating_sub(previous.cpu.idle_ticks);
    if total > 0 && previous.cpu.total_ticks > 0 {
        next.cpu.usage =
            (total.saturating_sub(idle) as f32 / total as f32 * 100.0).clamp(0.0, 100.0);
        next.cpu.sampled = true;
    }
    if !next.cpu.core_ticks.is_empty() {
        next.cpu.per_core = next
            .cpu
            .core_ticks
            .iter()
            .zip(&previous.cpu.core_ticks)
            .map(|((total, idle), (old_total, old_idle))| {
                let elapsed = total.saturating_sub(*old_total);
                let idle = idle.saturating_sub(*old_idle);
                if elapsed == 0 {
                    0.0
                } else {
                    elapsed.saturating_sub(idle) as f32 / elapsed as f32 * 100.0
                }
            })
            .collect();
    }
    let elapsed = next
        .sampled_at
        .zip(previous.sampled_at)
        .map(|(now, then)| now.saturating_duration_since(then).as_secs_f64())
        .unwrap_or(0.0);
    if elapsed > 0.0 {
        let old: HashMap<_, _> = previous
            .processes
            .iter()
            .map(|p| ((p.pid, p.start_ticks), p))
            .collect();
        for process in &mut next.processes {
            if let Some(old) = old.get(&(process.pid, process.start_ticks)) {
                process.cpu = (process.cpu_ticks.saturating_sub(old.cpu_ticks) as f64
                    / platform::clock_ticks() as f64
                    / elapsed
                    * 100.0) as f32;
                process.read_rate =
                    process.io_read.saturating_sub(old.io_read) as f32 / elapsed as f32;
                process.write_rate =
                    process.io_write.saturating_sub(old.io_write) as f32 / elapsed as f32;
            }
        }
        for network in &mut next.network {
            if let Some(old) = previous.network.iter().find(|old| old.name == network.name) {
                network.receive_rate =
                    network.received.saturating_sub(old.received) as f32 / elapsed as f32;
                network.transmit_rate =
                    network.transmitted.saturating_sub(old.transmitted) as f32 / elapsed as f32;
                network.sampled = true;
            }
        }
        crate::telemetry::sample_drives(&previous.drives, &mut next.drives, elapsed as f32);
    }
}

pub fn cpu_temperature(snapshot: &Snapshot) -> Option<f32> {
    snapshot
        .temperatures
        .iter()
        .filter(|reading| {
            let label = reading.label.to_lowercase();
            [
                "cpu",
                "coretemp",
                "k10temp",
                "zenpower",
                "tctl",
                "tdie",
                "x86_pkg_temp",
            ]
            .iter()
            .any(|key| label.contains(key))
        })
        .map(|reading| reading.celsius)
        .filter(|value| value.is_finite())
        .reduce(f32::max)
}

pub fn bytes(bytes: u64) -> String {
    const U: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut n = bytes as f64;
    let mut i = 0;
    while n >= 1024.0 && i < U.len() - 1 {
        n /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n:.0} {}", U[i])
    } else {
        format!("{n:.1} {}", U[i])
    }
}
pub fn duration(seconds: u64) -> String {
    let d = Duration::from_secs(seconds);
    format!(
        "{}d {:02}h {:02}m",
        d.as_secs() / 86400,
        (d.as_secs() / 3600) % 24,
        (d.as_secs() / 60) % 60
    )
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn cpu_temperature_excludes_gpu_sensors_and_uses_the_hottest_core() {
        let mut snapshot = Snapshot::default();
        snapshot.temperatures = vec![Temperature {
            label: "amdgpu · edge".into(),
            celsius: 89.0,
        }];
        assert_eq!(cpu_temperature(&snapshot), None);
        snapshot.temperatures.extend([
            Temperature {
                label: "coretemp · Package id 0".into(),
                celsius: 46.0,
            },
            Temperature {
                label: "coretemp · Core 0".into(),
                celsius: 48.0,
            },
            Temperature {
                label: "coretemp · Core 1".into(),
                celsius: f32::NAN,
            },
        ]);
        assert_eq!(cpu_temperature(&snapshot), Some(48.0));
    }

    #[test]
    fn usage_measures_the_interval_and_ignores_recycled_pids() {
        let now = Instant::now();
        let mut previous = Snapshot::default();
        previous.sampled_at = Some(now - Duration::from_secs(2));
        previous.cpu.total_ticks = 1000;
        previous.cpu.idle_ticks = 600;
        previous.processes = vec![
            Process {
                pid: 42,
                cpu_ticks: 100,
                start_ticks: 10,
                ..Default::default()
            },
            Process {
                pid: 43,
                cpu_ticks: 100,
                start_ticks: 11,
                ..Default::default()
            },
        ];
        let mut next = Snapshot::default();
        next.sampled_at = Some(now);
        next.cpu.total_ticks = 1400;
        next.cpu.idle_ticks = 700;
        next.processes = vec![
            Process {
                pid: 42,
                cpu_ticks: 200,
                start_ticks: 10,
                ..Default::default()
            },
            Process {
                pid: 43,
                cpu_ticks: 200,
                start_ticks: 90,
                ..Default::default()
            },
        ];
        apply_samples(&previous, &mut next);
        assert_eq!(next.cpu.usage, 75.0);
        assert!(next.cpu.sampled);
        assert_eq!(
            next.processes[0].cpu,
            5000.0 / platform::clock_ticks() as f32
        );
        assert_eq!(next.processes[1].cpu, 0.0);
        // A refresh within one kernel tick must not invent a percentage.
        previous.cpu.total_ticks = next.cpu.total_ticks;
        next.cpu.sampled = false;
        apply_samples(&previous, &mut next);
        assert!(!next.cpu.sampled);
    }

    #[test]
    fn direct_cpu_samples_survive_platforms_without_core_tick_counters() {
        let mut previous = Snapshot::default();
        previous.cpu.total_ticks = 1000;
        previous.cpu.idle_ticks = 500;
        let mut next = Snapshot::default();
        next.cpu.total_ticks = 2000;
        next.cpu.idle_ticks = 1000;
        next.cpu.per_core = vec![12.0, 34.0];
        apply_samples(&previous, &mut next);
        assert_eq!(next.cpu.per_core, vec![12.0, 34.0]);
    }
}
