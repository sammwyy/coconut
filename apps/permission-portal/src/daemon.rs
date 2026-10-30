use crate::{
    identity,
    store::{store_path, Store, Verdict, SCREEN_CAPTURE},
};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::oneshot;
use zbus::{
    fdo, interface,
    message::Header,
    object_server::{ObjectServer, SignalEmitter},
    zvariant::{ObjectPath, OwnedValue, Value},
    Connection,
};

pub const SERVICE: &str = "org.coconut.Permissions";
pub const PATH: &str = "/org/coconut/Permissions1";
const ACCESS_PATH: &str = "/org/freedesktop/portal/desktop";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(180);

pub type Choices = Vec<(String, String, Vec<(String, String)>, String)>;
type RequestDescription = (String, String, String, String, String, bool, Choices);

#[derive(Clone)]
struct PromptSpec {
    subject: String,
    permission: String,
    title: String,
    body: String,
    deny_label: String,
    grant_label: String,
    offer_always: bool,
    choices: Choices,
}

#[derive(Clone)]
struct Answer {
    verdict: Verdict,
    choices: Vec<(String, String)>,
}

impl Answer {
    fn denied() -> Self {
        Self {
            verdict: Verdict::Deny,
            choices: Vec::new(),
        }
    }
}

struct Pending {
    spec: PromptSpec,
    pair: Option<(String, String)>,
    waiters: Vec<(Option<String>, oneshot::Sender<Answer>)>,
    prompt_pid: Option<u32>,
    stop: Option<oneshot::Sender<()>>,
}

struct NativeRequest {
    owner: String,
    prompt: Option<String>,
    receiver: Option<oneshot::Receiver<Answer>>,
}

struct Daemon {
    store: Mutex<Store>,
    pending: Mutex<HashMap<String, Pending>>,
    native: Mutex<HashMap<String, NativeRequest>>,
    seq: AtomicU64,
    prompt_executable: PathBuf,
    settings: PathBuf,
    prompt_timeout: Duration,
}

fn failure(error: impl std::fmt::Display) -> fdo::Error {
    eprintln!("permission-portal: {error}");
    fdo::Error::Failed(error.to_string())
}

impl Daemon {
    fn new(store: Store, prompt_executable: PathBuf) -> Self {
        let directory = prompt_executable
            .parent()
            .expect("absolute executable path");
        Self {
            store: Mutex::new(store),
            pending: Mutex::new(HashMap::new()),
            native: Mutex::new(HashMap::new()),
            seq: AtomicU64::new(0),
            settings: directory.join("coconut-settings"),
            prompt_timeout: PROMPT_TIMEOUT,
            prompt_executable,
        }
    }

    fn register(
        &self,
        spec: PromptSpec,
        pair: Option<(String, String)>,
    ) -> (String, oneshot::Receiver<Answer>, bool) {
        self.register_for(spec, pair, None)
    }

    fn register_for(
        &self,
        spec: PromptSpec,
        pair: Option<(String, String)>,
        request: Option<String>,
    ) -> (String, oneshot::Receiver<Answer>, bool) {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap();
        if let Some(pair) = &pair {
            if let Some((id, entry)) = pending
                .iter_mut()
                .find(|(_, entry)| entry.pair.as_ref() == Some(pair))
            {
                entry.waiters.push((request, tx));
                return (id.clone(), rx, false);
            }
        }
        let id = format!("coconut-{}", self.seq.fetch_add(1, Ordering::Relaxed));
        pending.insert(
            id.clone(),
            Pending {
                spec,
                pair,
                waiters: vec![(request, tx)],
                prompt_pid: None,
                stop: None,
            },
        );
        (id, rx, true)
    }

    fn deliver(&self, id: &str, answer: Answer) {
        if let Some(pending) = self.pending.lock().unwrap().remove(id) {
            for (_, waiter) in pending.waiters {
                let _ = waiter.send(answer.clone());
            }
        }
    }

