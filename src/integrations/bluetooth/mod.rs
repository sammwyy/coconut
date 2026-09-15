#[cfg(not(target_os = "windows"))]
mod bluez;
#[cfg(target_os = "windows")]
mod windows;

pub trait BluetoothIntegration {
    fn powered(&self) -> bool;
    fn connected(&self) -> bool;
    fn device_name(&self) -> Option<String>;
    fn set_powered(&self, powered: bool);
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use bluez::BlueZ;
#[cfg(target_os = "windows")]
pub use windows::WindowsBluetooth;

pub struct Fallback;

impl BluetoothIntegration for Fallback {
    fn powered(&self) -> bool {
        false
    }
    fn connected(&self) -> bool {
        false
    }
    fn device_name(&self) -> Option<String> {
        None
    }
    fn set_powered(&self, _: bool) {}
}
