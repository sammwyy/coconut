//! Session service owning the permission store and the consent flow.
//!
//! `Ask` either answers from the store or creates a pending request, spawns
//! the prompt (`--prompt <id>`), and awaits the user's `Decide` call. The
//! same machinery backs the standard `org.freedesktop.impl.portal.Access`
//! interface, whose verdicts are reported through `Request.Response`
//! signals instead of a method return.

use crate::store::{Store, Verdict, SCREEN_CAPTURE};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::oneshot;
use zbus::{
    interface,
    message::Header,
    object_server::{ObjectServer, SignalEmitter},
    zvariant::{ObjectPath, Value},
    Connection,
};

pub const SERVICE: &str = "org.coconut.Permissions";
pub const PATH: &str = "/org/coconut/Permissions1";
const ACCESS_PATH: &str = "/org/freedesktop/impl/portal/Access";
const REQUEST_IFACE: &str = "org.freedesktop.impl.portal.Request";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(180);

/// What the prompt window shows. `subject` is the store key (executable
/// path or Flatpak app id).
struct PromptSpec {
    subject: String,
    permission: String,
    title: String,
    body: String,
    deny_label: String,
    grant_label: String,
    /// Whether the prompt offers a persistent "always allow" choice.
    offer_always: bool,
}

enum ResponseSink {
    Ask(Vec<oneshot::Sender<Verdict>>),
    Access {
        handle: ObjectPath<'static>,
        connection: Connection,
    },
}

struct Pending {
    spec: PromptSpec,
    sink: ResponseSink,
}

#[derive(Default)]
struct Daemon {
    store: Mutex<Store>,
    by_pair: Mutex<HashMap<(String, String), String>>,
    by_id: Mutex<HashMap<String, Pending>>,
    seq: AtomicU64,
}

type RequestDescription = (String, String, String, String, String, bool);

fn warn(message: impl std::fmt::Display) {
    eprintln!("permission-portal: {message}");
}

impl Daemon {
    fn next_id(&self) -> String {
        format!("coconut-{}", self.seq.fetch_add(1, Ordering::Relaxed))
    }

    fn register(
        &self,
        pair: Option<(String, String)>,
        spec: PromptSpec,
        sink: ResponseSink,
    ) -> String {
        let id = self.next_id();
        self.by_id
            .lock()
            .unwrap()
            .insert(id.clone(), Pending { spec, sink });
        if let Some(pair) = pair {
            self.by_pair.lock().unwrap().insert(pair, id.clone());
        }
        id
    }

    /// Registers a new waiter on an already-visible prompt for the same
    /// subject/permission pair.
    fn coalesce(&self, pair: &(String, String)) -> Option<oneshot::Receiver<Verdict>> {
        let id = self.by_pair.lock().unwrap().get(pair)?.clone();
        let mut pending = self.by_id.lock().unwrap();
        let ResponseSink::Ask(senders) = &mut pending.get_mut(&id)?.sink else {
            return None;
        };
        let (tx, rx) = oneshot::channel();
        senders.push(tx);
        Some(rx)
    }

    /// Removes a pending request; the caller decides how to deliver the
    /// verdict (method reply, portal `Response` signal, or silently when
    /// the requester already cancelled).
    fn take(&self, request_id: &str) -> Option<Pending> {
        let pending = self.by_id.lock().unwrap().remove(request_id)?;
        self.by_pair
            .lock()
            .unwrap()
            .retain(|_, id| id != request_id);
        Some(pending)
    }

    async fn emit_changed(connection: &Connection) {
        let Ok(emitter) = SignalEmitter::new(connection, PATH) else {
            return;
        };
        if let Err(error) = Permissions::changed(&emitter).await {
            warn(format!("could not emit Permissions1.Changed: {error}"));
        }
    }
}

/// Delivers the verdict to whoever is waiting on `request_id`: the blocked
/// `Ask` callers or the portal `Request` object. Returns the pending spec
/// so `Decide` can persist remembered verdicts.
async fn deliver(daemon: &Daemon, request_id: &str, verdict: Verdict) -> Option<PromptSpec> {
    let pending = daemon.take(request_id)?;
    match pending.sink {
        ResponseSink::Ask(senders) => {
            for sender in senders {
                let _ = sender.send(verdict);
            }
        }
        ResponseSink::Access { handle, connection } => {
            let code = if verdict.allowed() { 0u32 } else { 1u32 };
            let results: HashMap<String, Value<'_>> = HashMap::new();
            if let Err(error) = connection
                .emit_signal(
                    None::<String>,
                    handle.as_str(),
                    REQUEST_IFACE,
                    "Response",
                    &(code, results),
                )
                .await
            {
                warn(format!("could not emit Access response: {error}"));
            }
            let _ = connection
                .object_server()
                .remove::<Request, _>(handle.as_str())
                .await;
        }
    }
    Some(pending.spec)
}

