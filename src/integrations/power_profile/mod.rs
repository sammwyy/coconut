#[cfg(not(target_os = "windows"))]
mod power_profiles_daemon;

/// One power profile the system supports (e.g. `power-saver`, `balanced`,
/// `performance`), as exposed by power-profiles-daemon.
#[derive(Clone)]
pub struct PowerProfile {
    pub id: String,
    pub active: bool,
}

pub trait PowerProfileIntegration {
    /// Every profile the system supports, in the backend's own order.
    /// Empty when there's no power-plan backend to talk to.
    fn profiles(&self) -> Vec<PowerProfile> {
        Vec::new()
    }
    fn set_profile(&self, _id: &str) {}
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal) observes a change. `None` when the backend has no such
    /// hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use power_profiles_daemon::PowerProfilesDaemon;

pub struct Fallback;
impl PowerProfileIntegration for Fallback {}
