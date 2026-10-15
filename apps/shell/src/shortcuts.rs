use blair_client::{BlairClient, Events};
use coconut_core::{CustomShortcut, ShortcutAction, ShortcutConfig};
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};
use tokio::sync::{mpsc as async_channel, oneshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutEvent {
    BuiltIn(ShortcutAction),
    Custom(CustomShortcut),
}

fn action(config: &ShortcutConfig, id: &str) -> Option<ShortcutEvent> {
    let index: usize = id.parse().ok()?;
    if let Some(binding) = config.shortcuts.get(index) {
        Some(ShortcutEvent::BuiltIn(binding.action))
    } else {
        config
            .custom_shortcuts
            .get(index.checked_sub(config.shortcuts.len())?)
            .cloned()
            .map(ShortcutEvent::Custom)
    }
}

struct ApplyRequest {
    config: ShortcutConfig,
    response: oneshot::Sender<Result<(), String>>,
}

struct ShortcutService {
    requests: async_channel::Sender<ApplyRequest>,
    status: Arc<Mutex<String>>,
}

#[zbus::interface(name = "org.coconut.Shortcuts1")]
impl ShortcutService {
    async fn apply(&self, document: &str) -> zbus::fdo::Result<()> {
        let config: ShortcutConfig = toml::from_str(document)
            .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;
        config.validate().map_err(zbus::fdo::Error::InvalidArgs)?;
        let (response, result) = oneshot::channel();
        self.requests
            .send(ApplyRequest { config, response })
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Shortcut service stopped".into()))?;
        result
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Shortcut service stopped".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }

    fn status(&self) -> String {
        self.status.lock().expect("shortcut status").clone()
    }
}

struct Registration {
    client: BlairClient,
    events: Events,
    config: ShortcutConfig,
}

impl Registration {
    async fn create(config: ShortcutConfig) -> Result<Self, String> {
        config.validate()?;
        let client = BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?;
        let events = client.events().await.map_err(|error| error.to_string())?;
        for (index, accelerator) in config.accelerators().enumerate() {
            if accelerator.is_empty() {
                continue;
            }
            match client.bind_shortcut(&index.to_string(), accelerator).await {
                Ok(true) => {}
                Ok(false) => {
                    return Err(format!(
                        "Blair rejected shortcut {} ({accelerator})",
                        index + 1
                    ))
                }
                Err(error) => return Err(format!("Could not register {accelerator}: {error}")),
            }
        }
        Ok(Self {
            client,
            events,
            config,
        })
    }

    async fn unbind(&self) {
        for (index, accelerator) in self.config.accelerators().enumerate() {
            if !accelerator.is_empty() {
                if let Err(error) = self.client.unbind_shortcut(&index.to_string()).await {
                    eprintln!("shortcuts: could not unbind {accelerator}: {error}");
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ShortcutListener {
    events: Arc<Mutex<Receiver<ShortcutEvent>>>,
    _lifetime: Arc<ListenerLifetime>,
}

struct ListenerLifetime(Mutex<Option<oneshot::Sender<()>>>);

impl Drop for ListenerLifetime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.0.lock().expect("shortcut lifetime").take() {
            let _ = shutdown.send(());
        }
    }
}

impl ShortcutListener {
    pub fn start(config: ShortcutConfig) -> Option<Self> {
        let (sender, events) = mpsc::channel();
        let (shutdown, mut stopped) = oneshot::channel();
        let lifetime = Arc::new(ListenerLifetime(Mutex::new(Some(shutdown))));
        if std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|desktop| desktop.to_ascii_lowercase().contains("kde"))
        {
            let bindings = config
                .accelerators()
                .enumerate()
                .map(|(index, accelerator)| (index.to_string(), accelerator.to_owned()))
                .collect();
            let registered = coconut_integration_kwin::register_global_shortcuts(bindings)?;
            std::thread::spawn(move || {
                while let Ok(id) = registered.recv() {
                    if let Some(event) = action(&config, &id) {
                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            });
            return Some(Self {
                events: Arc::new(Mutex::new(events)),
                _lifetime: lifetime,
            });
        }
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("shortcuts: could not start service: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                let (mut current, initial_status) = match Registration::create(config).await {
                    Ok(registration) => (Some(registration), "Shortcuts active".to_owned()),
                    Err(error) => { eprintln!("shortcuts: {error}"); (None, error) }
                };
                let status = Arc::new(Mutex::new(initial_status));
                let (requests, mut receiver) = async_channel::channel::<ApplyRequest>(8);
                let service = ShortcutService { requests, status: status.clone() };
                let connection = match zbus::connection::Builder::session()
                    .and_then(|builder| builder.name("org.coconut.Shortcuts"))
                    .and_then(|builder| builder.serve_at("/org/coconut/Shortcuts", service))
                {
                    Ok(builder) => match builder.build().await {
                        Ok(connection) => Some(connection),
                        Err(error) => { eprintln!("shortcuts: live settings unavailable: {error}"); None }
                    },
                    Err(error) => { eprintln!("shortcuts: live settings unavailable: {error}"); None }
                };
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        request = receiver.recv(), if connection.is_some() => {
                            let Some(request) = request else { break };
                            let result = match Registration::create(request.config).await {
                                Ok(next) => match next.config.save() {
                                    Ok(()) => {
                                        if let Some(previous) = current.take() { previous.unbind().await; }
                                        current = Some(next);
                                        Ok(())
                                    }
                                    Err(error) => { next.unbind().await; Err(format!("Could not save shortcuts: {error}")) }
                                },
                                Err(error) => Err(error),
                            };
                            *status.lock().expect("shortcut status") = match &result {
                                Ok(()) => "Shortcuts active".into(),
                                Err(error) => error.clone(),
                            };
                            let _ = request.response.send(result);
                        }
                        event = async {
                            match current.as_mut() {
                                Some(registration) => registration.events.next().await,
                                None => std::future::pending().await,
                            }
                        } => {
                            let Some(event) = event else {
                                current = None;
                                *status.lock().expect("shortcut status") = "Blair event stream disconnected; save to retry".into();
                                continue;
                            };
                            if let blair_protocol::CompositorEvent::ShortcutActivated { id, .. } = event {
                                if let Some(event) = current.as_ref().and_then(|entry| action(&entry.config, &id)) {
                                    if sender.send(event).is_err() { break; }
                                }
                            }
                        }
                    }
                    if current.is_none() && connection.is_none() { break; }
                }
            });
        });
        Some(Self {
            events: Arc::new(Mutex::new(events)),
            _lifetime: lifetime,
        })
    }

    pub fn next(&self) -> Option<ShortcutEvent> {
        self.events.lock().ok()?.recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCompositor {
        bindings: Arc<Mutex<std::collections::HashMap<(String, String), String>>>,
    }

    #[zbus::interface(name = "org.blair.Compositor1")]
    impl MockCompositor {
        fn bind_shortcut(
            &self,
            id: &str,
            accelerator: &str,
            #[zbus(header)] header: zbus::message::Header<'_>,
        ) -> bool {
            if accelerator == "Super+F5" {
                return false;
            }
            self.bindings.lock().unwrap().insert(
                (header.sender().unwrap().to_string(), id.into()),
                accelerator.into(),
            );
            true
        }

        fn unbind_shortcut(&self, id: &str, #[zbus(header)] header: zbus::message::Header<'_>) {
            self.bindings
                .lock()
                .unwrap()
                .remove(&(header.sender().unwrap().to_string(), id.into()));
        }

        #[zbus(signal)]
        async fn shortcut_activated(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            id: &str,
        ) -> zbus::Result<()>;
    }

    #[test]
    #[ignore = "requires an isolated session bus and configuration directory"]
    fn live_service_replaces_bindings_and_retains_the_old_generation_on_rejection() {
        assert_eq!(std::env::var("COCONUT_SHORTCUT_TEST").as_deref(), Ok("1"));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let bindings = Arc::new(Mutex::new(std::collections::HashMap::new()));
            let server = zbus::connection::Builder::session()
                .unwrap()
                .name("org.blair.Compositor")
                .unwrap()
                .serve_at(
                    "/org/blair/Compositor",
                    MockCompositor {
                        bindings: bindings.clone(),
                    },
                )
                .unwrap()
                .build()
                .await
                .unwrap();
            let listener = ShortcutListener::start(ShortcutConfig::default()).unwrap();
            let dbus = zbus::fdo::DBusProxy::new(&server).await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while !dbus
                    .name_has_owner("org.coconut.Shortcuts".try_into().unwrap())
                    .await
                    .unwrap()
                {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let previous = bindings.lock().unwrap().keys().next().unwrap().0.clone();
            let proxy = zbus::Proxy::new(
                &server,
                "org.coconut.Shortcuts",
                "/org/coconut/Shortcuts",
                "org.coconut.Shortcuts1",
            )
            .await
            .unwrap();
            let config = ShortcutConfig {
                shortcuts: vec![coconut_core::ShellShortcut {
                    accelerator: "Super+K".into(),
                    action: ShortcutAction::CloseFocused,
                }],
                custom_shortcuts: Vec::new(),
            };
            proxy
                .call::<_, _, ()>("Apply", &(toml::to_string(&config).unwrap(),))
                .await
                .unwrap();
            assert_eq!(ShortcutConfig::load(), config);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while dbus
                    .name_has_owner(previous.as_str().try_into().unwrap())
                    .await
                    .unwrap()
                {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("old event stream releases its D-Bus connection without another event");
            let current = bindings
                .lock()
                .unwrap()
                .iter()
                .find(|(_, accelerator)| accelerator.as_str() == "Super+K")
                .unwrap()
                .0
                 .0
                .clone();
            let mut rejected = config.clone();
            rejected.shortcuts[0].accelerator = "Super+T".into();
            rejected.shortcuts.push(coconut_core::ShellShortcut {
                accelerator: "Super+F5".into(),
                action: ShortcutAction::NextWindow,
            });
            assert!(proxy
                .call::<_, _, ()>("Apply", &(toml::to_string(&rejected).unwrap(),))
                .await
                .is_err());
            assert_eq!(ShortcutConfig::load(), config);
            let status: String = proxy.call("Status", &()).await.unwrap();
            assert!(status.contains("rejected"), "{status}");
            let emitter = zbus::object_server::SignalEmitter::new(&server, "/org/blair/Compositor")
                .unwrap()
                .set_destination(current.as_str().try_into().unwrap());
            MockCompositor::shortcut_activated(&emitter, "0")
                .await
                .unwrap();
            assert_eq!(
                listener
                    .events
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap(),
                ShortcutEvent::BuiltIn(ShortcutAction::CloseFocused)
            );
            drop(listener);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while dbus
                    .name_has_owner("org.coconut.Shortcuts".try_into().unwrap())
                    .await
                    .unwrap()
                {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn dispatch_uses_the_registered_generation_not_a_file_snapshot() {
        let mut config = ShortcutConfig::default();
        config.shortcuts[0].action = ShortcutAction::CloseFocused;
        config.custom_shortcuts.push(CustomShortcut {
            accelerator: "Ctrl+Alt+T".into(),
            command: "terminal".into(),
            argument: "--new".into(),
        });
        assert_eq!(
            action(&config, "0"),
            Some(ShortcutEvent::BuiltIn(ShortcutAction::CloseFocused))
        );
        assert_eq!(
            action(&config, &config.shortcuts.len().to_string()),
            Some(ShortcutEvent::Custom(config.custom_shortcuts[0].clone()))
        );
        assert_eq!(action(&config, "invalid"), None);
        assert_eq!(action(&config, "999"), None);
    }
}
