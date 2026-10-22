use std::{
    cell::RefCell,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use alphin::client::{socket_from_environment, Client};
use anyhow::{bail, Context, Result};
use creamui_core::BoxedWidget;
use creamui_reactive::Signal;
use creamui_render::{
    platform::WindowRole, AppBuilder, AppHandle, RenderBackend, WindowHandle, WindowOptions,
};
use creamui_theme::Color;
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    Button, ButtonSize, ButtonState, Text, TextController, TextInput,
};
use serde_json::Value;

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
    SessionQueued,
}

fn main() {
    let client = match socket_from_environment() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("coconut-greeter: {error:#}");
            std::process::exit(1);
        }
    };
    run(client);
}

fn run(client: Client) {
    let users = Signal::new(Vec::<User>::new());
    let sessions = Signal::new(Vec::<Session>::new());
    let selected_user = Signal::new(None::<String>);
    let selected_session = Signal::new(None::<String>);
    let prompt = Signal::new(None::<Prompt>);
    let response = TextController::new("");
    let message = Signal::new(String::from("Loading accounts and sessions…"));
    let busy = Signal::new(true);
    let window = Rc::new(RefCell::new(None::<WindowHandle>));

    AppBuilder::new()
        .on_started(move |app| {
            load_catalogue(
                app.clone(),
                client.clone(),
                users.clone(),
                sessions.clone(),
                selected_user.clone(),
                selected_session.clone(),
                message.clone(),
                busy.clone(),
            );
            let ui_app = app.clone();
            let ui_window = window.clone();
            app.append_window(
                WindowOptions {
                    title: "Sign in".into(),
                    width: 560,
                    height: 640,
                    decorations: false,
                    resizable: true,
                    transparent: true,
                    role: WindowRole::Normal,
                    backend: RenderBackend::Gpu,
                    ..Default::default()
                },
                Color::rgba(0, 0, 0, 1),
                move |handle| *ui_window.borrow_mut() = Some(handle),
                move |_| {
                    view(
                        ui_app.clone(),
                        client.clone(),
                        users.clone(),
                        sessions.clone(),
                        selected_user.clone(),
                        selected_session.clone(),
                        prompt.clone(),
                        response.clone(),
                        message.clone(),
                        busy.clone(),
                        window.clone(),
                    )
                },
            );
        })
        .run();
}

