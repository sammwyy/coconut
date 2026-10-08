use blair_client::BlairClient;
use coconut_core::ShellShortcut;
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct ShortcutListener {
    events: Arc<Mutex<Receiver<String>>>,
}

impl ShortcutListener {
    pub fn start(bindings: &[ShellShortcut]) -> Option<Self> {
        let bindings = bindings.to_vec();
        if std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|desktop| desktop.to_ascii_lowercase().contains("kde"))
        {
            let bindings = bindings
                .into_iter()
                .enumerate()
                .map(|(index, binding)| (index.to_string(), binding.accelerator))
                .collect();
            return coconut_integration_kwin::register_global_shortcuts(bindings).map(|events| {
                Self {
                    events: Arc::new(Mutex::new(events)),
                }
            });
        }
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async move {
                let Ok(client) = BlairClient::connect().await else {
                    return;
                };
                for (index, binding) in bindings.iter().enumerate() {
                    if !binding.accelerator.is_empty() {
                        let _ = client
                            .bind_shortcut(&index.to_string(), &binding.accelerator)
                            .await;
                    }
                }
                let Ok(mut events) = client.events().await else {
                    return;
                };
                while let Some(event) = events.next().await {
                    if let blair_protocol::CompositorEvent::ShortcutActivated { id, .. } = event {
                        if sender.send(id).is_err() {
                            break;
                        }
                    }
                }
            });
        });
        Some(Self {
            events: Arc::new(Mutex::new(events)),
        })
    }

    pub fn next(&self) -> Option<usize> {
        self.events.lock().ok()?.recv().ok()?.parse().ok()
    }
}
