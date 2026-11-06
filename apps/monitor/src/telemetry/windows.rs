use super::{Drive, Gpu};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::{process::Command, sync::OnceLock, time::Duration};

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct DiskResponse {
    performance: Vec<DiskSample>,
    models: Vec<DiskModel>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct DiskSample {
    name: String,
    percent_disk_time: Option<f64>,
    disk_read_bytes_persec: Option<f64>,
    disk_write_bytes_persec: Option<f64>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct DiskModel {
    index: u32,
    model: Option<String>,
    size: Option<u64>,
    media_type: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct GpuInfo {
    name: String,
    driver_version: String,
    #[serde(rename = "AdapterRAM")]
    adapter_ram: Option<u64>,
}

pub fn os_name() -> String {
    sysinfo::System::long_os_version()
        .or_else(sysinfo::System::name)
        .unwrap_or_else(|| "Windows".into())
}

pub fn drives() -> Vec<Drive> {
    let script = r#"
[pscustomobject]@{
  Performance = @(Get-CimInstance Win32_PerfFormattedData_PerfDisk_PhysicalDisk |
    Where-Object Name -ne '_Total' |
    Select-Object Name,PercentDiskTime,DiskReadBytesPersec,DiskWriteBytesPersec)
  Models = @(Get-CimInstance Win32_DiskDrive |
    Select-Object Index,Model,Size,MediaType)
} | ConvertTo-Json -Depth 4 -Compress
"#;
    let response: DiskResponse = powershell_json(script).unwrap_or_default();
    let mut drives = response
        .performance
        .into_iter()
        .filter(|sample| !sample.name.is_empty())
        .map(|sample| {
            let index = sample
                .name
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u32>().ok());
            let model =
                index.and_then(|index| response.models.iter().find(|model| model.index == index));
            Drive {
                name: sample.name,
                model: model
                    .and_then(|model| model.model.clone())
                    .unwrap_or_else(|| "Disk drive".into()),
                kind: model
                    .and_then(|model| model.media_type.clone())
                    .filter(|kind| !kind.is_empty())
                    .unwrap_or_else(|| "Fixed disk".into()),
                size: model.and_then(|model| model.size).unwrap_or(0),
                read_rate: sample.disk_read_bytes_persec.unwrap_or(0.0).max(0.0) as f32,
                write_rate: sample.disk_write_bytes_persec.unwrap_or(0.0).max(0.0) as f32,
                usage: sample.percent_disk_time.unwrap_or(0.0).clamp(0.0, 100.0) as f32,
                sampled: true,
                direct_rates: true,
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    drives.sort_by(|a, b| a.name.cmp(&b.name));
    drives
}

pub fn gpus() -> Vec<Gpu> {
    static GPUS: OnceLock<Vec<Gpu>> = OnceLock::new();
    GPUS.get_or_init(|| {
        let script = r#"
@(Get-CimInstance Win32_VideoController |
  Select-Object Name,DriverVersion,AdapterRAM) | ConvertTo-Json -Compress
"#;
        powershell_json::<Vec<GpuInfo>>(script)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, gpu)| Gpu {
                id: format!("gpu{index}"),
                name: gpu.name,
                driver: gpu.driver_version,
                memory_total: gpu.adapter_ram.filter(|bytes| *bytes > 0),
                ..Default::default()
            })
            .collect()
    })
    .clone()
}

pub(crate) fn powershell(script: &str, timeout: Duration) -> Result<String, String> {
    let output = super::output_timeout(
        Command::new("powershell.exe").args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]),
        timeout,
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if error.is_empty() {
            "Windows PowerShell could not complete the operation".into()
        } else {
            error
        })
    }
}

fn powershell_json<T: DeserializeOwned>(script: &str) -> Option<T> {
    let output = powershell(script, Duration::from_secs(5)).ok()?;
    serde_json::from_str(&output).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_disk_json_accepts_windows_counter_names() {
        let response: DiskResponse = serde_json::from_str(
            r#"{"Performance":[{"Name":"0 C:","PercentDiskTime":12.5,"DiskReadBytesPersec":1024,"DiskWriteBytesPersec":2048}],"Models":[{"Index":0,"Model":"NVMe drive","Size":1024,"MediaType":"Fixed hard disk media"}]}"#,
        )
        .unwrap();
        assert_eq!(response.performance[0].name, "0 C:");
        assert_eq!(response.performance[0].disk_read_bytes_persec, Some(1024.0));
        assert_eq!(response.models[0].index, 0);
    }
}