#[allow(clippy::too_many_arguments)]
fn load_catalogue(
    app: AppHandle,
    client: Client,
    users: Signal<Vec<User>>,
    sessions: Signal<Vec<Session>>,
    selected_user: Signal<Option<String>>,
    selected_session: Signal<Option<String>>,
    message: Signal<String>,
    busy: Signal<bool>,
) {
    app.spawn_background(
        move || catalogue(&client),
        move |result| {
            busy.set(false);
            match result {
                Ok((available_users, available_sessions)) => {
                    selected_user.set(available_users.first().map(|user| user.username.clone()));
                    selected_session
                        .set(available_sessions.first().map(|session| session.id.clone()));
                    users.set(available_users);
                    sessions.set(available_sessions);
                    message.set("Select an account and a session.".into());
                }
                Err(error) => message.set(format!("Unable to load the login screen: {error}")),
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn view(
    app: AppHandle,
    client: Client,
    users: Signal<Vec<User>>,
    sessions: Signal<Vec<Session>>,
    selected_user: Signal<Option<String>>,
    selected_session: Signal<Option<String>>,
    prompt: Signal<Option<Prompt>>,
    response: TextController,
    message: Signal<String>,
    busy: Signal<bool>,
    window: Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let is_busy = busy.get();
    let selected_account = selected_user.get();
    let selected_desktop = selected_session.get();
    let current_prompt = prompt.get();
    let submit = {
        let app = app.clone();
        let client = client.clone();
        let selected_user = selected_user.clone();
        let selected_session = selected_session.clone();
        let prompt = prompt.clone();
        let response = response.clone();
        let message = message.clone();
        let busy = busy.clone();
        let window = window.clone();
        move || {
            if busy.peek() {
                return;
            }
            let Some(username) = selected_user.peek() else {
                message.set("Choose an account before signing in.".into());
                return;
            };
            let Some(session) = selected_session.peek() else {
                message.set("Choose a session before signing in.".into());
                return;
            };
            let prompt_pending = prompt.peek().is_some();
            let answer = response.peek();
            if prompt_pending && answer.is_empty() {
                message.set("Enter the requested authentication response.".into());
                return;
            }
            response.set_value("");
            busy.set(true);
            message.set(
                if prompt_pending {
                    "Verifying…"
                } else {
                    "Authenticating…"
                }
                .into(),
            );
            let worker_client = client.clone();
            let worker_answer = prompt_pending.then_some(answer);
            let done_prompt = prompt.clone();
            let done_message = message.clone();
            let done_busy = busy.clone();
            let done_window = window.clone();
            app.spawn_background(
                move || authenticate_step(&worker_client, &username, &session, worker_answer),
                move |result| {
                    done_busy.set(false);
                    match result {
                        Ok(AuthenticationStep::Prompt(next_prompt)) => {
                            done_message.set(next_prompt.message.clone());
                            done_prompt.set(Some(next_prompt));
                            if let Some(window) = done_window.borrow().as_ref() {
                                window.focus_text_input();
                            }
                        }
                        Ok(AuthenticationStep::SessionQueued) => std::process::exit(0),
                        Err(error) => {
                            done_prompt.set(None);
                            done_message.set(format!("Sign-in failed: {error}"));
                        }
                    }
                },
            );
        }
    };

    let mut content = Flex::column()
        .gap(12.0)
        .align(Align::Center)
        .justify(Justify::Center)
        .child(Box::new(Text::new("Welcome").font_size(28.0)))
        .child(Box::new(Text::secondary(message.get())))
        .child(Box::new(Text::new("Account").font_size(16.0)));
    for user in users.get() {
        let username = user.username.clone();
        let label = if user.display_name.is_empty() {
            user.username
        } else {
            user.display_name
        };
        let selected_user = selected_user.clone();
        let prompt = prompt.clone();
        let message = message.clone();
        content = content.child(Box::new(
            Button::secondary(ButtonSize::Sm, label, move || {
                selected_user.set(Some(username.clone()));
                prompt.set(None);
                message.set("Select a session, then sign in.".into());
            })
            .disabled(is_busy),
        ));
    }
    content = content.child(Box::new(Text::new("Session").font_size(16.0)));
    for session in sessions.get() {
        let id = session.id.clone();
        let selected_session = selected_session.clone();
        content = content.child(Box::new(
            Button::secondary(ButtonSize::Sm, session.name, move || {
                selected_session.set(Some(id.clone()));
            })
            .disabled(is_busy),
        ));
    }
    if let Some(current_prompt) = current_prompt.clone() {
        let mut input = TextInput::controlled(&response)
            .placeholder(current_prompt.message)
            .on_submit(submit.clone());
        if !current_prompt.echo {
            input = input.password();
        }
        content = content.child(Box::new(input));
    }
    content = content
        .child(Box::new(
            Button::state(
                ButtonSize::Md,
                if current_prompt.is_some() {
                    "Continue"
                } else {
                    "Sign in"
                },
                if is_busy {
                    ButtonState::Loading
                } else {
                    ButtonState::Normal
                },
                submit,
            )
            .disabled(selected_account.is_none() || selected_desktop.is_none()),
        ))
        .child(Box::new(power_button(
            app.clone(),
            client.clone(),
            "Restart",
            "reboot",
            message.clone(),
            busy.clone(),
        )))
        .child(Box::new(power_button(
            app,
            client,
            "Power off",
            "poweroff",
            message,
            busy,
        )));
    Box::new(content)
}

fn power_button(
    app: AppHandle,
    client: Client,
    label: &'static str,
    action: &'static str,
    message: Signal<String>,
    busy: Signal<bool>,
) -> Button {
    let disabled = busy.get();
    Button::secondary(ButtonSize::Sm, label, move || {
        if busy.peek() {
            return;
        }
        busy.set(true);
        message.set(format!("Requesting {}…", label.to_lowercase()));
        let done_app = app.clone();
        let done_message = message.clone();
        let done_busy = busy.clone();
        let worker_client = client.clone();
        app.spawn_background(
            move || worker_client.power_action(action),
            move |result| {
                done_busy.set(false);
                match result {
                    Ok(response) if response["status"] == "power_requested" => done_app.exit(),
                    Ok(response) => {
                        done_message.set(format!("Power request failed: {}", error_code(&response)))
                    }
                    Err(error) => done_message.set(format!("Power request failed: {error}")),
                }
            },
        );
    })
    .disabled(disabled)
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match client.state()?.get("phase").and_then(Value::as_str) {
            Some("authenticated") => {
                let response = client.start_session(session)?;
                if response["status"] == "session_queued" {
                    return Ok(AuthenticationStep::SessionQueued);
                }
                bail!("{}", error_code(&response));
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
