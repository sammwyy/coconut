#[cfg(not(target_os = "windows"))]
mod pipewire;
#[cfg(target_os = "windows")]
mod windows;

pub trait VolumeIntegration {
    fn level(&self) -> f32;
    fn muted(&self) -> bool;
    fn set_level(&self, level: f32);
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use pipewire::PipeWire;
#[cfg(target_os = "windows")]
pub use windows::WindowsVolume;
pub struct Fallback;
impl VolumeIntegration for Fallback {
    fn level(&self) -> f32 {
        0.0
    }
    fn muted(&self) -> bool {
        false
    }
    fn set_level(&self, _: f32) {}
}
