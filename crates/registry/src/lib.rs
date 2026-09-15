//! Assembles a [`coconut_api::Registry`] from whichever integration
//! crates are compiled in, selected through this crate's Cargo features
//! (one feature per crate under `integrations/`, same name). This is the
//! only crate that knows about concrete backends: `coconut-api` stays
//! backend-agnostic, and each `integrations/<id>` crate only knows about
//! the one trait it implements.

use coconut_api::{IntegrationRegistry, Registry};
use std::rc::Rc;

pub fn detect() -> Registry {
    IntegrationRegistry {
        desktop: detect_desktop(),
        audio: detect_audio(),
        brightness: detect_brightness(),
        battery: detect_battery(),
        volume: detect_volume(),
        network: detect_network(),
        bluetooth: detect_bluetooth(),
        power_profile: detect_power_profile(),
    }
}

fn detect_desktop() -> Rc<dyn coconut_api::desktop::DesktopIntegration> {
    #[cfg(feature = "kwin")]
    if let Some(item) = coconut_integration_kwin::KWinDbus::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::desktop::WindowsDesktop::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::desktop::Fallback)
}

fn detect_audio() -> Rc<dyn coconut_api::audio::AudioIntegration> {
    #[cfg(feature = "mpris")]
    if let Some(item) = coconut_integration_mpris::Mpris::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::audio::WindowsMedia::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::audio::Fallback)
}

fn detect_brightness() -> Rc<dyn coconut_api::brightness::BrightnessIntegration> {
    #[cfg(feature = "brightnessctl")]
    if let Some(item) = coconut_integration_brightnessctl::BrightnessCtl::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::brightness::WindowsBrightness::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::brightness::Fallback)
}

fn detect_battery() -> Rc<dyn coconut_api::battery::BatteryIntegration> {
    #[cfg(feature = "upower")]
    if let Some(item) = coconut_integration_upower::UPower::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::battery::WindowsBattery::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::battery::Fallback)
}

fn detect_volume() -> Rc<dyn coconut_api::volume::VolumeIntegration> {
    #[cfg(feature = "pipewire")]
    if let Some(item) = coconut_integration_pipewire::PipeWire::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::volume::WindowsVolume::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::volume::Fallback)
}

fn detect_network() -> Rc<dyn coconut_api::network::NetworkIntegration> {
    #[cfg(feature = "network-manager")]
    if let Some(item) = coconut_integration_network_manager::NetworkManager::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::network::WindowsNetwork::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::network::Fallback)
}

fn detect_bluetooth() -> Rc<dyn coconut_api::bluetooth::BluetoothIntegration> {
    #[cfg(feature = "bluez")]
    if let Some(item) = coconut_integration_bluez::BlueZ::detect() {
        return Rc::new(item);
    }
    #[cfg(feature = "windows")]
    if let Some(item) = coconut_integration_windows::bluetooth::WindowsBluetooth::detect() {
        return Rc::new(item);
    }
    Rc::new(coconut_api::bluetooth::Fallback)
}

fn detect_power_profile() -> Rc<dyn coconut_api::power_profile::PowerProfileIntegration> {
    #[cfg(feature = "power-profiles-daemon")]
    if let Some(item) = coconut_integration_power_profiles_daemon::PowerProfilesDaemon::detect()
    {
        return Rc::new(item);
    }
    Rc::new(coconut_api::power_profile::Fallback)
}
