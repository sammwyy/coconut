#[cfg(not(target_os = "windows"))]
mod network_manager;
#[cfg(target_os = "windows")]
mod windows;

pub trait NetworkIntegration {
    fn connected(&self) -> bool;
    fn network_name(&self) -> Option<String>;
    fn strength(&self) -> Option<u8>;
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
}
