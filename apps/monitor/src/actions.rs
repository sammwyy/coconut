//! Actions run only after an explicit UI interaction, against captured identities.
use crate::system::Process;
use std::{
    fs,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::Command,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessAction {
    End,
    Kill,
    Pause,
    Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}
impl ServiceAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}
#[derive(Clone)]
pub enum Action {
    Process(Process, ProcessAction),
    Application(String, Vec<Process>),
    Service {
        unit: String,
        user: bool,
        action: ServiceAction,
    },
}
impl Action {
    pub fn title(&self) -> String {
        match self {
            Self::Process(p, action) => format!(
                "{} {}?",
                match action {
                    ProcessAction::End => "End",
                    ProcessAction::Kill => "Force kill",
                    ProcessAction::Pause => "Pause",
                    ProcessAction::Resume => "Resume",
                },
                p.name
            ),
            Self::Application(name, _) => format!("End {name}?"),
            Self::Service { unit, action, .. } => format!(
                "{} {unit}?",
                match action {
                    ServiceAction::Start => "Start",
                    ServiceAction::Stop => "Stop",
                    ServiceAction::Restart => "Restart",
                }
            ),
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
        Self::Process(_,ProcessAction::Kill)=>"The process will stop immediately. Unsaved changes may be lost.",
        Self::Process(..)|Self::Application(..)=>"The selected task will be asked to exit. Unsaved changes may be lost.",
        Self::Service{..}=>"This changes the selected service and may interrupt applications that depend on it.",
    }
    }
    pub fn needs_confirmation(&self) -> bool {
        matches!(
            self,
            Self::Process(_, ProcessAction::End | ProcessAction::Kill)
                | Self::Application(..)
                | Self::Service {
                    action: ServiceAction::Stop | ServiceAction::Restart,
                    ..
                }
        )
    }
    pub fn run(&self) -> Result<String, String> {
        match self {
            Self::Process(p, action) => {
                signal(p, *action)?;
                Ok(format!("{}: action completed", p.name))
            }
            Self::Application(name, processes) => {
                let mut errors = vec![];
                let mut ended = 0;
                // Children first, so a launcher cannot disappear before its helpers.
                for p in processes.iter().rev() {
                    match signal(p, ProcessAction::End) {
                        Ok(()) => ended += 1,
                        Err(error) => errors.push(error),
                    }
                }
                if errors.is_empty() {
                    Ok(format!("{name}: requested exit for {ended} processes"))
                } else {
                    Err(format!(
                        "{ended} processes signalled. {}",
                        errors.join("; ")
                    ))
                }
            }
            Self::Service { unit, user, action } => {
                validate_unit(unit)?;
                let mut command = Command::new("systemctl");
                if *user {
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
                .map_err(|e| e.to_string())?;
                if output.status.success() {
                    Ok(format!("{unit}: {} completed", action.verb()))
                } else {
                    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(if error.is_empty() {
                        format!("Could not {} {unit}", action.verb())
                    } else {
                        error
                    })
                }
            }
        }
    }
}
fn validate_unit(unit: &str) -> Result<(), String> {
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
pub fn can_signal(process: &Process) -> bool {
    process.pid > 1
        && process.pid != std::process::id()
        && process.user == unsafe { libc::geteuid() }.to_string()
}

fn signal(process: &Process, action: ProcessAction) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_targets_and_protected_processes_are_rejected() {
        assert!(validate_unit("sshd.service").is_ok());
        for unit in [
            "--all.service",
            "/etc/example.service",
            "name.service --all",
            "name.socket",
        ] {
            assert!(validate_unit(unit).is_err());
        }
        let mut p = Process::default();
        p.pid = 1;
        assert!(!can_signal(&p));
        p.pid = std::process::id();
        assert!(!can_signal(&p));
    }
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
