#[cfg(not(target_os = "windows"))]
mod brightnessctl;
#[cfg(target_os = "windows")]
mod windows;

pub trait BrightnessIntegration {
    fn level(&self) -> f32;
    fn set_level(&self, level: f32);
    /// A listener that wakes whenever this integration's native hook (a
    /// sysfs/D-Bus notification, a platform event) observes a change.
    /// `None` when the backend has no such hook, so callers fall back to
    /// polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use brightnessctl::BrightnessCtl;
#[cfg(target_os = "windows")]
pub use windows::WindowsBrightness;
pub struct Fallback;
impl BrightnessIntegration for Fallback {
    fn level(&self) -> f32 {
        1.0
    }
    fn set_level(&self, _: f32) {}
}