/// Spawns the prompt binary and watches it: a prompt that dies without
/// calling `Decide` resolves the request as denied instead of leaving the
/// caller parked until timeout. Reaping the child also avoids zombies.
fn spawn_watched_prompt(daemon: Arc<Daemon>, id: String) {
    let spawn = std::env::current_exe().and_then(|exe| {
        tokio::process::Command::new(exe)
            .arg("--prompt")
            .arg(&id)
            .spawn()
    });
    match spawn {
        Ok(mut child) => {
            tokio::spawn(async move {
                let _ = child.wait().await;
                deliver(&daemon, &id, Verdict::Deny).await;
            });
        }
        Err(error) => {
            warn(format!("could not spawn prompt: {error}"));
            tokio::spawn(async move {
                deliver(&daemon, &id, Verdict::Deny).await;
            });
        }
    }
}

fn display_name(subject: &str) -> String {
    subject
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(subject)
        .to_owned()
}

fn capture_spec(subject: &str) -> PromptSpec {
    PromptSpec {
        subject: subject.into(),
        permission: SCREEN_CAPTURE.into(),
        title: "Screen capture".into(),
        body: format!(
            "{} wants to capture the contents of the screen.",
            display_name(subject)
        ),
        deny_label: "Deny".into(),
        grant_label: "Allow once".into(),
        offer_always: true,
    }
}

struct Permissions {
    daemon: Arc<Daemon>,
}

#[interface(
    interface = "org.coconut.Permissions1",
    spawn = false,
    proxy(
        async_name = "PermissionsProxy",
        gen_blocking = true,
        assume_defaults = false,
        default_service = "org.coconut.Permissions",
        default_path = "/org/coconut/Permissions1",
        visibility = "pub",
    )
)]
impl Permissions {
    /// Blocks until the decision is known: instantly for stored verdicts,
    /// after the prompt otherwise. Returns "allow" or "deny".
    async fn ask(&self, subject: String, permission: String) -> String {
        if let Some(verdict) = self
            .daemon
            .store
            .lock()
            .unwrap()
            .lookup(&subject, &permission)
        {
            return verdict.name().to_owned();
        }
        let pair = (subject.clone(), permission.clone());
        let rx = if let Some(rx) = self.daemon.coalesce(&pair) {
            rx
        } else {
            let (tx, rx) = oneshot::channel();
            let id = self.daemon.register(
                Some(pair),
                capture_spec(&subject),
                ResponseSink::Ask(vec![tx]),
            );
            spawn_watched_prompt(self.daemon.clone(), id);
            rx
        };
        match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
            Ok(Ok(verdict)) => verdict.name().to_owned(),
            _ => "deny".to_owned(),
        }
    }

    /// Called by the prompt app once the user chooses. `remember` stores
    /// the verdict persistently for future `Ask` calls.
    async fn decide(
        &self,
        request_id: String,
        verdict: String,
        remember: bool,
        #[zbus(connection)] connection: &Connection,
    ) {
        let Some(verdict) = Verdict::parse(&verdict) else {
            return;
        };
        let Some(spec) = deliver(&self.daemon, &request_id, verdict).await else {
            return;
        };
        if remember {
            self.daemon
                .store
                .lock()
                .unwrap()
                .set(&spec.subject, &spec.permission, verdict);
            Daemon::emit_changed(connection).await;
        }
    }

    /// Everything the prompt window needs to render a request.
    async fn describe_request(&self, request_id: String) -> RequestDescription {
        let pending = self.daemon.by_id.lock().unwrap();
        let Some(entry) = pending.get(&request_id) else {
            return Default::default();
        };
        let spec = &entry.spec;
        (
            spec.subject.clone(),
            spec.title.clone(),
            spec.body.clone(),
            spec.deny_label.clone(),
            spec.grant_label.clone(),
            spec.offer_always,
        )
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
                    entry.verdict.name().to_owned(),
                    entry.granted_at,
                )
            })
            .collect()
    }

    async fn revoke(
        &self,
        subject: String,
        permission: String,
        #[zbus(connection)] connection: &Connection,
    ) -> bool {
        let removed = self
            .daemon
            .store
            .lock()
            .unwrap()
            .revoke(&subject, &permission);
        if removed {
            Daemon::emit_changed(connection).await;
        }
        removed
    }

    async fn set_decision(
        &self,
        subject: String,
        permission: String,
        verdict: String,
        #[zbus(connection)] connection: &Connection,
    ) -> bool {
        let Some(verdict) = Verdict::parse(&verdict) else {
            return false;
        };
        self.daemon
            .store
            .lock()
            .unwrap()
            .set(&subject, &permission, verdict);
        Daemon::emit_changed(connection).await;
        true
    }

    async fn forget(&self, subject: String, #[zbus(connection)] connection: &Connection) -> u32 {
        let removed = self.daemon.store.lock().unwrap().forget(&subject) as u32;
        if removed > 0 {
            Daemon::emit_changed(connection).await;
        }
        removed
    }

    #[zbus(signal)]
    pub async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// The standard generic consent dialog. Apps talking to a frontend portal
