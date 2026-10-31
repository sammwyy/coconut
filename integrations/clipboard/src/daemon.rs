use crate::{
    config::Config, history::History, wayland::Backend, Entry, Status, HISTORY, PATH, READ,
    SERVICE, WRITE,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::{fdo, interface, message::Header, object_server::SignalEmitter, Connection};

#[derive(Clone)]
struct Clipboard {
    history: Arc<Mutex<History>>,
    backend: Backend,
    directory: PathBuf,
}

impl Clipboard {
    async fn authorize(
        &self,
        permission: &str,
        header: &Header<'_>,
        connection: &Connection,
    ) -> fdo::Result<()> {
        let caller = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?;
        let body = (caller.as_str(), permission);
        let reply = connection.call_method(
            Some("org.coconut.Permissions"),
            "/org/coconut/Permissions1",
            Some("org.coconut.Permissions1"),
            "AuthorizeClipboard",
            &body,
        );
        let result = tokio::time::timeout(Duration::from_secs(185), reply)
            .await
            .map_err(|_| fdo::Error::AccessDenied("Clipboard permission request timed out".into()))?
            .map_err(|error| {
                fdo::Error::AccessDenied(format!("Clipboard permission unavailable: {error}"))
            })?;
        let verdict: String = result.body().deserialize().map_err(failure)?;
        if verdict == "allow" {
            Ok(())
        } else {
            Err(fdo::Error::AccessDenied(format!(
                "Permission denied: {permission}"
            )))
        }
    }

    async fn require_manager(
        &self,
        header: &Header<'_>,
        connection: &Connection,
    ) -> fdo::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let sender = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?;
        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetConnectionUnixProcessID",
                &sender.as_str(),
            )
            .await
            .map_err(failure)?;
        let pid: u32 = reply.body().deserialize().map_err(failure)?;
        let actual = std::fs::metadata(format!("/proc/{pid}/exe")).map_err(failure)?;
        for name in ["coconut", "coconut-settings"] {
            if let Ok(expected) = std::fs::metadata(self.directory.join(name)) {
                if expected.is_file()
                    && actual.dev() == expected.dev()
                    && actual.ino() == expected.ino()
                {
                    return Ok(());
                }
            }
        }
        Err(fdo::Error::AccessDenied(
            "Only the installed shell or Settings may pause collection".into(),
        ))
    }
}

fn failure(error: impl std::fmt::Display) -> fdo::Error {
    eprintln!("clipboard: {error}");
    fdo::Error::Failed(error.to_string())
}

#[interface(name = "org.coconut.Clipboard1")]
impl Clipboard {
    fn status(&self) -> Status {
        self.history.lock().unwrap().status()
    }

    fn get_config(&self) -> (u32, u32, u32, u64) {
        let history = self.history.lock().unwrap();
        let config = &history.config;
        (
            config.max_entries as u32,
            config.max_item_bytes as u32,
            config.max_total_bytes as u32,
            config.max_age_seconds,
        )
    }

    async fn configure(
        &self,
        max_entries: u32,
        max_item_bytes: u32,
        max_total_bytes: u32,
        max_age_seconds: u64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        self.require_manager(&header, connection).await?;
        let mut history = self.history.lock().unwrap();
        let config = Config {
            max_entries: max_entries as usize,
            max_item_bytes: max_item_bytes as usize,
            max_total_bytes: max_total_bytes as usize,
            max_age_seconds,
            ..history.config.clone()
        };
        config
            .validate()
            .map_err(|error| fdo::Error::InvalidArgs(error.to_string()))?;
        config.save(&history.config).map_err(failure)?;
        history.config = config;
        history.prune();
        history.notify();
        Ok(())
    }

