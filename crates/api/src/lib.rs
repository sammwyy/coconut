#![allow(dead_code)]

pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod desktop;
pub mod network;
pub mod power_profile;
pub mod volume;

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

/// The set of device/service integrations the shell and settings panel
/// consume, wired up by `coconut-registry`'s `detect()` to whichever
/// concrete backends are compiled in.
pub struct IntegrationRegistry {
    pub desktop: Rc<dyn desktop::DesktopIntegration>,
    pub audio: Rc<dyn audio::AudioIntegration>,
    pub brightness: Rc<dyn brightness::BrightnessIntegration>,
    pub battery: Rc<dyn battery::BatteryIntegration>,
    pub volume: Rc<dyn volume::VolumeIntegration>,
    pub network: Rc<dyn network::NetworkIntegration>,
    pub bluetooth: Rc<dyn bluetooth::BluetoothIntegration>,
    pub power_profile: Rc<dyn power_profile::PowerProfileIntegration>,
}

pub type Registry = IntegrationRegistry;
