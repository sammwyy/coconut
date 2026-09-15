use super::VolumeIntegration;
use crate::integrations::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use pipewire::context::ContextRc;
use pipewire::main_loop::MainLoopRc;
use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::node::{Node, NodeListener};
use pipewire::spa::param::ParamType;
use pipewire::types::ObjectType;
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

/// PipeWire's native session-manager client. `wpctl` talks to WirePlumber for
/// reading and writing the default sink's volume, matching what it displays;
/// a direct libpipewire connection (FFI, linked via `pipewire-devel`) only
/// supplies the native hook that wakes a re-read instead of polling.
#[derive(Default)]
struct State {
    level: f32,
    muted: bool,
}

pub struct PipeWire {
    state: Arc<Mutex<State>>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl PipeWire {
    pub fn detect() -> Option<Self> {
        volume_output().map(|_| Self::new())
    }

    fn new() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        refresh(&state);
        let cache = state.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = run_event_bridge(cache, change_tx, shutdown_rx) {
                eprintln!("pipewire: {error}");
            }
        });
        Self {
            state,
            changes,
            _bridge: bridge,
        }
    }
}

impl VolumeIntegration for PipeWire {
    fn level(&self) -> f32 {
        self.state.lock().map(|state| state.level).unwrap_or(0.0)
    }

    fn muted(&self) -> bool {
        self.state.lock().map(|state| state.muted).unwrap_or(false)
    }

    fn set_level(&self, level: f32) {
        let value = format!("{:.3}", level.clamp(0.0, 1.0));
        if let Ok(mut state) = self.state.lock() {
            state.level = level.clamp(0.0, 1.0);
        }
        thread::spawn(move || {
            let _ = wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", &value]);
        });
    }

    fn changes(&self) -> Option<ChangeListener> {
        Some(self.changes.clone())
    }
}

fn run_event_bridge(
    state: Arc<Mutex<State>>,
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
    let main_loop_weak = main_loop.downgrade();
    let _quit_receiver = quit_rx.attach(main_loop.loop_(), move |_| {
        if let Some(main_loop) = main_loop_weak.upgrade() {
            main_loop.quit();
        }
    });

    let nodes: Rc<RefCell<HashMap<u32, (Node, NodeListener)>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let default_sink_metadata: Rc<RefCell<Option<(Metadata, MetadataListener)>>> =
        Rc::new(RefCell::new(None));

    let bind_registry = registry.clone();
    let global_nodes = nodes.clone();
    let global_metadata = default_sink_metadata.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| match global.type_ {
            ObjectType::Node => {
                let is_sink =
                    global.props.and_then(|props| props.get("media.class")) == Some("Audio/Sink");
                if !is_sink {
                    return;
                }
                let Ok(node) = bind_registry.bind::<Node, _>(global) else {
                    return;
                };
                let cache = state.clone();
                let change_tx = change_tx.clone();
                let listener = node
                    .add_listener_local()
                    .param(move |_seq, param_type, _index, _next, _param| {
                        if param_type == ParamType::Props {
                            refresh(&cache);
                            let _ = change_tx.try_send(());
                        }
                    })
                    .register();
                node.subscribe_params(&[ParamType::Props]);
                global_nodes
                    .borrow_mut()
                    .insert(global.id, (node, listener));
            }
            ObjectType::Metadata => {
                let is_default =
                    global.props.and_then(|props| props.get("metadata.name")) == Some("default");
                if !is_default {
                    return;
                }
                let Ok(metadata) = bind_registry.bind::<Metadata, _>(global) else {
                    return;
                };
                let cache = state.clone();
                let change_tx = change_tx.clone();
                let listener = metadata
                    .add_listener_local()
                    .property(move |_subject, key, _type_, _value| {
                        if key == Some("default.audio.sink") {
                            refresh(&cache);
                            let _ = change_tx.try_send(());
                        }
                        0
                    })
                    .register();
                *global_metadata.borrow_mut() = Some((metadata, listener));
            }
            _ => {}
        })
        .global_remove(move |id| {
            nodes.borrow_mut().remove(&id);
        })
        .register();

    main_loop.run();
    Ok(())
}

fn refresh(state: &Arc<Mutex<State>>) {
    if let Some((level, muted)) = volume_output() {
        if let Ok(mut current) = state.lock() {
            *current = State { level, muted };
        }
    }
}

fn volume_output() -> Option<(f32, bool)> {
    let output = wpctl(&["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    let level = output
        .split_whitespace()
        .find_map(|part| part.parse::<f32>().ok())?;
    Some((level.clamp(0.0, 1.0), output.contains("MUTED")))
}

fn wpctl(args: &[&str]) -> Option<String> {
    Command::new("timeout")
        .args(["2s", "wpctl"])
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
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
