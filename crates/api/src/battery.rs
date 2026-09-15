pub trait BatteryIntegration {
    fn percentage(&self) -> Option<u8>;
    fn charging(&self) -> bool;
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

pub struct Fallback;
impl BatteryIntegration for Fallback {
    fn percentage(&self) -> Option<u8> {
        None
    }
    fn charging(&self) -> bool {
        false
    }
}
