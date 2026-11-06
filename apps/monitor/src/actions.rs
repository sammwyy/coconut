//! Actions run only after an explicit UI interaction, against captured identities.
use crate::system::Process;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
use windows as platform;

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
            Self::Process(_, ProcessAction::Kill) => {
                "The process will stop immediately. Unsaved changes may be lost."
            }
            Self::Process(..) | Self::Application(..) => {
                #[cfg(target_os = "windows")]
                {
                    "The selected task will be stopped. Unsaved changes may be lost."
                }
                #[cfg(target_os = "linux")]
                {
                    "The selected task will be asked to exit. Unsaved changes may be lost."
                }
            }
            Self::Service { .. } => {
                "This changes the selected service and may interrupt applications that depend on it."
            }
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
                platform::signal(p, *action)?;
                Ok(format!("{}: action completed", p.name))
            }
            Self::Application(name, processes) => {
                let mut errors = vec![];
                let mut ended = 0;
                // Children first, so a launcher cannot disappear before its helpers.
                for p in processes.iter().rev() {
                    match platform::signal(p, ProcessAction::End) {
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
                platform::run_service(unit, *user, *action)?;
                Ok(format!("{unit}: {} completed", action.verb()))
            }
        }
    }
}

pub fn can_signal(process: &Process) -> bool {
    platform::can_signal(process)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_targets_and_protected_processes_are_rejected() {
        assert!(platform::validate_unit("sshd.service").is_ok());
        for unit in [
            "--all.service",
            "/etc/example.service",
            "name.service --all",
        ] {
            assert!(platform::validate_unit(unit).is_err());
        }
        #[cfg(target_os = "linux")]
        assert!(platform::validate_unit("name.socket").is_err());
        let mut p = Process::default();
        p.pid = 1;
        assert!(!can_signal(&p));
        p.pid = std::process::id();
        assert!(!can_signal(&p));
    }
}
