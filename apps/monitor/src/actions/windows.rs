use super::{Process, ProcessAction, ServiceAction};
use ::windows::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE},
    System::Threading::{
        GetProcessTimes, OpenProcess, TerminateProcess, PROCESS_ACCESS_RIGHTS,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_TERMINATE,
    },
};
use std::time::Duration;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSuspendProcess(process_handle: HANDLE) -> i32;
    fn NtResumeProcess(process_handle: HANDLE) -> i32;
}

pub fn can_signal(process: &Process) -> bool {
    let current_user = crate::system::current_user();
    process.pid > 4
        && process.pid != std::process::id()
        && !current_user.is_empty()
        && process.user == current_user
}

pub fn signal(process: &Process, action: ProcessAction) -> Result<(), String> {
    if !can_signal(process) {
        return Err("This process cannot be controlled by the current user".into());
    }
    let handle = open_process(process, action)?;
    let result = unsafe {
        match action {
            ProcessAction::End | ProcessAction::Kill => {
                TerminateProcess(handle, 1).map_err(|error| error.to_string())
            }
            ProcessAction::Pause => nt_result(NtSuspendProcess(handle)),
            ProcessAction::Resume => nt_result(NtResumeProcess(handle)),
        }
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result?;
    if matches!(action, ProcessAction::Pause | ProcessAction::Resume) {
        crate::system::mark_process_suspended(process, action == ProcessAction::Pause);
    }
    Ok(())
}

fn open_process(process: &Process, action: ProcessAction) -> Result<HANDLE, String> {
    let access = match action {
        ProcessAction::End | ProcessAction::Kill => PROCESS_TERMINATE,
        ProcessAction::Pause | ProcessAction::Resume => PROCESS_SUSPEND_RESUME,
    };
    let handle = unsafe {
        OpenProcess(
            PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0 | access.0),
            false,
            process.pid,
        )
    }
    .map_err(|error| error.to_string())?;
    let verified = verify_identity(handle, process.start_ticks);
    if let Err(error) = verified {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(error);
    }
    Ok(handle)
}

fn verify_identity(handle: HANDLE, expected_start: u64) -> Result<(), String> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) }
        .map_err(|error| error.to_string())?;
    let start = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    if start == expected_start {
        Ok(())
    } else {
        Err("The process changed since it was selected. Refresh and select it again.".into())
    }
}

fn nt_result(status: i32) -> Result<(), String> {
    if status >= 0 {
        Ok(())
    } else {
        Err(format!(
            "Windows process operation failed with NTSTATUS {status:#010x}"
        ))
    }
}

pub fn run_service(unit: &str, user: bool, action: ServiceAction) -> Result<(), String> {
    validate_unit(unit)?;
    if user {
        return Err("Windows services do not have a user service scope".into());
    }
    let command = match action {
        ServiceAction::Start => "Start-Service",
        ServiceAction::Stop => "Stop-Service",
        ServiceAction::Restart => "Restart-Service",
    };
    crate::telemetry::powershell(
        &format!("{command} -Name '{unit}' -ErrorAction Stop"),
        Duration::from_secs(60),
    )
    .map(|_| ())
}

pub fn validate_unit(unit: &str) -> Result<(), String> {
    if !unit.starts_with('-')
        && !unit.is_empty()
        && unit.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        Ok(())
    } else {
        Err("Select a valid Windows service first".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn stale_identity_cannot_open_a_recycled_process() {
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep", "-Seconds", "30"])
            .spawn()
            .unwrap();
        let mut process = Process::default();
        process.pid = child.id();
        process.start_ticks = u64::MAX;
        let result = open_process(&process, ProcessAction::End);
        let still_running = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert!(result.is_err());
        assert!(still_running);
    }

    #[test]
    fn service_names_cannot_inject_powershell() {
        assert!(validate_unit("Print.Spooler").is_ok());
        assert!(validate_unit("Spooler'; Stop-Computer; '").is_err());
        assert!(validate_unit("").is_err());
    }
}
