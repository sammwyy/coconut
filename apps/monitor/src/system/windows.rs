use super::*;
use ::windows::Win32::{
    Foundation::{CloseHandle, FILETIME},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
        Threading::{
            GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    process::Command,
    sync::{Mutex, OnceLock},
};
use sysinfo::{Disks, Networks, System};

pub const SERVICE_SCOPES: &[(bool, &str)] = &[(false, "System")];
pub const SOCKET_ID_LABEL: &str = "Process ID";

static CURRENT_USER: OnceLock<String> = OnceLock::new();
static SUSPENDED: OnceLock<Mutex<HashSet<(u32, u64)>>> = OnceLock::new();

struct Collector {
    system: System,
    disks: Disks,
    networks: Networks,
}

impl Collector {
    fn new() -> Self {
        Self {
            system: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_all();
        self.disks.refresh(true);
        self.networks.refresh(true);
    }
}

pub fn collect() -> Snapshot {
    static COLLECTOR: OnceLock<Mutex<Collector>> = OnceLock::new();
    let mut collector = COLLECTOR
        .get_or_init(|| Mutex::new(Collector::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    collector.refresh();
    let system = &collector.system;
    let processes = processes(system);
    let (total_ticks, idle_ticks) = system_times().unwrap_or((0, 0));
    let cpus = system.cpus();
    Snapshot {
        hostname: System::host_name().unwrap_or_else(|| "This computer".into()),
        kernel: System::kernel_version().unwrap_or_else(|| "Windows".into()),
        uptime: System::uptime(),
        cpu: Cpu {
            model: cpus
                .first()
                .map(|cpu| cpu.brand().trim().to_owned())
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| "Processor".into()),
            cores: cpus.len().max(1),
            per_core: cpus.iter().map(|cpu| cpu.cpu_usage()).collect(),
            frequency_mhz: cpus.first().map(|cpu| cpu.frequency() as f32),
            total_ticks,
            idle_ticks,
            ..Default::default()
        },
        os_name: crate::telemetry::os_name(),
        architecture: std::env::consts::ARCH.into(),
        drives: crate::telemetry::drives(),
        gpus: crate::telemetry::gpus(),
        memory: Memory {
            total: system.total_memory(),
            available: system.available_memory(),
            swap_total: system.total_swap(),
            swap_free: system.free_swap(),
        },
        processes,
        disks: disks(&collector.disks),
        network: networks(&collector.networks),
        sockets: sockets(),
        battery: battery(),
        services: services(),
        ..Default::default()
    }
}

pub fn current_user() -> String {
    CURRENT_USER
        .get_or_init(|| {
            let pid = sysinfo::Pid::from_u32(std::process::id());
            let mut system = System::new();
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                true,
                sysinfo::ProcessRefreshKind::nothing().with_user(sysinfo::UpdateKind::Always),
            );
            system
                .process(pid)
                .and_then(|process| process.user_id())
                .map(|user| user.to_string())
                .unwrap_or_default()
        })
        .clone()
}

pub fn clock_ticks() -> u64 {
    1000
}

pub fn mark_process_suspended(process: &Process, suspended: bool) {
    let mut processes = SUSPENDED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let identity = (process.pid, process.start_ticks);
    if suspended {
        processes.insert(identity);
    } else {
        processes.remove(&identity);
    }
}

fn processes(system: &System) -> Vec<Process> {
    let thread_counts = thread_counts();
    let suspended = SUSPENDED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut suspended = suspended
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    suspended.retain(|(pid, _)| system.process(sysinfo::Pid::from_u32(*pid)).is_some());
    let current_pid = std::process::id();
    let mut processes = system
        .processes()
        .values()
        .map(|process| {
            let pid = process.pid().as_u32();
            let name = process.name().to_string_lossy().into_owned();
            let user = process
                .user_id()
                .map(|user| user.to_string())
                .unwrap_or_else(|| "—".into());
            if pid == current_pid && !user.is_empty() && user != "—" {
                let _ = CURRENT_USER.set(user.clone());
            }
            let arguments = process
                .cmd()
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            let command = (!arguments.is_empty())
                .then(|| arguments.join(" "))
                .filter(|command| !command.is_empty())
                .unwrap_or_else(|| name.clone());
            let start_ticks = creation_time(pid).unwrap_or_else(|| process.start_time());
            let disk = process.disk_usage();
            Process {
                pid,
                name,
                user,
                state: if suspended.contains(&(pid, start_ticks))
                    || process.status() == sysinfo::ProcessStatus::Stop
                {
                    "T".into()
                } else {
                    "R".into()
                },
                cpu: 0.0,
                memory: process.memory(),
                command,
                parent_pid: process.parent().map_or(0, |parent| parent.as_u32()),
                executable: process
                    .exe()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                arguments,
                cgroup: String::new(),
                threads: thread_counts.get(&pid).copied().unwrap_or(0),
                read_rate: 0.0,
                write_rate: 0.0,
                io_available: true,
                io_read: disk.total_read_bytes,
                io_write: disk.total_written_bytes,
                cpu_ticks: process.accumulated_cpu_time(),
                start_ticks,
            }
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| std::cmp::Reverse(process.memory));
    processes
}

fn thread_counts() -> HashMap<u32, u64> {
    let mut counts = HashMap::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
            return counts;
        };
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                *counts.entry(entry.th32OwnerProcessID).or_insert(0) += 1;
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    counts
}

fn system_times() -> Option<(u64, u64)> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some((
        filetime(kernel).saturating_add(filetime(user)),
        filetime(idle),
    ))
}

fn creation_time(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let result = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
            .map(|_| filetime(creation));
        let _ = CloseHandle(handle);
        result.ok()
    }
}

