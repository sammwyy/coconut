mod bluetoothctl;

pub trait BluetoothIntegration {
    fn powered(&self) -> bool;
    fn connected(&self) -> bool;
    fn device_name(&self) -> Option<String>;
}

pub use bluetoothctl::BluetoothCtl;

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
}
