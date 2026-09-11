mod network_manager;

pub trait NetworkIntegration {
    fn connected(&self) -> bool;
    fn network_name(&self) -> Option<String>;
    fn strength(&self) -> Option<u8>;
}

pub use network_manager::NetworkManager;
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