    fn spawn_prompt(self: &Arc<Self>, id: String) -> fdo::Result<()> {
        let (stop, stopped) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap();
        let entry = pending
            .get_mut(&id)
            .ok_or_else(|| failure("Request disappeared"))?;
        let mut child = tokio::process::Command::new(&self.prompt_executable)
            .args(["--prompt", &id])
            .kill_on_drop(true)
            .spawn()
            .map_err(failure)?;
        entry.prompt_pid = child.id();
        entry.stop = Some(stop);
        drop(pending);
        let daemon = self.clone();
        let timeout = self.prompt_timeout;
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    if let Err(error) = status { failure(error); }
                }
                result = stopped => {
                    if result.is_err() || tokio::time::timeout(Duration::from_secs(5), child.wait()).await.is_err() {
                        if let Err(error) = child.kill().await { failure(error); }
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    if let Err(error) = child.kill().await { failure(error); }
                }
            }
            daemon.deliver(&id, Answer::denied());
        });
        Ok(())
    }

    fn start(
        self: &Arc<Self>,
        spec: PromptSpec,
        pair: Option<(String, String)>,
    ) -> (String, oneshot::Receiver<Answer>) {
        let (id, rx, fresh) = self.register(spec, pair);
        if fresh {
            if let Err(error) = self.spawn_prompt(id.clone()) {
                failure(error);
                self.deliver(&id, Answer::denied());
            }
        }
        (id, rx)
    }

    fn cancel_native(&self, owner: &str, id: &str) -> bool {
        let request = {
            let mut native = self.native.lock().unwrap();
            if !native.get(id).is_some_and(|request| request.owner == owner) {
                return false;
            }
            native.remove(id).unwrap()
        };
        if let Some(prompt) = request.prompt {
            let mut pending = self.pending.lock().unwrap();
            if let Some(entry) = pending.get_mut(&prompt) {
                entry
                    .waiters
                    .retain(|(request, _)| request.as_deref() != Some(id));
                if entry.waiters.is_empty() {
                    pending.remove(&prompt);
                }
            }
        }
        true
    }

    fn cancel_owner(&self, owner: &str) {
        let ids: Vec<_> = self
            .native
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, request)| request.owner == owner)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.cancel_native(owner, &id);
        }
    }

    async fn emit_changed(connection: &Connection) {
        if let Err(error) =
            Permissions::changed(&SignalEmitter::new(connection, PATH).expect("valid path")).await
        {
            failure(error);
        }
    }

    async fn prompt_pid(
        &self,
        id: &str,
        header: &Header<'_>,
        connection: &Connection,
    ) -> fdo::Result<u32> {
        let pid = identity::caller_pid(header, connection).await?;
        let pending = self.pending.lock().unwrap();
        if pending
            .get(id)
            .is_some_and(|entry| entry.prompt_pid == Some(pid))
        {
            Ok(pid)
        } else {
            Err(fdo::Error::AccessDenied(
                "Only this request's prompt may describe or decide it".into(),
            ))
        }
    }
}

fn capture_spec(subject: &str) -> PromptSpec {
    PromptSpec {
        subject: subject.into(),
        permission: SCREEN_CAPTURE.into(),
        title: "Screen capture".into(),
        body: format!(
            "{} wants to capture the contents of the screen.",
            subject.rsplit('/').next().unwrap_or(subject)
        ),
        deny_label: "Deny".into(),
        grant_label: "Allow once".into(),
        offer_always: true,
        choices: Vec::new(),
    }
}

struct Permissions {
    daemon: Arc<Daemon>,
}

