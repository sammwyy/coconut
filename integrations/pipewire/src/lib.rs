mod snapshot;
mod worker;

use coconut_api::volume::{AudioDevice, AudioStream, VolumeIntegration};
use coconut_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use pipewire::context::ContextRc;
use pipewire::main_loop::MainLoopRc;
use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::node::{Node, NodeListener};
use pipewire::spa::param::ParamType;
use pipewire::types::ObjectType;
use snapshot::Snapshot;
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use worker::{Job, Queue};

pub struct PipeWire {
    state: Arc<Mutex<Snapshot>>,
    jobs: Arc<Queue>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl PipeWire {
    pub fn detect() -> Option<Self> {
        volume_output().map(|(level, muted)| Self::new(level, muted))
    }

    fn new(level: f32, muted: bool) -> Self {
        let initial = Snapshot {
            level,
            muted,
            loading: true,
            ..Default::default()
        };
        let state = Arc::new(Mutex::new(initial));
        let jobs = Arc::new(Queue::default());
        let cache = state.clone();
        let events = jobs.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            let queue = events.clone();
            let updates = change_tx.clone();
            let worker = thread::spawn(move || worker::run(queue, cache, updates));
            if let Err(error) = run_event_bridge(events.clone(), change_tx, shutdown_rx) {
                eprintln!("pipewire: {error}");
            }
            events.stop();
            if worker.join().is_err() {
                eprintln!("pipewire: audio worker stopped unexpectedly");
            }
        });
        Self {
            state,
            jobs,
            changes,
            _bridge: bridge,
        }
    }

    fn submit(&self, job: Job) {
        if let Ok(mut state) = self.state.lock() {
            state.error = None;
            match &job {
                Job::Volume(id, level) if id == "@DEFAULT_AUDIO_SINK@" => state.level = *level,
                Job::Mute(id, muted) if id == "@DEFAULT_AUDIO_SINK@" => state.muted = *muted,
                Job::Volume(id, level) => {
                    if let Some(node) = state.nodes.iter_mut().find(|node| node.serial == *id) {
                        node.level = *level;
                    }
                }
                Job::Mute(id, muted) => {
                    if let Some(node) = state.nodes.iter_mut().find(|node| node.serial == *id) {
                        node.muted = *muted;
                    }
                }
                Job::Default(id, direction) => {
                    for node in state
                        .nodes
                        .iter_mut()
                        .filter(|node| !node.stream && node.direction == *direction)
                    {
                        node.active = node.serial == *id;
                    }
                }
                Job::Refresh => {}
            }
        }
        if let Err(error) = self.jobs.push(job) {
            eprintln!("pipewire: {error}");
            if let Ok(mut state) = self.state.lock() {
                state.error = Some(error);
            }
        }
    }
}

impl VolumeIntegration for PipeWire {
    fn level(&self) -> f32 {
        self.state.lock().map(|state| state.level).unwrap_or(0.0)
    }
    fn muted(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.muted)
    }
    fn set_level(&self, level: f32) {
        self.submit(Job::Volume(
            "@DEFAULT_AUDIO_SINK@".into(),
            level.clamp(0.0, 1.0),
        ));
    }
    fn supports_mute(&self) -> bool {
        true
    }
    fn loading(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.loading)
    }
    fn set_muted(&self, muted: bool) {
        self.submit(Job::Mute("@DEFAULT_AUDIO_SINK@".into(), muted));
    }
    fn devices(&self) -> Vec<AudioDevice> {
        self.state
            .lock()
            .map(|state| state.devices())
            .unwrap_or_default()
    }
    fn streams(&self) -> Vec<AudioStream> {
        self.state
            .lock()
            .map(|state| state.streams())
            .unwrap_or_default()
    }
    fn set_default_device(&self, id: &str) {
        let direction = self.state.lock().ok().and_then(|state| {
            state
                .nodes
                .iter()
                .find(|node| node.serial == id && !node.stream)
                .map(|node| node.direction)
        });
        if let Some(direction) = direction {
            self.submit(Job::Default(id.into(), direction));
        }
    }
    fn set_device_level(&self, id: &str, level: f32) {
        self.submit(Job::Volume(id.into(), level.clamp(0.0, 1.0)));
    }
    fn set_device_muted(&self, id: &str, muted: bool) {
        self.submit(Job::Mute(id.into(), muted));
    }
    fn set_stream_level(&self, id: &str, level: f32) {
        self.submit(Job::Volume(id.into(), level.clamp(0.0, 1.0)));
    }
    fn set_stream_muted(&self, id: &str, muted: bool) {
        self.submit(Job::Mute(id.into(), muted));
    }
    fn last_error(&self) -> Option<String> {
        self.state.lock().ok().and_then(|state| state.error.clone())
    }
    fn changes(&self) -> Option<ChangeListener> {
        Some(self.changes.clone())
    }
}

