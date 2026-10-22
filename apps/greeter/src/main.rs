use std::{
    cell::RefCell,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use alphin::client::{socket_from_environment, Client};
use anyhow::{bail, Context, Result};
use creamui_reactive::Signal;
use creamui_render::{
    platform::WindowRole, AppBuilder, AppHandle, RenderBackend, WindowHandle, WindowOptions,
};
use creamui_widgets::{SelectController, TextController};
use serde_json::Value;

mod ui;

#[derive(Clone, Debug, PartialEq, Eq)]
struct User {
    username: String,
    display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Session {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Prompt {
    message: String,
    echo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AuthenticationStep {
    Prompt(Prompt),
    Authenticated,
    SessionQueued,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Live,
    Preview,
    PreviewMultiple,
    Help,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DemoLogin {
    username: String,
    password: String,
}

struct Options {
    mode: Mode,
    demo: Option<DemoLogin>,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = Mode::Live;
        let mut username = None;
        let mut password = None;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--preview" => mode = Mode::Preview,
                "--preview-multiple" => mode = Mode::PreviewMultiple,
                "--demouser" => {
                    username = Some(args.next().context("--demouser requires a username")?)
                }
                "--demopassord" | "--demopassword" => {
                    password = Some(args.next().context("--demopassord requires a password")?)
                }
                "--help" | "-h" => {
                    return Ok(Self {
                        mode: Mode::Help,
                        demo: None,
                    })
                }
                _ => bail!("unknown argument: {arg}. Use --help for usage."),
            }
        }
        let demo = match (username, password) {
            (None, None) => None,
            (Some(username), Some(password))
                if !username.trim().is_empty() && !password.is_empty() =>
            {
                Some(DemoLogin { username, password })
            }
            (Some(_), Some(_)) => bail!("demo credentials cannot be empty"),
            _ => bail!("--demouser and --demopassord must be supplied together"),
        };
        Ok(Self { mode, demo })
    }
}

#[derive(Clone)]
enum Backend {
    Live(Client),
    Preview { multiple: bool },
}

impl Backend {
    fn is_preview(&self) -> bool {
        matches!(self, Self::Preview { .. })
    }

    fn catalogue(&self) -> Result<(Vec<User>, Vec<Session>)> {
        match self {
            Self::Live(client) => catalogue(client),
            Self::Preview { multiple } => {
                let mut users = vec![
                    User {
                        username: "sammy".into(),
                        display_name: "Sammy".into(),
                    },
                    User {
                        username: "sam".into(),
                        display_name: "Sam Rivera".into(),
                    },
                    User {
                        username: "jules".into(),
                        display_name: "Jules Chen".into(),
                    },
                ];
                let mut sessions = vec![
                    Session {
                        id: "coconut".into(),
                        name: "Coconut".into(),
                    },
                    Session {
                        id: "plasma".into(),
                        name: "Plasma (Wayland)".into(),
                    },
                    Session {
                        id: "gnome".into(),
                        name: "GNOME".into(),
                    },
                ];
                if !multiple {
                    users.truncate(1);
                    sessions.truncate(1);
                }
                Ok((users, sessions))
            }
        }
    }

    fn authenticate(
        &self,
        username: &str,
        session: &str,
        answer: Option<String>,
    ) -> Result<AuthenticationStep> {
        match self {
            Self::Live(client) => authenticate_step(client, username, session, answer),
            Self::Preview { .. } => match answer.as_deref() {
                None => Ok(AuthenticationStep::Prompt(Prompt {
                    message: "Password".into(),
                    echo: false,
                })),
                Some("coconut") => Ok(AuthenticationStep::SessionQueued),
                Some(_) => bail!("Incorrect password. Try again."),
            },
        }
    }

    fn prepare(&self, username: &str, session: &str) -> Result<AuthenticationStep> {
        match self {
            Self::Live(client) => {
                client.begin_authentication(username)?;
                wait_for_authentication(client, session, false)
            }
            Self::Preview { .. } => self.authenticate(username, session, None),
        }
    }

    fn start_session(&self, session: &str) -> Result<AuthenticationStep> {
        match self {
            Self::Live(client) => start_session(client, session),
            Self::Preview { .. } => Ok(AuthenticationStep::SessionQueued),
        }
    }

    fn cancel(&self) -> Result<()> {
        if let Self::Live(client) = self {
            let response = client.cancel()?;
            if response["status"] != "cancelled" {
                bail!("{}", error_code(&response));
            }
        }
        Ok(())
    }

    fn power(&self, action: &str) -> Result<()> {
        if let Self::Live(client) = self {
            let response = client.power_action(action)?;
            if response["status"] != "power_requested" {
                bail!("{}", error_code(&response));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoticeKind {
    Info,
    Error,
    Success,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Notice {
    text: String,
    kind: NoticeKind,
}

#[derive(Clone)]
struct Greeter {
    backend: Backend,
    demo: Option<DemoLogin>,
    users: Signal<Vec<User>>,
    sessions: Signal<Vec<Session>>,
    selected_user: Signal<Option<String>>,
    session_menu: SelectController,
    prompt: Signal<Option<Prompt>>,
    authenticated: Signal<bool>,
    response: TextController,
    notice: Signal<Notice>,
    busy: Signal<bool>,
    users_offset: Signal<f32>,
    assets: Rc<ui::Assets>,
    window: Rc<RefCell<Option<WindowHandle>>>,
}

impl Greeter {
    fn new(backend: Backend, demo: Option<DemoLogin>) -> Self {
        Self {
            backend,
            demo,
            users: Signal::new(Vec::new()),
            sessions: Signal::new(Vec::new()),
            selected_user: Signal::new(None),
            session_menu: SelectController::new(0),
            prompt: Signal::new(None),
            authenticated: Signal::new(false),
            response: TextController::new(""),
            notice: Signal::new(Notice {
                text: "Loading accounts and sessions…".into(),
                kind: NoticeKind::Info,
            }),
            busy: Signal::new(true),
            users_offset: Signal::new(0.0),
            assets: Rc::new(ui::Assets::load()),
            window: Rc::new(RefCell::new(None)),
        }
    }

    fn notify(&self, kind: NoticeKind, text: impl Into<String>) {
        self.notice.set(Notice {
            text: text.into(),
            kind,
        });
    }

    fn load(&self, app: &AppHandle) {
        let backend = self.backend.clone();
        let state = self.clone();
        let app_done = app.clone();
        app.spawn_background(
            move || backend.catalogue(),
            move |result| {
                state.busy.set(false);
                match result {
                    Ok((users, sessions)) => {
                        state
                            .selected_user
                            .set(users.first().map(|user| user.username.clone()));
                        state.session_menu.select(0);
                        state.users.set(users);
                        state.sessions.set(sessions);
                        state.notify(NoticeKind::Info, "");
                        state.prepare(&app_done, false);
                    }
                    Err(error) => state.notify(
                        NoticeKind::Error,
                        format!("Unable to load the login screen: {error}"),
                    ),
                }
            },
        );
    }

    fn selected_session(&self) -> Option<String> {
        self.sessions
            .peek()
            .get(self.session_menu.peek_selected())
            .map(|session| session.id.clone())
    }

    fn prepare(&self, app: &AppHandle, preserve_notice: bool) {
        let (Some(username), Some(session)) = (self.selected_user.peek(), self.selected_session())
        else {
            return;
        };
        self.busy.set(true);
        self.authenticated.set(false);
        self.prompt.set(None);
        let backend = self.backend.clone();
        let state = self.clone();
        app.spawn_background(
            move || backend.prepare(&username, &session),
            move |result| {
                state.busy.set(false);
                match result {
                    Ok(AuthenticationStep::Prompt(prompt)) => state.prompt.set(Some(prompt)),
                    Ok(AuthenticationStep::Authenticated) => state.authenticated.set(true),
                    Ok(AuthenticationStep::SessionQueued) => {
                        unreachable!("preparing authentication cannot launch a session")
                    }
                    Err(error) => {
                        state.notify(NoticeKind::Error, format!("Unable to sign in: {error}"));
                        return;
                    }
                }
                if !preserve_notice {
                    state.notify(NoticeKind::Info, "");
                }
                state.focus_password();
            },
        );
    }

    fn focus_password(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.focus_text_input();
        }
    }

    fn select_user(&self, app: &AppHandle, username: String) {
        if self.busy.peek() || self.selected_user.peek().as_ref() == Some(&username) {
            return;
        }
        self.response.set_value("");
        if self.prompt.peek().is_none() && !self.authenticated.peek() {
            self.selected_user.set(Some(username));
            self.prepare(app, false);
            return;
        }
        self.busy.set(true);
        let backend = self.backend.clone();
        let state = self.clone();
        let app_done = app.clone();
        app.spawn_background(
            move || backend.cancel(),
            move |result| {
                state.busy.set(false);
                match result {
                    Ok(()) => {
                        state.prompt.set(None);
                        state.selected_user.set(Some(username));
                        state.prepare(&app_done, false);
                    }
                    Err(error) => state.notify(
                        NoticeKind::Error,
                        format!("Unable to switch accounts: {error}"),
                    ),
                }
            },
        );
    }

    fn submit(&self, app: &AppHandle) {
        if self.busy.peek() {
            return;
        }
        let (Some(username), Some(session)) = (self.selected_user.peek(), self.selected_session())
        else {
            self.notify(
                NoticeKind::Error,
                "Choose an account and a desktop session.",
            );
            return;
        };
        let pending = self.prompt.peek().is_some();
        if !pending && !self.authenticated.peek() {
            self.prepare(app, false);
            return;
        }
        let answer = self.response.peek();
        if pending && answer.is_empty() {
            self.notify(
                NoticeKind::Error,
                "Enter the requested authentication response.",
            );
            return;
        }
        self.response.set_value("");
        self.busy.set(true);
        self.notify(
            NoticeKind::Info,
            if pending {
                "Signing in…"
            } else {
                "Starting session…"
            },
        );
        let backend = self.backend.clone();
        let state = self.clone();
        let app_done = app.clone();
        let authenticated = self.authenticated.peek();
        app.spawn_background(
            move || {
                if backend.is_preview() {
                    thread::sleep(Duration::from_millis(450));
                }
                if authenticated {
                    backend.start_session(&session)
                } else {
                    backend.authenticate(&username, &session, Some(answer))
                }
            },
            move |result| {
                state.busy.set(false);
                match result {
                    Ok(AuthenticationStep::Prompt(prompt)) => {
                        state.notify(NoticeKind::Info, "");
                        state.prompt.set(Some(prompt));
                        state.focus_password();
                    }
                    Ok(AuthenticationStep::SessionQueued) if state.backend.is_preview() => {
                        state.notify(NoticeKind::Success, "Preview: signed in.");
                    }
                    Ok(AuthenticationStep::SessionQueued) => std::process::exit(0),
                    Ok(AuthenticationStep::Authenticated) => {
                        unreachable!("submitting authentication launches the session")
                    }
                    Err(error) => {
                        state.notify(NoticeKind::Error, format!("Sign-in failed: {error}"));
                        state.prepare(&app_done, true);
                    }
                }
            },
        );
    }

    fn power(&self, app: &AppHandle, action: &'static str, label: &'static str) {
        if self.busy.peek() {
            return;
        }
        self.busy.set(true);
        self.notify(
            NoticeKind::Info,
            format!("Requesting {}…", label.to_lowercase()),
        );
        let backend = self.backend.clone();
        let state = self.clone();
        let app_done = app.clone();
        app.spawn_background(
            move || backend.power(action),
            move |result| {
                state.busy.set(false);
                match result {
                    Ok(()) if state.backend.is_preview() => state.notify(
                        NoticeKind::Success,
                        format!("Preview: {} simulated.", label.to_lowercase()),
                    ),
                    Ok(()) => app_done.exit(),
                    Err(error) => {
                        state.notify(NoticeKind::Error, format!("Power request failed: {error}"))
                    }
                }
            },
        );
    }
}

fn main() {
    if let Err(error) = start() {
        eprintln!("coconut-greeter: {error:#}");
        std::process::exit(1);
    }
}

fn start() -> Result<()> {
    let options = Options::parse(std::env::args().skip(1))?;
    let backend = match options.mode {
        Mode::Preview => Backend::Preview { multiple: false },
        Mode::PreviewMultiple => Backend::Preview { multiple: true },
        Mode::Live => Backend::Live(socket_from_environment()?),
        Mode::Help => {
            println!("Coconut Greeter\n\nUsage: coconut-greeter [--preview | --preview-multiple] [--demouser USER --demopassord PASSWORD]\n\n  --preview           Preview with one sample account and session.\n  --preview-multiple  Preview with multiple accounts and sessions.\n                      No Alphin or Blair required. Password: coconut.\n                      Other passwords show an error; power actions are simulated.\n  --demouser USER     Display a live-session login hint for this username.\n  --demopassord PASS  Display this password in the login hint.\n                      --demopassword is also accepted. Both demo values are required.\n  -h, --help          Show this help.\n\nWithout a preview flag, ALPHIN_SOCKET must point to the Alphin daemon.\nPreview requires a running Wayland desktop (any compositor).");
            return Ok(());
        }
    };
    run(backend, options.demo);
    Ok(())
}

fn run(backend: Backend, demo: Option<DemoLogin>) {
    ui::load_fonts();
    let preview = backend.is_preview();
    let state = Greeter::new(backend, demo);

    AppBuilder::new()
        .on_started(move |app| {
            state.load(&app);
            let ui_app = app.clone();
            let ui_window = state.window.clone();
            app.append_window(
                WindowOptions {
                    title: if preview {
                        "Coconut Greeter · Preview"
                    } else {
                        "Sign in"
                    }
                    .into(),
                    width: 1080,
                    height: 760,
                    decorations: preview,
                    resizable: true,
                    transparent: false,
                    role: WindowRole::Normal,
                    backend: RenderBackend::Gpu,
                    theme: ui::theme(),
                    ..Default::default()
                },
                ui::BACKGROUND,
                move |handle| {
                    if !preview {
                        handle.maximize();
                    }
                    *ui_window.borrow_mut() = Some(handle);
                },
                move |size| ui::view(&state, &ui_app, size),
            );
        })
        .run();
}

fn catalogue(client: &Client) -> Result<(Vec<User>, Vec<Session>)> {
    let users = parse_users(&client.users()?)?;
    let sessions = parse_sessions(&client.sessions()?)?;
    if users.is_empty() {
        bail!("no local accounts are available")
    }
    if sessions.is_empty() {
        bail!("no Wayland sessions are available")
    }
    Ok((users, sessions))
}

fn parse_users(response: &Value) -> Result<Vec<User>> {
    response["users"]
        .as_array()
        .context("Alphin returned an invalid account list")?
        .iter()
        .map(|user| {
            Ok(User {
                username: user["username"]
                    .as_str()
                    .context("account is missing a username")?
                    .into(),
                display_name: user["display_name"].as_str().unwrap_or_default().into(),
            })
        })
        .collect()
}

fn parse_sessions(response: &Value) -> Result<Vec<Session>> {
    response["sessions"]
        .as_array()
        .context("Alphin returned an invalid session list")?
        .iter()
        .map(|session| {
            Ok(Session {
                id: session["id"]
                    .as_str()
                    .context("session is missing an id")?
                    .into(),
                name: session["name"]
                    .as_str()
                    .context("session is missing a name")?
                    .into(),
            })
        })
        .collect()
}

fn authenticate_step(
    client: &Client,
    username: &str,
    session: &str,
    answer: Option<String>,
) -> Result<AuthenticationStep> {
    match answer {
        Some(answer) => client.respond(answer)?,
        None => client.begin_authentication(username)?,
    };
    wait_for_authentication(client, session, true)
}

fn start_session(client: &Client, session: &str) -> Result<AuthenticationStep> {
    let response = client.start_session(session)?;
    if response["status"] == "session_queued" {
        Ok(AuthenticationStep::SessionQueued)
    } else {
        bail!("{}", error_code(&response))
    }
}

fn wait_for_authentication(
    client: &Client,
    session: &str,
    launch: bool,
) -> Result<AuthenticationStep> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match client.state()?.get("phase").and_then(Value::as_str) {
            Some("authenticated") => {
                return if launch {
                    start_session(client, session)
                } else {
                    Ok(AuthenticationStep::Authenticated)
                };
            }
            Some("authenticating") => {
                let status = client.status()?;
                if status["kind"] == "prompt" {
                    return Ok(AuthenticationStep::Prompt(Prompt {
                        message: status["message"]
                            .as_str()
                            .unwrap_or("Authentication response")
                            .into(),
                        echo: status["echo"].as_bool().unwrap_or(false),
                    }));
                }
            }
            Some("greeter") => bail!("authentication was rejected"),
            Some(phase) => bail!("unexpected login state: {phase}"),
            None => bail!("Alphin returned an invalid login state"),
        }
        if Instant::now() >= deadline {
            bail!("authentication timed out")
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn error_code(response: &Value) -> &str {
    response["code"]
        .as_str()
        .or_else(|| response["status"].as_str())
        .unwrap_or("unknown error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn with_live_backend(replies: Vec<(Value, Value)>, test: impl FnOnce(Backend)) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("coconut-greeter-{}-{nonce}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let socket = directory.join("alphin.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            for (expected, reply) in replies {
                let deadline = Instant::now() + Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "missing request: {expected}");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept mock connection: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut request)
                    .unwrap();
                let request: Value = serde_json::from_str(&request).unwrap();
                for (key, value) in expected.as_object().unwrap() {
                    assert_eq!(&request[key], value, "request field {key}");
                }
                writeln!(stream, "{reply}").unwrap();
            }
        });
        test(Backend::Live(Client::new(&socket)));
        server.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn parses_launch_modes_without_a_socket() {
        assert_eq!(Options::parse(Vec::new()).unwrap().mode, Mode::Live);
        assert_eq!(
            Options::parse(["--preview".into()]).unwrap().mode,
            Mode::Preview
        );
        assert_eq!(
            Options::parse(["--preview-multiple".into()]).unwrap().mode,
            Mode::PreviewMultiple
        );
        assert_eq!(Options::parse(["--help".into()]).unwrap().mode, Mode::Help);
        assert_eq!(Options::parse(["-h".into()]).unwrap().mode, Mode::Help);
        assert!(Options::parse(["--unknown".into()]).is_err());
    }

    #[test]
    fn demo_credentials_are_optional_and_require_both_values() {
        assert!(Options::parse(Vec::new()).unwrap().demo.is_none());
        let options = Options::parse(
            ["--preview", "--demouser", "hello", "--demopassord", "world"].map(String::from),
        )
        .unwrap();
        assert_eq!(options.mode, Mode::Preview);
        assert_eq!(
            options.demo,
            Some(DemoLogin {
                username: "hello".into(),
                password: "world".into()
            })
        );
        assert!(Options::parse(["--demouser", "hello"].map(String::from)).is_err());
        assert!(Options::parse(["--demopassord", "world"].map(String::from)).is_err());
        assert!(Options::parse(["--demouser"].map(String::from)).is_err());
        assert!(
            Options::parse(["--demouser", "hello", "--demopassord"].map(String::from)).is_err()
        );
        assert!(
            Options::parse(["--demouser", "", "--demopassord", "world"].map(String::from)).is_err()
        );
        assert!(
            Options::parse(["--demouser", "hello", "--demopassord", ""].map(String::from)).is_err()
        );
    }

    #[test]
    fn default_preview_has_one_account_and_session() {
        let backend = Backend::Preview { multiple: false };
        let (users, sessions) = backend.catalogue().unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(sessions.len(), 1);
        assert!(matches!(
            backend
                .prepare(&users[0].username, &sessions[0].id)
                .unwrap(),
            AuthenticationStep::Prompt(Prompt { echo: false, .. })
        ));
    }

    #[test]
    fn preparing_passwordless_authentication_does_not_launch_a_session() {
        with_live_backend(
            vec![
                (
                    json!({"method": "begin_authentication", "username": "coconut"}),
                    json!({"status": "authenticating"}),
                ),
                (
                    json!({"method": "get_state"}),
                    json!({"phase": "authenticated"}),
                ),
                (
                    json!({"method": "start_session", "session": "coconut"}),
                    json!({"status": "session_queued"}),
                ),
            ],
            |backend| {
                assert_eq!(
                    backend.prepare("coconut", "coconut").unwrap(),
                    AuthenticationStep::Authenticated
                );
                assert_eq!(
                    backend.start_session("coconut").unwrap(),
                    AuthenticationStep::SessionQueued
                );
            },
        );
    }

    #[test]
    fn preparing_authentication_fetches_the_password_prompt() {
        with_live_backend(
            vec![
                (
                    json!({"method": "begin_authentication", "username": "coconut"}),
                    json!({"status": "authenticating"}),
                ),
                (
                    json!({"method": "get_state"}),
                    json!({"phase": "authenticating"}),
                ),
                (
                    json!({"method": "status"}),
                    json!({"kind": "prompt", "message": "Password:", "echo": false}),
                ),
            ],
            |backend| {
                assert_eq!(
                    backend.prepare("coconut", "coconut").unwrap(),
                    AuthenticationStep::Prompt(Prompt {
                        message: "Password:".into(),
                        echo: false
                    })
                );
            },
        );
    }

    #[test]
    fn preview_supports_sign_in_errors_and_safe_power_actions() {
        let backend = Backend::Preview { multiple: true };
        let (users, sessions) = backend.catalogue().unwrap();
        assert!(users.len() > 1 && sessions.len() > 1);
        for user in &users {
            for session in &sessions {
                assert_eq!(
                    backend
                        .authenticate(&user.username, &session.id, None)
                        .unwrap(),
                    AuthenticationStep::Prompt(Prompt {
                        message: "Password".into(),
                        echo: false
                    })
                );
                assert!(backend
                    .authenticate(&user.username, &session.id, Some("incorrect".into()))
                    .is_err());
                assert_eq!(
                    backend
                        .authenticate(&user.username, &session.id, Some("coconut".into()))
                        .unwrap(),
                    AuthenticationStep::SessionQueued
                );
            }
        }
        backend.cancel().unwrap();
        backend.power("reboot").unwrap();
        backend.power("poweroff").unwrap();
        assert_eq!(backend.catalogue().unwrap(), (users, sessions));
    }

    #[test]
    fn live_authentication_preserves_echo_prompts_and_session_handoff() {
        with_live_backend(
            vec![
                (
                    json!({"method": "begin_authentication", "username": "alex"}),
                    json!({"status": "authenticating"}),
                ),
                (
                    json!({"method": "get_state"}),
                    json!({"phase": "authenticating"}),
                ),
                (
                    json!({"method": "status"}),
                    json!({"kind": "prompt", "message": "Verification code", "echo": true}),
                ),
                (
                    json!({"method": "respond", "response": "123456"}),
                    json!({"status": "authenticating"}),
                ),
                (
                    json!({"method": "get_state"}),
                    json!({"phase": "authenticated"}),
                ),
                (
                    json!({"method": "start_session", "session": "plasma"}),
                    json!({"status": "session_queued"}),
                ),
            ],
            |backend| {
                assert_eq!(
                    backend.authenticate("alex", "plasma", None).unwrap(),
                    AuthenticationStep::Prompt(Prompt {
                        message: "Verification code".into(),
                        echo: true
                    })
                );
                assert_eq!(
                    backend
                        .authenticate("alex", "plasma", Some("123456".into()))
                        .unwrap(),
                    AuthenticationStep::SessionQueued
                );
            },
        );
    }

    #[test]
    fn live_backend_cancels_and_reports_rejected_power_requests() {
        with_live_backend(
            vec![
                (json!({"method": "cancel"}), json!({"status": "cancelled"})),
                (
                    json!({"method": "power_action", "action": "poweroff"}),
                    json!({"status": "error", "code": "power_denied"}),
                ),
                (
                    json!({"method": "power_action", "action": "reboot"}),
                    json!({"status": "power_requested"}),
                ),
            ],
            |backend| {
                backend.cancel().unwrap();
                assert!(backend
                    .power("poweroff")
                    .unwrap_err()
                    .to_string()
                    .contains("power_denied"));
                backend.power("reboot").unwrap();
            },
        );
    }

    #[test]
    fn parses_alphin_catalogues() {
        assert_eq!(
            parse_users(
                &json!({ "users": [{ "username": "coconut", "display_name": "Coconut" }] })
            )
            .unwrap(),
            vec![User {
                username: "coconut".into(),
                display_name: "Coconut".into()
            }]
        );
        assert_eq!(
            parse_sessions(&json!({ "sessions": [{ "id": "coconut", "name": "Coconut" }] }))
                .unwrap(),
            vec![Session {
                id: "coconut".into(),
                name: "Coconut".into()
            }]
        );
    }

    #[test]
    fn rejects_malformed_catalogues() {
        assert!(parse_users(&json!({ "users": [{ "display_name": "Coconut" }] })).is_err());
        assert!(parse_sessions(&json!({ "sessions": [{ "id": "coconut" }] })).is_err());
    }
}