#[interface(
    interface = "org.coconut.Permissions1",
    proxy(
        async_name = "PermissionsProxy",
        gen_blocking = true,
        assume_defaults = false,
        default_service = "org.coconut.Permissions",
        default_path = "/org/coconut/Permissions1",
        visibility = "pub"
    )
)]
impl Permissions {
    async fn ask(
        &self,
        subject: String,
        permission: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<String> {
        identity::require_owner("org.blair.Compositor", &header, connection).await?;
        if permission != SCREEN_CAPTURE
            || !(subject.starts_with('/') || subject.starts_with("flatpak:"))
        {
            return Err(fdo::Error::InvalidArgs(
                "Unsupported permission or subject".into(),
            ));
        }
        if let Some(verdict) = self
            .daemon
            .store
            .lock()
            .unwrap()
            .lookup(&subject, &permission)
        {
            return Ok(verdict.name().into());
        }
        let (_, rx) = self
            .daemon
            .start(capture_spec(&subject), Some((subject, permission)));
        Ok(rx
            .await
            .unwrap_or_else(|_| Answer::denied())
            .verdict
            .name()
            .into())
    }

    async fn begin_request(
        &self,
        subject: String,
        permission: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<String> {
        identity::require_owner("org.blair.Compositor", &header, connection).await?;
        if permission != SCREEN_CAPTURE
            || !(subject.starts_with('/') || subject.starts_with("flatpak:"))
            || subject.len() > 4096
        {
            return Err(fdo::Error::InvalidArgs(
                "Unsupported capture request".into(),
            ));
        }
        let owner = header.sender().unwrap().to_string();
        let id = format!(
            "request-{}",
            self.daemon.seq.fetch_add(1, Ordering::Relaxed)
        );
        let mut native = self.daemon.native.lock().unwrap();
        if native.len() >= 256 {
            return Err(fdo::Error::LimitsExceeded(
                "Too many capture permission requests".into(),
            ));
        }
        let verdict = self
            .daemon
            .store
            .lock()
            .unwrap()
            .lookup(&subject, &permission);
        let (prompt, rx, fresh) = if let Some(verdict) = verdict {
            let (tx, rx) = oneshot::channel();
            let _ = tx.send(Answer {
                verdict,
                choices: Vec::new(),
            });
            (None, rx, false)
        } else {
            let (prompt, rx, fresh) = self.daemon.register_for(
                capture_spec(&subject),
                Some((subject, permission)),
                Some(id.clone()),
            );
            (Some(prompt), rx, fresh)
        };
        native.insert(
            id.clone(),
            NativeRequest {
                owner,
                prompt: prompt.clone(),
                receiver: Some(rx),
            },
        );
        drop(native);
        if fresh {
            let prompt = prompt.unwrap();
            if let Err(error) = self.daemon.spawn_prompt(prompt.clone()) {
                failure(error);
                self.daemon.deliver(&prompt, Answer::denied());
            }
        }
        Ok(id)
    }

    async fn wait_request(
        &self,
        request_id: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<String> {
        let owner = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing caller".into()))?
            .to_string();
        let receiver = {
            let mut native = self.daemon.native.lock().unwrap();
            let request = native
                .get_mut(&request_id)
                .filter(|request| request.owner == owner)
                .ok_or_else(|| fdo::Error::AccessDenied("Unknown capture request".into()))?;
            request
                .receiver
                .take()
                .ok_or_else(|| fdo::Error::InvalidArgs("Request already has a waiter".into()))?
        };
        let answer = receiver.await.unwrap_or_else(|_| Answer::denied());
        self.daemon.native.lock().unwrap().remove(&request_id);
        Ok(answer.verdict.name().into())
    }

    async fn cancel_request(
        &self,
        request_id: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<bool> {
        let owner = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing caller".into()))?;
        Ok(self.daemon.cancel_native(owner.as_str(), &request_id))
    }

    async fn decide(
        &self,
        request_id: String,
        verdict: String,
        remember: bool,
        choices: Vec<(String, String)>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<()> {
        let pid = self
            .daemon
            .prompt_pid(&request_id, &header, connection)
            .await?;
        let verdict = Verdict::parse(&verdict)
            .ok_or_else(|| fdo::Error::InvalidArgs("Invalid verdict".into()))?;
        let pending = {
            let mut pending = self.daemon.pending.lock().unwrap();
            let entry = pending
                .get(&request_id)
                .filter(|entry| entry.prompt_pid == Some(pid))
                .ok_or_else(|| fdo::Error::AccessDenied("Request expired".into()))?;
            if remember && !entry.spec.offer_always {
                return Err(fdo::Error::InvalidArgs(
                    "This decision cannot be remembered".into(),
                ));
            }
            if verdict.allowed() && !valid_choices(&entry.spec.choices, &choices) {
                return Err(fdo::Error::InvalidArgs("Invalid choices".into()));
            }
            if remember {
                self.daemon
                    .store
                    .lock()
                    .unwrap()
                    .set(&entry.spec.subject, &entry.spec.permission, verdict)
                    .map_err(failure)?;
            }
            pending.remove(&request_id).unwrap()
        };
        for (_, waiter) in pending.waiters {
            let _ = waiter.send(Answer {
                verdict,
                choices: choices.clone(),
            });
        }
        if let Some(stop) = pending.stop {
            let _ = stop.send(());
        }
        if remember {
            Daemon::emit_changed(connection).await;
        }
        Ok(())
    }

    async fn describe_request(
        &self,
        request_id: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<RequestDescription> {
        self.daemon
            .prompt_pid(&request_id, &header, connection)
            .await?;
        let pending = self.daemon.pending.lock().unwrap();
        let spec = &pending
            .get(&request_id)
            .ok_or_else(|| failure("Request expired"))?
            .spec;
        Ok((
            spec.subject.clone(),
            spec.title.clone(),
            spec.body.clone(),
            spec.deny_label.clone(),
            spec.grant_label.clone(),
            spec.offer_always,
            spec.choices.clone(),
        ))
    }

    async fn list(&self) -> Vec<(String, String, String, u64)> {
        self.daemon
            .store
            .lock()
            .unwrap()
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.subject.clone(),
                    entry.permission.clone(),
                    entry.verdict.name().into(),
                    entry.granted_at,
                )
            })
            .collect()
    }

    async fn revoke(
        &self,
        subject: String,
        permission: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<bool> {
        identity::require_executable(&self.daemon.settings, &header, connection).await?;
        let removed = self
            .daemon
            .store
            .lock()
            .unwrap()
            .revoke(&subject, &permission)
            .map_err(failure)?;
        if removed {
            Daemon::emit_changed(connection).await;
        }
        Ok(removed)
    }

    async fn set_decision(
        &self,
        subject: String,
        permission: String,
        verdict: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<bool> {
        identity::require_executable(&self.daemon.settings, &header, connection).await?;
        let verdict = Verdict::parse(&verdict)
            .ok_or_else(|| fdo::Error::InvalidArgs("Invalid verdict".into()))?;
        self.daemon
            .store
            .lock()
            .unwrap()
            .set(&subject, &permission, verdict)
            .map_err(failure)?;
        Daemon::emit_changed(connection).await;
        Ok(true)
    }

    async fn forget(
        &self,
        subject: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<u32> {
        identity::require_executable(&self.daemon.settings, &header, connection).await?;
        let removed = self
            .daemon
            .store
            .lock()
            .unwrap()
            .forget(&subject)
            .map_err(failure)? as u32;
        if removed > 0 {
            Daemon::emit_changed(connection).await;
        }
        Ok(removed)
    }

    async fn manage(
        &self,
        operation: String,
        subject: String,
        permission: String,
        verdict: String,
        #[zbus(connection)] connection: &Connection,
    ) -> fdo::Result<bool> {
        let parsed = Verdict::parse(&verdict);
        let description = match operation.as_str() {
            "set" if parsed.is_some() => {
                format!("Set {permission} to {verdict} for this application?")
            }
            "revoke" => format!("Remove the saved {permission} decision for this application?"),
            "forget" => "Remove all saved decisions for this application?".into(),
            _ => return Err(fdo::Error::InvalidArgs("Invalid permission change".into())),
        };
        let spec = PromptSpec {
            subject: subject.clone(),
            permission: String::new(),
            title: "Change application permissions".into(),
            body: description,
            deny_label: "Cancel".into(),
            grant_label: "Confirm change".into(),
            offer_always: false,
            choices: Vec::new(),
        };
        let (_, rx) = self.daemon.start(spec, None);
        if !rx
            .await
            .unwrap_or_else(|_| Answer::denied())
            .verdict
            .allowed()
        {
            return Ok(false);
        }
        {
            let mut store = self.daemon.store.lock().unwrap();
            match operation.as_str() {
                "set" => store
                    .set(&subject, &permission, parsed.unwrap())
                    .map_err(failure)?,
                "revoke" => {
                    store.revoke(&subject, &permission).map_err(failure)?;
                }
                "forget" => {
                    store.forget(&subject).map_err(failure)?;
                }
                _ => unreachable!(),
            }
        }
        Daemon::emit_changed(connection).await;
        Ok(true)
    }

    #[zbus(signal)]
    pub async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

fn valid_choices(spec: &Choices, selected: &[(String, String)]) -> bool {
    spec.len() == selected.len()
        && spec.iter().all(|(id, _, options, _)| {
            selected.iter().filter(|(key, _)| key == id).count() == 1
                && selected.iter().any(|(key, value)| {
                    key == id
                        && if options.is_empty() {
                            value == "true" || value == "false"
                        } else {
                            options.iter().any(|(option, _)| option == value)
                        }
                })
        })
}

struct Access {
    daemon: Arc<Daemon>,
}

#[interface(interface = "org.freedesktop.impl.portal.Access")]
impl Access {
    #[allow(clippy::too_many_arguments)]
    async fn access_dialog(
        &self,
        handle: ObjectPath<'_>,
        app_id: String,
        _parent_window: String,
        title: String,
        subtitle: String,
        body: String,
        options: HashMap<String, Value<'_>>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<(u32, HashMap<String, OwnedValue>)> {
        identity::require_owner("org.freedesktop.portal.Desktop", &header, connection).await?;
        let choices = match options.get("choices") {
            Some(value) => Choices::try_from(value.try_clone().map_err(failure)?)
                .map_err(|error| fdo::Error::InvalidArgs(error.to_string()))?,
            None => Vec::new(),
        };
        let mut ids = std::collections::HashSet::new();
        for (id, _, options, selected) in &choices {
            let mut option_ids = std::collections::HashSet::new();
            if id.is_empty()
                || !ids.insert(id)
                || options
                    .iter()
                    .any(|(id, _)| id.is_empty() || !option_ids.insert(id))
                || (!selected.is_empty()
                    && if options.is_empty() {
                        selected != "true" && selected != "false"
                    } else {
                        !options.iter().any(|(id, _)| id == selected)
                    })
            {
                return Err(fdo::Error::InvalidArgs("Invalid Access choices".into()));
            }
        }
        let spec = PromptSpec {
            subject: app_id,
            permission: String::new(),
            title,
            body: [subtitle, body]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            deny_label: string_option(&options, "deny_label").unwrap_or_else(|| "Deny".into()),
            grant_label: string_option(&options, "grant_label").unwrap_or_else(|| "Allow".into()),
            offer_always: false,
            choices,
        };
        let (id, rx, _) = self.daemon.register(spec, None);
        let request = Request {
            daemon: self.daemon.clone(),
            id: id.clone(),
            owner: header.sender().unwrap().to_string(),
        };
        let handle = handle.into_owned();
        match server.at(handle.clone(), request).await {
            Ok(true) => {}
            result => {
                self.daemon.deliver(&id, Answer::denied());
                return Err(match result {
                    Err(error) => failure(error),
                    _ => fdo::Error::InvalidArgs("Request handle is already in use".into()),
                });
            }
        }
        if let Err(error) = self.daemon.spawn_prompt(id.clone()) {
            failure(error);
            self.daemon.deliver(&id, Answer::denied());
        }
        let answer = rx.await.unwrap_or_else(|_| Answer::denied());
        if let Err(error) = server.remove::<Request, _>(handle).await {
            failure(error);
        }
        let mut results = HashMap::new();
        if answer.verdict.allowed() {
            results.insert(
                "choices".into(),
                OwnedValue::try_from(Value::from(answer.choices)).map_err(failure)?,
            );
        }
        Ok((if answer.verdict.allowed() { 0 } else { 1 }, results))
    }
}

struct Request {
    daemon: Arc<Daemon>,
    id: String,
    owner: String,
}

#[interface(interface = "org.freedesktop.impl.portal.Request")]
impl Request {
    async fn close(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        if !header
            .sender()
            .is_some_and(|sender| sender.as_str() == self.owner)
        {
            return Err(fdo::Error::AccessDenied(
                "Only the request owner may cancel it".into(),
            ));
        }
        self.daemon.deliver(&self.id, Answer::denied());
        Ok(())
    }
}

fn string_option<'a>(options: &'a HashMap<String, Value<'a>>, key: &str) -> Option<String> {
    options
        .get(key)?
        .downcast_ref::<&'a str>()
        .ok()
        .map(str::to_owned)
}

async fn connect(daemon: Arc<Daemon>, connection: Connection) -> Result<Connection, String> {
    let server = connection.object_server();
    server
        .at(
            PATH,
            Permissions {
                daemon: daemon.clone(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    server
        .at(
            ACCESS_PATH,
            Access {
                daemon: daemon.clone(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")
        .map_err(|error| error.to_string())?
        .interface("org.freedesktop.DBus")
        .map_err(|error| error.to_string())?
        .member("NameOwnerChanged")
        .map_err(|error| error.to_string())?
        .build();
    let mut owners = zbus::MessageStream::for_match_rule(rule, &connection, Some(64))
        .await
        .map_err(|error| error.to_string())?;
    tokio::spawn(async move {
        while let Some(message) = owners.next().await {
            match message
                .and_then(|message| message.body().deserialize::<(String, String, String)>())
            {
                Ok((name, _, new_owner)) if name.starts_with(':') && new_owner.is_empty() => {
                    daemon.cancel_owner(&name)
                }
                Err(error) => {
                    failure(error);
                    break;
                }
                _ => {}
            }
        }
        let owners: Vec<_> = daemon
            .native
            .lock()
            .unwrap()
            .values()
            .map(|request| request.owner.clone())
            .collect();
        for owner in owners {
            daemon.cancel_owner(&owner);
        }
    });
    connection
        .request_name(SERVICE)
        .await
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

pub async fn serve() -> Result<(), String> {
    let store = Store::load(store_path())
        .map_err(|error| format!("Cannot load permission store: {error}"))?;
    let connection = Connection::session()
        .await
        .map_err(|error| error.to_string())?;
    let _connection = connect(
        Arc::new(Daemon::new(
            store,
            std::env::current_exe().map_err(|error| error.to_string())?,
        )),
        connection,
    )
    .await?;
    std::future::pending::<()>().await;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
