mod pipewire;

pub trait VolumeIntegration {
    fn level(&self) -> f32;
    fn muted(&self) -> bool;
    fn set_level(&self, level: f32);
}

pub use pipewire::PipeWire;
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