/// implementation land here; Coconut answers with the same prompt used for
/// compositor gates, so the UX stays unified.
struct Access {
    daemon: Arc<Daemon>,
}

#[interface(interface = "org.freedesktop.impl.portal.Access", spawn = false)]
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
    ) {
        let subject = if app_id.is_empty() {
            caller_subject(&header, connection).await
        } else {
            format!("flatpak:{app_id}")
        };
        let spec = PromptSpec {
            subject,
            permission: String::new(),
            title: if subtitle.is_empty() { title } else { subtitle },
            body,
            deny_label: string_option(&options, "deny_label").unwrap_or_else(|| "Deny".into()),
            grant_label: string_option(&options, "grant_label").unwrap_or_else(|| "Allow".into()),
            offer_always: false,
        };
        let handle: ObjectPath<'static> = handle.into_owned();
        let id = self.daemon.register(
            None,
            spec,
            ResponseSink::Access {
                handle: handle.clone(),
                connection: connection.clone(),
            },
        );
        let request = Request {
            daemon: self.daemon.clone(),
            id: id.clone(),
        };
        if let Err(error) = server.at(handle, request).await {
            warn(format!("could not serve Access request: {error}"));
            let _ = self.daemon.take(&id);
            return;
        }
        spawn_watched_prompt(self.daemon.clone(), id);
    }
}

struct Request {
    daemon: Arc<Daemon>,
    id: String,
}

#[interface(interface = "org.freedesktop.impl.portal.Request", spawn = false)]
impl Request {
    /// The calling app cancelled before the user decided: clear the pending
    /// prompt without reporting a response.
    async fn close(&self) {
        let _ = self.daemon.take(&self.id);
    }
}

fn string_option<'a>(options: &'a HashMap<String, Value<'a>>, key: &str) -> Option<String> {
    options
        .get(key)?
        .downcast_ref::<&'a str>()
        .ok()
        .map(str::to_owned)
}

/// Resolves the D-Bus caller's unique name to a PID and then to an
/// executable path — the same identification the compositor performs
/// through `SO_PEERCRED`.
async fn caller_subject(header: &Header<'_>, connection: &Connection) -> String {
    let Some(sender) = header.sender() else {
        return "unknown".into();
    };
    let pid: u32 = match connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &sender.as_str(),
        )
        .await
        .and_then(|reply| reply.body().deserialize())
    {
        Ok(pid) => pid,
        Err(_) => return sender.to_string(),
    };
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| format!("pid:{pid}"))
}

pub async fn serve() -> Result<(), String> {
    let daemon = Arc::new(Daemon::default());
    let connection = Connection::session()
        .await
        .map_err(|error| format!("session bus connect failed: {error}"))?;
    let server = connection.object_server();
    server
        .at(
            PATH,
            Permissions {
                daemon: daemon.clone(),
            },
        )
        .await
        .map_err(|error| format!("could not serve {PATH}: {error}"))?;
    server
        .at(
            ACCESS_PATH,
            Access {
                daemon: daemon.clone(),
            },
        )
        .await
        .map_err(|error| format!("could not serve {ACCESS_PATH}: {error}"))?;
    connection
        .request_name(SERVICE)
        .await
        .map_err(|error| format!("could not own {SERVICE}: {error}"))?;
    std::future::pending::<()>().await;
    Ok(())
}
