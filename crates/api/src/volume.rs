#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioDirection {
    Output,
    Input,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub description: String,
    pub direction: AudioDirection,
    pub level: f32,
    pub muted: bool,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioStream {
    pub id: String,
    pub name: String,
    pub description: String,
    pub direction: AudioDirection,
    pub level: f32,
    pub muted: bool,
}

pub trait VolumeIntegration {
    fn level(&self) -> f32;
    fn muted(&self) -> bool;
    fn set_level(&self, level: f32);
    fn supports_mute(&self) -> bool {
        false
    }
    fn loading(&self) -> bool {
        false
    }
    fn set_muted(&self, _muted: bool) {}
    fn devices(&self) -> Vec<AudioDevice> {
        vec![]
    }
    fn streams(&self) -> Vec<AudioStream> {
        vec![]
    }
    fn set_default_device(&self, _id: &str) {}
    fn set_device_level(&self, _id: &str, _level: f32) {}
    fn set_device_muted(&self, _id: &str, _muted: bool) {}
    fn set_stream_level(&self, _id: &str, _level: f32) {}
    fn set_stream_muted(&self, _id: &str, _muted: bool) {}
    fn last_error(&self) -> Option<String> {
        None
    }
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }
}

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
