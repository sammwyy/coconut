use super::{Process, ProcessAction, ServiceAction};
use std::{
    fs,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::Command,
    time::Duration,
};

pub fn can_signal(process: &Process) -> bool {
    process.pid > 1
        && process.pid != std::process::id()
        && process.user == unsafe { libc::geteuid() }.to_string()
}

pub fn signal(process: &Process, action: ProcessAction) -> Result<(), String> {
    if !can_signal(process) {
        return Err("This process cannot be controlled by the current user".into());
    }
    // A pidfd continues to identify the same process even if Linux reuses its PID.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, process.pid, 0) };
    if raw < 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error.to_string())
        };
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
    let stat = fs::read_to_string(format!("/proc/{}/stat", process.pid))
        .map_err(|_| "The process has already exited".to_string())?;
    let start = stat
        .rfind(')')
        .and_then(|index| stat.get(index + 2..))
        .and_then(|rest| rest.split_whitespace().nth(19))
        .and_then(|v| v.parse::<u64>().ok());
    if start != Some(process.start_ticks) {
        return Err(
            "The process changed since it was selected. Refresh and select it again.".into(),
        );
    }
    let signal = match action {
        ProcessAction::End => libc::SIGTERM,
        ProcessAction::Kill => libc::SIGKILL,
        ProcessAction::Pause => libc::SIGSTOP,
        ProcessAction::Resume => libc::SIGCONT,
    };
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error.to_string())
        }
    }
}

pub fn run_service(unit: &str, user: bool, action: ServiceAction) -> Result<(), String> {
    validate_unit(unit)?;
    let mut command = Command::new("systemctl");
    if user {
        command.arg("--user");
    }
    let output = crate::telemetry::output_timeout(
        command
            .arg("--no-pager")
            .arg(action.verb())
            .arg("--")
            .arg(unit),
        Duration::from_secs(60),
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if error.is_empty() {
            format!("Could not {} {unit}", action.verb())
        } else {
            error
        })
    }
}

pub fn validate_unit(unit: &str) -> Result<(), String> {
    if unit.ends_with(".service")
        && !unit.starts_with('-')
        && !unit.contains('/')
        && !unit.chars().any(char::is_whitespace)
    {
        Ok(())
    } else {
        Err("Select a valid service unit first".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_identity_cannot_signal_a_recycled_pid() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let mut p = Process::default();
        p.pid = child.id();
        p.user = unsafe { libc::geteuid() }.to_string();
        p.start_ticks = u64::MAX;
        let result = signal(&p, ProcessAction::End);
        let still_running = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert!(result.is_err());
        assert!(still_running);
    }
}
