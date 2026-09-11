mod upower;

pub trait BatteryIntegration {
    fn percentage(&self) -> Option<u8>;
    fn charging(&self) -> bool;
}

pub use upower::UPower;
pub struct Fallback;
impl BatteryIntegration for Fallback {
    fn percentage(&self) -> Option<u8> {
        None
    }
    fn charging(&self) -> bool {
        false
    }
}