fn filetime(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}

fn disks(disks: &Disks) -> Vec<Disk> {
    let mut result = disks
        .list()
        .iter()
        .map(|disk| Disk {
            mount: disk.mount_point().to_string_lossy().into_owned(),
            device: disk.name().to_string_lossy().into_owned(),
            total: disk.total_space(),
            available: disk.available_space(),
            filesystem: disk.file_system().to_string_lossy().into_owned(),
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.mount.cmp(&b.mount));
    result
}

fn networks(networks: &Networks) -> Vec<Network> {
    let mut result = networks
        .iter()
        .map(|(name, network)| Network {
            name: name.clone(),
            received: network.total_received(),
            transmitted: network.total_transmitted(),
            state: "Up".into(),
            addresses: network
                .ip_networks()
                .iter()
                .map(|network| network.addr.to_string())
                .collect(),
            mac: network.mac_address().to_string(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn sockets() -> Vec<Socket> {
    let Ok(output) = crate::telemetry::output_timeout(
        Command::new("netstat.exe").arg("-ano"),
        Duration::from_secs(3),
    ) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_socket)
        .collect()
}

fn parse_socket(line: &str) -> Option<Socket> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let tcp = fields.first() == Some(&"TCP");
    let udp = fields.first() == Some(&"UDP");
    if (!tcp && !udp) || fields.len() < if tcp { 5 } else { 4 } {
        return None;
    }
    let local = fields[1];
    let ipv6 = local.starts_with('[');
    let protocol = match (tcp, ipv6) {
        (true, false) => "TCP",
        (true, true) => "TCP6",
        (false, false) => "UDP",
        (false, true) => "UDP6",
    };
    let state = if tcp {
        match fields[3] {
            "LISTENING" => "LISTEN",
            state => state,
        }
    } else {
        ""
    };
    Some(Socket {
        protocol: protocol.into(),
        local: local.into(),
        remote: if udp {
            if ipv6 { "[::]:0" } else { "0.0.0.0:0" }.into()
        } else {
            fields[2].into()
        },
        state: state.into(),
        inode: fields.last()?.to_string(),
    })
}

fn battery() -> Option<Battery> {
    let mut status = unsafe { std::mem::zeroed::<SYSTEM_POWER_STATUS>() };
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    if status.BatteryFlag & 128 != 0 || status.BatteryLifePercent == 255 {
        return None;
    }
    Some(Battery {
        capacity: format!("{}%", status.BatteryLifePercent),
        status: if status.BatteryFlag & 8 != 0 {
            "Charging"
        } else if status.ACLineStatus == 1 {
            "AC power"
        } else {
            "Discharging"
        }
        .into(),
        energy: "—".into(),
    })
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct ServiceInfo {
    name: String,
    status: serde_json::Value,
    display_name: String,
}

fn service_state(status: &serde_json::Value) -> &'static str {
    let status = match status {
        serde_json::Value::Number(number) => number.as_u64().map(|status| status.to_string()),
        serde_json::Value::String(status) => Some(status.clone()),
        _ => None,
    };
    match status.as_deref() {
        Some("4" | "Running") => "active / running",
        Some("1" | "Stopped") => "inactive / stopped",
        Some("2" | "StartPending") => "activating / start-pending",
        Some("3" | "StopPending") => "deactivating / stop-pending",
        Some("7" | "Paused") => "inactive / paused",
        _ => "unknown",
    }
}

fn services() -> Vec<Service> {
    let script = r#"
@(Get-Service | Select-Object Name,Status,DisplayName) | ConvertTo-Json -Compress
"#;
    let Ok(output) = crate::telemetry::powershell(script, Duration::from_secs(5)) else {
        return Vec::new();
    };
    let mut services = serde_json::from_str::<Vec<ServiceInfo>>(&output)
        .unwrap_or_default()
        .into_iter()
        .filter(|service| !service.name.is_empty())
        .map(|service| Service {
            unit: service.name,
            state: service_state(&service.status).into(),
            description: service.display_name,
        })
        .collect::<Vec<_>>();
    services.sort_by(|a, b| a.unit.cmp(&b.unit));
    services
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netstat_tcp_and_udp_rows_keep_their_process_identity() {
        let tcp = parse_socket("  TCP    127.0.0.1:443    127.0.0.1:5000    ESTABLISHED    1234")
            .unwrap();
        assert_eq!(tcp.protocol, "TCP");
        assert_eq!(tcp.state, "ESTABLISHED");
        assert_eq!(tcp.inode, "1234");
        let listener = parse_socket("  TCP    [::]:135    [::]:0    LISTENING    42").unwrap();
        assert_eq!(listener.protocol, "TCP6");
        assert_eq!(listener.state, "LISTEN");
        let udp = parse_socket("  UDP    0.0.0.0:5353    *:*    99").unwrap();
        assert_eq!(udp.remote, "0.0.0.0:0");
        assert_eq!(udp.inode, "99");
        assert!(parse_socket("not a socket").is_none());
    }

    #[test]
    fn service_json_preserves_names_and_display_descriptions() {
        let services = serde_json::from_str::<Vec<ServiceInfo>>(
            r#"[{"Name":"Spooler","Status":"Running","DisplayName":"Print Spooler"}]"#,
        )
        .unwrap();
        assert_eq!(services[0].name, "Spooler");
        assert_eq!(services[0].display_name, "Print Spooler");
        assert_eq!(service_state(&services[0].status), "active / running");
        let numeric = serde_json::from_str::<ServiceInfo>(
            r#"{"Name":"Spooler","Status":4,"DisplayName":"Print Spooler"}"#,
        )
        .unwrap();
        assert_eq!(service_state(&numeric.status), "active / running");
    }

    #[test]
    fn windows_services_are_loaded_with_numeric_status_values() {
        let services = services();
        assert!(!services.is_empty());
        assert!(services.iter().any(|service| service.state != "unknown"));
    }

    #[test]
    fn thread_snapshot_includes_the_current_process() {
        assert!(thread_counts()
            .get(&std::process::id())
            .is_some_and(|threads| *threads > 0));
    }
}
