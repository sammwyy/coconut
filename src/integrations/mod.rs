#![allow(dead_code)]

pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod desktop;
pub mod network;
pub mod volume;

#[cfg(target_os = "windows")]
mod windows;

use std::rc::Rc;
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};

/// A wake-up channel a background event thread pushes into whenever an
/// integration's native hook (a D-Bus signal, a platform event) observes a
/// change, so callers can block waiting for the next one instead of polling.
#[derive(Clone)]
pub struct ChangeListener {
    receiver: Arc<Mutex<Receiver<()>>>,
}

impl ChangeListener {
    pub fn new(receiver: Receiver<()>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn wait(&self) -> bool {
        self.receiver
            .lock()
            .is_ok_and(|receiver| receiver.recv().is_ok())
    }
}

/// Runs `run` on a background thread; it should block on native event I/O
/// (a D-Bus connection, a platform API) and push into the sender it is
/// given whenever it observes a change, stopping once the receiver it is
/// given yields. The returned guard stops and joins that thread when
/// dropped, and the listener wakes on each push.
pub fn spawn_event_bridge(
    run: impl FnOnce(mpsc::SyncSender<()>, Receiver<()>) + Send + 'static,
) -> (ChangeListener, EventBridgeGuard) {
    let (change_tx, change_rx) = mpsc::sync_channel(1);
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let watcher = thread::spawn(move || run(change_tx, shutdown_rx));
    (
        ChangeListener::new(change_rx),
        EventBridgeGuard {
            shutdown: Some(shutdown_tx),
            watcher: Some(watcher),
        },
    )
}

pub struct EventBridgeGuard {
    shutdown: Option<mpsc::Sender<()>>,
    watcher: Option<JoinHandle<()>>,
}

impl Drop for EventBridgeGuard {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}

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
            #[cfg(not(target_os = "windows"))]
            desktop: desktop::KWinDbus::detect()
                .map(|item| Rc::new(item) as Rc<dyn desktop::DesktopIntegration>)
                .unwrap_or_else(|| Rc::new(desktop::Fallback)),
            #[cfg(target_os = "windows")]
            desktop: desktop::WindowsDesktop::detect()
                .map(|item| Rc::new(item) as Rc<dyn desktop::DesktopIntegration>)
                .unwrap_or_else(|| Rc::new(desktop::Fallback)),
            #[cfg(not(target_os = "windows"))]
            audio: audio::Mpris::detect()
                .map(|item| Rc::new(item) as Rc<dyn audio::AudioIntegration>)
                .unwrap_or_else(|| Rc::new(audio::Fallback)),
            #[cfg(target_os = "windows")]
            audio: audio::WindowsMedia::detect()
                .map(|item| Rc::new(item) as Rc<dyn audio::AudioIntegration>)
                .unwrap_or_else(|| Rc::new(audio::Fallback)),
            #[cfg(not(target_os = "windows"))]
            brightness: brightness::BrightnessCtl::detect()
                .map(|item| Rc::new(item) as Rc<dyn brightness::BrightnessIntegration>)
                .unwrap_or_else(|| Rc::new(brightness::Fallback)),
            #[cfg(target_os = "windows")]
            brightness: brightness::WindowsBrightness::detect()
                .map(|item| Rc::new(item) as Rc<dyn brightness::BrightnessIntegration>)
                .unwrap_or_else(|| Rc::new(brightness::Fallback)),
            #[cfg(not(target_os = "windows"))]
            battery: battery::UPower::detect()
                .map(|item| Rc::new(item) as Rc<dyn battery::BatteryIntegration>)
                .unwrap_or_else(|| Rc::new(battery::Fallback)),
            #[cfg(target_os = "windows")]
            battery: battery::WindowsBattery::detect()
                .map(|item| Rc::new(item) as Rc<dyn battery::BatteryIntegration>)
                .unwrap_or_else(|| Rc::new(battery::Fallback)),
            #[cfg(not(target_os = "windows"))]
            volume: volume::PipeWire::detect()
                .map(|item| Rc::new(item) as Rc<dyn volume::VolumeIntegration>)
                .unwrap_or_else(|| Rc::new(volume::Fallback)),
            #[cfg(target_os = "windows")]
            volume: volume::WindowsVolume::detect()
                .map(|item| Rc::new(item) as Rc<dyn volume::VolumeIntegration>)
                .unwrap_or_else(|| Rc::new(volume::Fallback)),
            #[cfg(not(target_os = "windows"))]
            network: network::NetworkManager::detect()
                .map(|item| Rc::new(item) as Rc<dyn network::NetworkIntegration>)
                .unwrap_or_else(|| Rc::new(network::Fallback)),
            #[cfg(target_os = "windows")]
            network: network::WindowsNetwork::detect()
                .map(|item| Rc::new(item) as Rc<dyn network::NetworkIntegration>)
                .unwrap_or_else(|| Rc::new(network::Fallback)),
            #[cfg(not(target_os = "windows"))]
            bluetooth: bluetooth::BlueZ::detect()
                .map(|item| Rc::new(item) as Rc<dyn bluetooth::BluetoothIntegration>)
                .unwrap_or_else(|| Rc::new(bluetooth::Fallback)),
            #[cfg(target_os = "windows")]
            bluetooth: bluetooth::WindowsBluetooth::detect()
                .map(|item| Rc::new(item) as Rc<dyn bluetooth::BluetoothIntegration>)
                .unwrap_or_else(|| Rc::new(bluetooth::Fallback)),
        }
    }
}
