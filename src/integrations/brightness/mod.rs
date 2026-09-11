mod brightnessctl;

pub trait BrightnessIntegration {
    fn level(&self) -> f32;
    fn set_level(&self, level: f32);
}

pub use brightnessctl::BrightnessCtl;
pub struct Fallback;
impl BrightnessIntegration for Fallback {
    fn level(&self) -> f32 {
        1.0
    }
    fn set_level(&self, _: f32) {}
}
