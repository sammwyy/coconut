use super::*;
use std::{
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
};

struct Bus {
    process: Child,
    address: String,
}

impl Bus {
    fn new() -> Self {
        let mut process = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1", "--nopidfile"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is required for portal tests");
        let mut address = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            process,
            address: address.trim().into(),
        }
    }
    async fn connection(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        self.process.kill().unwrap();
        self.process.wait().unwrap();
    }
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn daemon(root: &std::path::Path, bus: &Bus, mode: &str) -> Arc<Daemon> {
    let script = root.join("prompt");
    let exe = std::env::current_exe().unwrap();
    std::fs::write(&script, format!(
        "#!/bin/sh\nexport DBUS_SESSION_BUS_ADDRESS={}\nexport COCONUT_TEST_PROMPT={}\nexport COCONUT_TEST_REQUEST=\"$2\"\nexec {} --exact daemon::tests::prompt_worker --nocapture >/dev/null\n",
        quote(&bus.address), quote(mode), quote(exe.to_str().unwrap())
    )).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut daemon = Daemon::new(Store::load(root.join("permissions.json")).unwrap(), script);
    daemon.settings = exe;
    Arc::new(daemon)
}

async fn wait_prompt(daemon: &Daemon) -> (String, u32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(pair) = daemon
                .pending
                .lock()
                .unwrap()
                .iter()
                .find_map(|(id, request)| request.prompt_pid.map(|pid| (id.clone(), pid)))
            {
                return pair;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("prompt did not start")
}

#[test]
fn prompt_worker() {
    let Ok(mode) = std::env::var("COCONUT_TEST_PROMPT") else {
        return;
    };
    let connection = zbus::blocking::Connection::session().unwrap();
    let proxy = PermissionsProxyBlocking::new(&connection).unwrap();
    let id = std::env::var("COCONUT_TEST_REQUEST").unwrap();
    let (_, _, _, _, _, _, choices) = proxy.describe_request(id.clone()).unwrap();
    proxy.list().unwrap();
    if mode == "hold" {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    let choices = choices
        .into_iter()
        .map(|(id, _, options, _)| {
            (
                id,
                options
                    .last()
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(|| "true".into()),
            )
        })
        .collect();
    if mode == "fail-save" {
        assert!(proxy
            .decide(id.clone(), "allow".into(), true, Vec::new())
            .is_err());
        proxy.decide(id, "deny".into(), false, Vec::new()).unwrap();
    } else {
        proxy
            .decide(id, "allow".into(), mode == "remember", choices)
            .unwrap();
    }
}

#[tokio::test]
async fn ask_remains_responsive_and_remembered_decisions_are_saved_before_reply() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "remember");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    connection
        .request_name("org.blair.Compositor")
        .await
        .unwrap();
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    let verdict = tokio::time::timeout(
        Duration::from_secs(5),
        proxy.ask("/usr/bin/grim".into(), SCREEN_CAPTURE.into()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(verdict, "allow");
    assert!(daemon.pending.lock().unwrap().is_empty());
    assert_eq!(
        Store::load(root.path().join("permissions.json"))
            .unwrap()
            .lookup("/usr/bin/grim", SCREEN_CAPTURE),
        Some(Verdict::Allow)
    );
}

#[tokio::test]
async fn untrusted_clients_cannot_impersonate_the_compositor_settings_or_prompt() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let mut daemon = Arc::try_unwrap(daemon(root.path(), &bus, "hold"))
        .ok()
        .unwrap();
    daemon.settings = root.path().join("foreign-settings");
    let daemon = Arc::new(daemon);
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    assert!(proxy
        .ask("/usr/bin/coconut-screenshots".into(), SCREEN_CAPTURE.into())
        .await
        .is_err());
    assert!(proxy
        .set_decision("app".into(), SCREEN_CAPTURE.into(), "allow".into())
        .await
        .is_err());
    assert!(proxy
        .revoke("app".into(), SCREEN_CAPTURE.into())
        .await
        .is_err());
    assert!(proxy.forget("app".into()).await.is_err());
    let (id, rx) = daemon.start(capture_spec("/usr/bin/grim"), None);
    let (_, pid) = wait_prompt(&daemon).await;
    assert!(proxy.describe_request(id.clone()).await.is_err());
    assert!(proxy
        .decide(id.clone(), "allow".into(), true, Vec::new())
        .await
        .is_err());
    assert!(tokio::time::timeout(Duration::from_secs(2), proxy.list())
        .await
        .unwrap()
        .unwrap()
        .is_empty());
    daemon.deliver(&id, Answer::denied());
    assert_eq!(rx.await.unwrap().verdict, Verdict::Deny);
    tokio::time::timeout(Duration::from_secs(2), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("cancelled prompt was not reaped");
}

#[tokio::test]
async fn save_failures_do_not_grant_or_leave_a_hung_request() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "fail-save");
    std::fs::create_dir(root.path().join("permissions.json")).unwrap();
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    connection
        .request_name("org.blair.Compositor")
        .await
        .unwrap();
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    let verdict = tokio::time::timeout(
        Duration::from_secs(5),
        proxy.ask("/usr/bin/grim".into(), SCREEN_CAPTURE.into()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(verdict, "deny");
    assert!(proxy.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn access_returns_a_method_reply_with_choices_at_the_standard_path() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "allow");
    let _server = connect(daemon, bus.connection().await).await.unwrap();
    let frontend = bus.connection().await;
    frontend
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let choices: Choices = vec![
        (
            "background".into(),
            "Run in background".into(),
            Vec::new(),
            "false".into(),
        ),
        (
            "level".into(),
            "Access level".into(),
            vec![("low".into(), "Low".into()), ("high".into(), "High".into())],
            "low".into(),
        ),
    ];
    let options: HashMap<&str, Value<'_>> =
        [("choices", Value::from(choices))].into_iter().collect();
    let handle =
        ObjectPath::try_from("/org/freedesktop/portal/desktop/request/test/access").unwrap();
    let reply = tokio::time::timeout(
        Duration::from_secs(5),
        frontend.call_method(
            Some(SERVICE),
            ACCESS_PATH,
            Some("org.freedesktop.impl.portal.Access"),
            "AccessDialog",
            &(
                handle.clone(),
                "org.test.App",
                "",
                "Access request",
                "Subtitle",
                "Body",
                options,
            ),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let (code, results): (u32, HashMap<String, OwnedValue>) = reply.body().deserialize().unwrap();
    assert_eq!(code, 0);
    let choices: Vec<(String, String)> =
        results["choices"].try_clone().unwrap().try_into().unwrap();
    assert_eq!(
        choices,
        [
            ("background".into(), "true".into()),
            ("level".into(), "high".into())
        ]
    );
    assert!(frontend
        .call_method(
            Some(SERVICE),
            handle,
            Some("org.freedesktop.impl.portal.Request"),
            "Close",
            &()
        )
        .await
        .is_err());
}

#[tokio::test]
async fn access_cancellation_checks_the_owner_and_closes_the_prompt() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let frontend = bus.connection().await;
    frontend
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let handle =
        ObjectPath::try_from("/org/freedesktop/portal/desktop/request/test/cancel").unwrap();
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let args = (
        handle.clone(),
        "org.test.App",
        "",
        "Access request",
        "",
        "Body",
        options,
    );
    let call = frontend.call_method(
        Some(SERVICE),
        ACCESS_PATH,
        Some("org.freedesktop.impl.portal.Access"),
        "AccessDialog",
        &args,
    );
    let cancel = async {
        let (id, _) = wait_prompt(&daemon).await;
        let attacker = bus.connection().await;
        assert!(attacker
            .call_method(
                Some(SERVICE),
                handle.clone(),
                Some("org.freedesktop.impl.portal.Request"),
                "Close",
                &()
            )
            .await
            .is_err());
        assert!(daemon.pending.lock().unwrap().contains_key(&id));
        frontend
            .call_method(
                Some(SERVICE),
                handle.clone(),
                Some("org.freedesktop.impl.portal.Request"),
                "Close",
                &(),
            )
            .await
            .unwrap();
    };
    let (reply, _) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(call, cancel) })
            .await
            .unwrap();
    let (code, _): (u32, HashMap<String, OwnedValue>) =
        reply.unwrap().body().deserialize().unwrap();
    assert_eq!(code, 1);
    assert!(daemon.pending.lock().unwrap().is_empty());
}

#[tokio::test]
async fn simultaneous_requests_share_one_prompt_and_all_waiters_complete() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    let pair = Some(("/usr/bin/grim".into(), SCREEN_CAPTURE.into()));
    let (id, first) = daemon.start(capture_spec("/usr/bin/grim"), pair.clone());
    let (other, second) = daemon.start(capture_spec("/usr/bin/grim"), pair);
    assert_eq!(id, other);
    assert_eq!(daemon.pending.lock().unwrap().len(), 1);
    let (_, pid) = wait_prompt(&daemon).await;
    daemon.deliver(&id, Answer::denied());
    assert_eq!(first.await.unwrap().verdict, Verdict::Deny);
    assert_eq!(second.await.unwrap().verdict, Verdict::Deny);
    tokio::time::timeout(Duration::from_secs(2), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("coalesced prompt was not reaped");
}

#[tokio::test]
async fn timed_out_and_failed_prompts_deny_and_remove_pending_requests() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let mut daemon = Arc::try_unwrap(daemon(root.path(), &bus, "hold"))
        .ok()
        .unwrap();
    daemon.prompt_timeout = Duration::from_millis(250);
    let daemon = Arc::new(daemon);
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let (id, rx) = daemon.start(capture_spec("/usr/bin/grim"), None);
    let (_, pid) = wait_prompt(&daemon).await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), rx)
            .await
            .unwrap()
            .unwrap()
            .verdict,
        Verdict::Deny
    );
    assert!(!daemon.pending.lock().unwrap().contains_key(&id));
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    std::fs::remove_file(root.path().join("prompt")).unwrap();
    let (_, rx) = daemon.start(capture_spec("/usr/bin/grim"), None);
    assert_eq!(rx.await.unwrap().verdict, Verdict::Deny);
    assert!(daemon.pending.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cli_management_requires_a_prompt_and_propagates_storage_failures() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "allow");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    assert!(proxy
        .manage(
            "set".into(),
            "app".into(),
            SCREEN_CAPTURE.into(),
            "allow".into()
        )
        .await
        .unwrap());
    assert_eq!(daemon.seq.load(Ordering::Relaxed), 1);
    assert_eq!(proxy.list().await.unwrap()[0].2, "allow");
    assert!(proxy
        .manage(
            "revoke".into(),
            "app".into(),
            SCREEN_CAPTURE.into(),
            "".into()
        )
        .await
        .unwrap());
    assert!(proxy.list().await.unwrap().is_empty());
    std::fs::remove_file(root.path().join("permissions.json")).unwrap();
    std::fs::create_dir(root.path().join("permissions.json")).unwrap();
    assert!(proxy
        .manage(
            "set".into(),
            "app".into(),
            SCREEN_CAPTURE.into(),
            "allow".into()
        )
        .await
        .is_err());
    assert!(proxy.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn settings_changes_succeed_and_report_failed_writes_to_the_caller() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    assert!(proxy
        .set_decision("app".into(), SCREEN_CAPTURE.into(), "deny".into())
        .await
        .unwrap());
    assert_eq!(proxy.list().await.unwrap()[0].2, "deny");
    std::fs::remove_file(root.path().join("permissions.json")).unwrap();
    std::fs::create_dir(root.path().join("permissions.json")).unwrap();
    assert!(proxy
        .set_decision("app".into(), SCREEN_CAPTURE.into(), "allow".into())
        .await
        .is_err());
    assert!(proxy
        .revoke("app".into(), SCREEN_CAPTURE.into())
        .await
        .is_err());
    assert!(proxy.forget("app".into()).await.is_err());
    assert_eq!(proxy.list().await.unwrap()[0].2, "deny");
}

