#![allow(dead_code)]

pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod desktop;
pub mod network;
pub mod volume;

use std::rc::Rc;

pub struct IntegrationRegistry {
    pub desktop: Rc<dyn desktop::DesktopIntegration>,
    pub audio: Rc<dyn audio::AudioIntegration>,
    pub brightness: Rc<dyn brightness::BrightnessIntegration>,
    pub battery: Rc<dyn battery::BatteryIntegration>,
    pub volume: Rc<dyn volume::VolumeIntegration>,
    pub network: Rc<dyn network::NetworkIntegration>,
    pub bluetooth: Rc<dyn bluetooth::BluetoothIntegration>,
}

pub type Registry = IntegrationRegistry;

impl IntegrationRegistry {
    pub fn detect() -> Self {
        Self {
            // Each probe is cheap and verifies the service/device, not merely
            // that a binary happens to be installed.
            desktop: desktop::KWinDbus::detect()
                .map(|item| Rc::new(item) as Rc<dyn desktop::DesktopIntegration>)
                .unwrap_or_else(|| Rc::new(desktop::Fallback)),
            audio: audio::Mpris::detect()
                .map(|item| Rc::new(item) as Rc<dyn audio::AudioIntegration>)
                .unwrap_or_else(|| Rc::new(audio::Fallback)),
            brightness: brightness::BrightnessCtl::detect()
                .map(|item| Rc::new(item) as Rc<dyn brightness::BrightnessIntegration>)
                .unwrap_or_else(|| Rc::new(brightness::Fallback)),
            battery: battery::UPower::detect()
                .map(|item| Rc::new(item) as Rc<dyn battery::BatteryIntegration>)
                .unwrap_or_else(|| Rc::new(battery::Fallback)),
            volume: volume::PipeWire::detect()
                .map(|item| Rc::new(item) as Rc<dyn volume::VolumeIntegration>)
                .unwrap_or_else(|| Rc::new(volume::Fallback)),
            network: network::NetworkManager::detect()
                .map(|item| Rc::new(item) as Rc<dyn network::NetworkIntegration>)
                .unwrap_or_else(|| Rc::new(network::Fallback)),
            bluetooth: bluetooth::BluetoothCtl::detect()
                .map(|item| Rc::new(item) as Rc<dyn bluetooth::BluetoothIntegration>)
                .unwrap_or_else(|| Rc::new(bluetooth::Fallback)),
        }
    }
}