fn run_event_bridge(
    jobs: Arc<Queue>,
    change_tx: SyncSender<()>,
    shutdown_rx: Receiver<()>,
) -> Result<(), String> {
    pipewire::init();
    let main_loop = MainLoopRc::new(None).map_err(|error| error.to_string())?;
    let context = ContextRc::new(&main_loop, None).map_err(|error| error.to_string())?;
    let core = context
        .connect_rc(None)
        .map_err(|error| error.to_string())?;
    let registry = core.get_registry_rc().map_err(|error| error.to_string())?;
    let (quit_tx, quit_rx) = pipewire::channel::channel::<()>();
    thread::spawn(move || {
        let _ = shutdown_rx.recv();
        let _ = quit_tx.send(());
    });
    let weak = main_loop.downgrade();
    let _quit_receiver = quit_rx.attach(main_loop.loop_(), move |_| {
        if let Some(main_loop) = weak.upgrade() {
            main_loop.quit();
        }
    });
    let nodes: Rc<RefCell<HashMap<u32, (Node, NodeListener)>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let metadata: Rc<RefCell<Option<(Metadata, MetadataListener)>>> = Rc::new(RefCell::new(None));
    let bind_registry = registry.clone();
    let global_nodes = nodes.clone();
    let global_jobs = jobs.clone();
    let removed_jobs = jobs.clone();
    let _listener = registry
        .add_listener_local()
        .global(move |global| match global.type_ {
            ObjectType::Node => {
                let class = global
                    .props
                    .and_then(|props| props.get("media.class"))
                    .unwrap_or("");
                if !matches!(
                    class,
                    "Audio/Sink" | "Audio/Source" | "Stream/Output/Audio" | "Stream/Input/Audio"
                ) {
                    return;
                }
                let Ok(node) = bind_registry.bind::<Node, _>(global) else {
                    return;
                };
                let queue = global_jobs.clone();
                let info_queue = queue.clone();
                let listener = node
                    .add_listener_local()
                    .info(move |_| {
                        let _ = info_queue.push(Job::Refresh);
                    })
                    .param(move |_, param_type, _, _, _| {
                        if param_type == ParamType::Props {
                            let _ = queue.push(Job::Refresh);
                        }
                    })
                    .register();
                node.subscribe_params(&[ParamType::Props]);
                global_nodes
                    .borrow_mut()
                    .insert(global.id, (node, listener));
                let _ = global_jobs.push(Job::Refresh);
            }
            ObjectType::Metadata => {
                if global.props.and_then(|props| props.get("metadata.name")) != Some("default") {
                    return;
                }
                let Ok(proxy) = bind_registry.bind::<Metadata, _>(global) else {
                    return;
                };
                let queue = global_jobs.clone();
                let listener = proxy
                    .add_listener_local()
                    .property(move |_, key, _, _| {
                        if matches!(key, Some("default.audio.sink" | "default.audio.source")) {
                            let _ = queue.push(Job::Refresh);
                        }
                        0
                    })
                    .register();
                *metadata.borrow_mut() = Some((proxy, listener));
            }
            _ => {}
        })
        .global_remove(move |id| {
            if nodes.borrow_mut().remove(&id).is_some() {
                let _ = removed_jobs.push(Job::Refresh);
            }
        })
        .register();
    let _ = change_tx.try_send(());
    main_loop.run();
    Ok(())
}

fn volume_output() -> Option<(f32, bool)> {
    snapshot::parse_volume(&command("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]).ok()?)
}

fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("timeout")
        .args(["2s", program])
        .args(args)
        .output()
        .map_err(|error| format!("Could not run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{PipeWire, VolumeIntegration};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a running PipeWire session and toggles its default sink volume"]
    fn reacts_to_an_external_volume_change() {
        let volume = PipeWire::detect().expect("a PipeWire session");
        sleep(Duration::from_millis(200));
        let initial = volume.level();
        let target = if initial > 0.5 {
            initial - 0.1
        } else {
            initial + 0.1
        };
        let listener = volume.changes().expect("a native change listener");
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = result_tx.send(listener.wait());
        });
        let _ = std::process::Command::new("wpctl")
            .args([
                "set-volume",
                "@DEFAULT_AUDIO_SINK@",
                &format!("{target:.2}"),
            ])
            .status();
        assert_eq!(result_rx.recv_timeout(Duration::from_secs(10)), Ok(true));
        sleep(Duration::from_millis(200));
        assert!((volume.level() - target).abs() < 0.05);

        let _ = std::process::Command::new("wpctl")
            .args([
                "set-volume",
                "@DEFAULT_AUDIO_SINK@",
                &format!("{initial:.2}"),
            ])
            .status();
    }
}