#[tokio::test]
async fn native_cancellation_is_owned_and_preserves_other_waiters() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    connection
        .request_name("org.blair.Compositor")
        .await
        .unwrap();
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    let first = proxy
        .begin_request("/usr/bin/grim".into(), SCREEN_CAPTURE.into())
        .await
        .unwrap();
    let second = proxy
        .begin_request("/usr/bin/grim".into(), SCREEN_CAPTURE.into())
        .await
        .unwrap();
    let (prompt, pid) = wait_prompt(&daemon).await;
    assert_eq!(daemon.pending.lock().unwrap().len(), 1);
    assert_eq!(daemon.pending.lock().unwrap()[&prompt].waiters.len(), 2);
    let attacker_connection = bus.connection().await;
    let attacker = PermissionsProxy::new(&attacker_connection).await.unwrap();
    assert!(attacker
        .begin_request("/usr/bin/grim".into(), SCREEN_CAPTURE.into())
        .await
        .is_err());
    assert!(attacker.wait_request(first.clone()).await.is_err());
    assert!(!attacker.cancel_request(first.clone()).await.unwrap());
    let waiter = proxy.clone();
    let first_wait = first.clone();
    let wait = tokio::spawn(async move { waiter.wait_request(first_wait).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while daemon.native.lock().unwrap()[&first].receiver.is_some() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(proxy.cancel_request(first).await.unwrap());
    assert_eq!(wait.await.unwrap().unwrap(), "deny");
    assert_eq!(daemon.pending.lock().unwrap()[&prompt].waiters.len(), 1);
    assert!(proxy.cancel_request(second).await.unwrap());
    assert!(daemon.pending.lock().unwrap().is_empty());
    tokio::time::timeout(Duration::from_secs(2), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(daemon.native.lock().unwrap().is_empty());
}

#[tokio::test]
async fn native_stored_verdicts_return_without_opening_a_prompt() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    daemon
        .store
        .lock()
        .unwrap()
        .set("/usr/bin/grim", SCREEN_CAPTURE, Verdict::Allow)
        .unwrap();
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    connection
        .request_name("org.blair.Compositor")
        .await
        .unwrap();
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    let id = proxy
        .begin_request("/usr/bin/grim".into(), SCREEN_CAPTURE.into())
        .await
        .unwrap();
    assert_eq!(proxy.wait_request(id).await.unwrap(), "allow");
    assert!(daemon.pending.lock().unwrap().is_empty());
    assert!(daemon.native.lock().unwrap().is_empty());
}

#[tokio::test]
async fn native_requests_are_cancelled_when_the_compositor_disconnects() {
    let bus = Bus::new();
    let root = tempfile::tempdir().unwrap();
    let daemon = daemon(root.path(), &bus, "hold");
    let _server = connect(daemon.clone(), bus.connection().await)
        .await
        .unwrap();
    let connection = bus.connection().await;
    connection
        .request_name("org.blair.Compositor")
        .await
        .unwrap();
    let proxy = PermissionsProxy::new(&connection).await.unwrap();
    proxy
        .begin_request("/usr/bin/grim".into(), SCREEN_CAPTURE.into())
        .await
        .unwrap();
    wait_prompt(&daemon).await;
    connection.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !daemon.pending.lock().unwrap().is_empty()
            || !daemon.native.lock().unwrap().is_empty()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("compositor requests were retained after disconnect");
}
