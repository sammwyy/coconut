#[cfg(not(target_os = "windows"))]
mod network_manager;
#[cfg(target_os = "windows")]
mod windows;

pub trait NetworkIntegration {
    fn connected(&self) -> bool;
    fn network_name(&self) -> Option<String>;
    fn strength(&self) -> Option<u8>;
    fn enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use network_manager::NetworkManager;
#[cfg(target_os = "windows")]
pub use windows::WindowsNetwork;
pub struct Fallback;
impl NetworkIntegration for Fallback {
    fn connected(&self) -> bool {
        false
    }
    fn network_name(&self) -> Option<String> {
        None
    }
    fn strength(&self) -> Option<u8> {
        None
    }
    fn enabled(&self) -> bool {
        false
    }
    fn set_enabled(&self, _: bool) {}
}
