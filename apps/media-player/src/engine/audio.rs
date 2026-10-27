//! Audio output backed by a `rodio::Player` and a channel-fed `Source`.

use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use super::decode::{AUDIO_CHANNELS, AUDIO_RATE};

#[derive(Clone, Default)]
pub struct AudioClock {
    /// Total interleaved `f32` samples delivered to the mixer across all channels.
    pub played: Arc<AtomicU64>,
    /// Set once the producer disconnects and all buffered samples are consumed.
    pub drained: Arc<AtomicBool>,
}

pub struct AudioOutput {
    _device: MixerDeviceSink,
    pub player: Player,
}

impl AudioOutput {
    pub fn open() -> Option<Self> {
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| eprintln!("cplayer: no audio output device: {e}"))
            .ok()?;
        let player = Player::connect_new(device.mixer());
        Some(AudioOutput {
            _device: device,
            player,
        })
    }

    pub fn play_stream(&self, rx: Receiver<Vec<u8>>) -> AudioClock {
        let clock = AudioClock::default();
        let source = PipeSource {
            rx,
            pending: Vec::new(),
            cursor: 0,
            closed: false,
            played: clock.played.clone(),
            drained: clock.drained.clone(),
        };
        self.player.stop();
        self.player.append(source);
        clock
    }
}

struct PipeSource {
    rx: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    cursor: usize,
    closed: bool,
    played: Arc<AtomicU64>,
    drained: Arc<AtomicBool>,
}

impl Iterator for PipeSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if self.cursor + 4 <= self.pending.len() {
                let bytes: [u8; 4] = self.pending[self.cursor..self.cursor + 4]
                    .try_into()
                    .unwrap();
                self.cursor += 4;
                self.played.fetch_add(1, Ordering::Relaxed);
                return Some(f32::from_le_bytes(bytes));
            }
            self.pending.clear();
            self.cursor = 0;
            if self.closed {
                self.drained.store(true, Ordering::Relaxed);
                return None;
            }
            match self.rx.try_recv() {
                Ok(chunk) => self.pending = chunk,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Emit silence on underrun while the producer is connected so rodio
                    // does not treat a temporary decoder stall as end-of-stream.
                    return Some(0.0);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.closed = true,
            }
        }
    }
}

impl Source for PipeSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::new(AUDIO_CHANNELS as u16).unwrap()
    }

    fn sample_rate(&self) -> std::num::NonZero<u32> {
        std::num::NonZero::new(AUDIO_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    #[test]
    fn underrun_does_not_advance_media_clock() {
        let (tx, rx) = sync_channel(1);
        let mut output = PipeSource {
            rx,
            pending: Vec::new(),
            cursor: 0,
            closed: false,
            played: Arc::new(AtomicU64::new(0)),
            drained: Arc::new(AtomicBool::new(false)),
        };
        assert_eq!(output.next(), Some(0.0));
        assert_eq!(output.played.load(Ordering::Relaxed), 0);
        tx.send(1.0f32.to_le_bytes().to_vec()).unwrap();
        assert_eq!(output.next(), Some(1.0));
        assert_eq!(output.played.load(Ordering::Relaxed), 1);
        drop(tx);
        assert_eq!(output.next(), None);
        assert!(output.drained.load(Ordering::Relaxed));
    }
}