    async fn current(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<(bool, String)> {
        self.authorize(READ, &header, connection).await?;
        let history = self.history.lock().unwrap();
        Ok((
            history.current.is_some(),
            history.current.as_deref().unwrap_or_default().to_string(),
        ))
    }

    async fn list(
        &self,
        offset: u32,
        limit: u32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<Vec<Entry>> {
        self.authorize(HISTORY, &header, connection).await?;
        Ok(self
            .history
            .lock()
            .unwrap()
            .entries(offset as usize, limit as usize))
    }

    async fn search(
        &self,
        query: &str,
        offset: u32,
        limit: u32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<Vec<Entry>> {
        self.authorize(HISTORY, &header, connection).await?;
        Ok(self
            .history
            .lock()
            .unwrap()
            .search(query, offset as usize, limit as usize))
    }

    async fn get(
        &self,
        id: u64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<String> {
        self.authorize(HISTORY, &header, connection).await?;
        self.history
            .lock()
            .unwrap()
            .text(id)
            .map(|text| text.to_string())
            .ok_or_else(|| fdo::Error::InvalidArgs("Clipboard entry no longer exists".into()))
    }

    async fn set_text(
        &self,
        text: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        if text.is_empty() || text.len() > self.history.lock().unwrap().config.max_item_bytes {
            return Err(fdo::Error::InvalidArgs(
                "Text is empty or exceeds configured clipboard limit".into(),
            ));
        }
        self.authorize(WRITE, &header, connection).await?;
        self.backend.set_text(text.into()).await.map_err(failure)
    }

    async fn restore(
        &self,
        id: u64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(HISTORY, &header, connection).await?;
        self.authorize(WRITE, &header, connection).await?;
        let text = self.history.lock().unwrap().text(id).ok_or_else(|| {
            fdo::Error::InvalidArgs("Clipboard entry expired or was removed".into())
        })?;
        self.backend.set_text(text).await.map_err(failure)
    }

    async fn remove(
        &self,
        id: u64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<bool> {
        self.authorize(HISTORY, &header, connection).await?;
        Ok(self.history.lock().unwrap().remove(id))
    }

    async fn clear(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        self.authorize(HISTORY, &header, connection).await?;
        self.history.lock().unwrap().clear();
        Ok(())
    }

    async fn set_enabled(
        &self,
        enabled: bool,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        self.require_manager(&header, connection).await?;
        let mut history = self.history.lock().unwrap();
        let config = Config {
            enabled,
            ..history.config.clone()
        };
        config.save(&history.config).map_err(failure)?;
        history.config = config;
        history.notify();
        Ok(())
    }

    async fn wait_changed(&self, revision: u64) -> u64 {
        let mut receiver = self.history.lock().unwrap().changed.subscribe();
        let _ = tokio::time::timeout(Duration::from_secs(25), async {
            loop {
                if *receiver.borrow_and_update() != revision {
                    break;
                }
                if receiver.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
        let revision = *receiver.borrow();
        revision
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, revision: u64) -> zbus::Result<()>;
}

pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    config.validate()?;
    let mut seed = [0; 8];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
    let (changed, mut receiver) = tokio::sync::watch::channel(u64::from_ne_bytes(seed));
    let history = Arc::new(Mutex::new(History::new(config, changed)));
    let backend = Backend::start(history.clone())?;
    let directory = std::env::current_exe()?
        .parent()
        .ok_or("Missing executable directory")?
        .to_path_buf();
    let connection = zbus::connection::Builder::session()?
        .name(SERVICE)?
        .serve_at(
            PATH,
            Clipboard {
                history,
                backend: backend.clone(),
                directory,
            },
        )?
        .build()
        .await?;
    eprintln!("clipboard: session API ready at {SERVICE}");
    let signals = connection.clone();
    tokio::spawn(async move {
        while receiver.changed().await.is_ok() {
            let revision = *receiver.borrow_and_update();
            match SignalEmitter::new(&signals, PATH) {
                Ok(emitter) => {
                    if let Err(error) = Clipboard::changed(&emitter, revision).await {
                        eprintln!("clipboard: cannot notify clients: {error}");
                    }
                }
                Err(error) => eprintln!("clipboard: cannot create signal emitter: {error}"),
            }
        }
    });
    let mut termination =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = termination.recv() => {},
        error = backend.wait_stopped() => return Err(error.into()),
    }
    connection.close().await?;
    Ok(())
}
