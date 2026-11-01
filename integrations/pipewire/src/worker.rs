use crate::snapshot::{self, Snapshot};
use coconut_api::volume::AudioDirection;
use std::collections::VecDeque;
use std::sync::{mpsc::SyncSender, Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(super) enum Job {
    Refresh,
    Volume(String, f32),
    Mute(String, bool),
    Default(String, AudioDirection),
}

impl Job {
    fn replaces(&self, previous: &Self) -> bool {
        match (self, previous) {
            (Self::Refresh, Self::Refresh) => true,
            (Self::Volume(a, _), Self::Volume(b, _)) | (Self::Mute(a, _), Self::Mute(b, _)) => {
                a == b
            }
            (Self::Default(_, a), Self::Default(_, b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Default)]
struct Pending {
    jobs: VecDeque<Job>,
    closed: bool,
}

#[derive(Default)]
pub(super) struct Queue {
    pending: Mutex<Pending>,
    ready: Condvar,
}

impl Queue {
    pub fn push(&self, job: Job) -> Result<(), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "Audio worker is unavailable")?;
        if pending.closed {
            return Err("Audio worker has stopped".into());
        }
        pending.jobs.retain(|previous| !job.replaces(previous));
        if pending.jobs.len() >= 128 {
            return Err("Audio controls are busy. Try again.".into());
        }
        pending.jobs.push_back(job);
        self.ready.notify_one();
        Ok(())
    }
    pub fn stop(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.closed = true;
            self.ready.notify_all();
        }
    }
    fn take(&self) -> Option<Vec<Job>> {
        let mut pending = self.pending.lock().ok()?;
        while pending.jobs.is_empty() && !pending.closed {
            pending = self.ready.wait(pending).ok()?;
        }
        if pending.closed {
            return None;
        }
        drop(pending);
        std::thread::sleep(Duration::from_millis(60));
        let mut pending = self.pending.lock().ok()?;
        if pending.closed {
            return None;
        }
        Some(pending.jobs.drain(..).collect())
    }
}

pub(super) fn run(queue: Arc<Queue>, state: Arc<Mutex<Snapshot>>, changes: SyncSender<()>) {
    while let Some(jobs) = queue.take() {
        let mut next = match snapshot::read() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let mut next = state.lock().map(|state| state.clone()).unwrap_or_default();
                next.loading = false;
                next.error = Some(error);
                for job in &jobs {
                    match job {
                        Job::Volume(id, _) | Job::Mute(id, _) if id == "@DEFAULT_AUDIO_SINK@" => {
                            if let Err(error) = execute(job, &Snapshot::default()) {
                                next.error = Some(error);
                            }
                        }
                        Job::Refresh => {}
                        _ => {
                            next.error =
                                Some("Audio device inventory is unavailable. Try again.".into())
                        }
                    }
                }
                if let Some((level, muted)) = super::volume_output() {
                    next.level = level;
                    next.muted = muted;
                }
                if let Ok(mut current) = state.lock() {
                    if *current != next {
                        *current = next;
                        let _ = changes.try_send(());
                    }
                }
                continue;
            }
        };
        let editing = jobs.iter().any(|job| !matches!(job, Job::Refresh));
        let mut error = None;
        for job in jobs.iter().filter(|job| !matches!(job, Job::Refresh)) {
            if let Err(message) = execute(job, &next) {
                eprintln!("pipewire: {message}");
                error = Some(message);
            }
        }
        if editing {
            match snapshot::read() {
                Ok(snapshot) => next = snapshot,
                Err(message) => error = Some(message),
            }
        }
        if !editing {
            error = state
                .lock()
                .ok()
                .and_then(|snapshot| snapshot.error.clone());
        }
        next.error = error;
        if let Ok(mut snapshot) = state.lock() {
            if *snapshot != next {
                *snapshot = next;
                let _ = changes.try_send(());
            }
        }
    }
}

fn execute(job: &Job, snapshot: &Snapshot) -> Result<(), String> {
    let resolve = |serial: &str| -> Result<String, String> {
        if serial == "@DEFAULT_AUDIO_SINK@" {
            return Ok(serial.into());
        }
        snapshot
            .nodes
            .iter()
            .find(|node| node.serial == serial)
            .map(|node| node.global_id.to_string())
            .ok_or_else(|| "The audio device or application is no longer available".into())
    };
    let args = match job {
        Job::Refresh => return Ok(()),
        Job::Volume(id, level) => vec![
            "set-volume".into(),
            resolve(id)?,
            format!("{:.3}", level.clamp(0.0, 1.0)),
        ],
        Job::Mute(id, muted) => vec![
            "set-mute".into(),
            resolve(id)?,
            if *muted { "1" } else { "0" }.into(),
        ],
        Job::Default(id, direction) => {
            if !snapshot
                .nodes
                .iter()
                .any(|node| node.serial == *id && !node.stream && node.direction == *direction)
            {
                return Err("This audio device is no longer available".into());
            }
            vec!["set-default".into(), resolve(id)?]
        }
    };
    super::command(
        "wpctl",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_slider_updates_are_coalesced_without_dropping_mute_or_defaults() {
        let queue = Queue::default();
        for step in 0..1000 {
            queue
                .push(Job::Volume("900".into(), step as f32 / 1000.0))
                .unwrap();
        }
        queue.push(Job::Mute("900".into(), true)).unwrap();
        queue
            .push(Job::Default("900".into(), AudioDirection::Output))
            .unwrap();
        queue
            .push(Job::Default("901".into(), AudioDirection::Input))
            .unwrap();
        let jobs = queue.take().unwrap();
        assert_eq!(jobs.len(), 4);
        assert!(matches!(&jobs[0], Job::Volume(_, level) if *level == 0.999));
    }
    #[test]
    fn stale_streams_cannot_control_a_reused_global_id() {
        let snapshot = Snapshot::default();
        assert!(execute(&Job::Volume("900".into(), 0.5), &snapshot)
            .unwrap_err()
            .contains("no longer available"));
    }
}
