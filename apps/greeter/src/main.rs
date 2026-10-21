use std::{thread, time::Duration};

use alphin::client::{socket_from_environment, Client};
use creamui_core::BoxedWidget;
use creamui_reactive::Signal;
use creamui_render::{platform::WindowRole, AppBuilder, RenderBackend, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::{
    layout::{Align, Flex, Justify},
    Button, Text, TextController, TextInput,
};

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
    let username = TextController::new("");
    let password = TextController::new("");
    let message = Signal::new(String::from("Enter your account name."));
    let awaiting_response = Signal::new(false);
    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                WindowOptions {
                    title: "Sign in".into(),
                    width: 480,
                    height: 360,
                    decorations: false,
                    resizable: false,
                    transparent: true,
                    role: WindowRole::Overlay,
                    backend: RenderBackend::Gpu,
                    ..Default::default()
                },
                Color::rgba(0, 0, 0, 1),
                |_| {},
                move |_| {
                    view(
                        client.clone(),
                        username.clone(),
                        password.clone(),
                        message.clone(),
                        awaiting_response.clone(),
                    )
                },
            );
        })
        .run();
}

fn view(
    client: Client,
    username: TextController,
    password: TextController,
    message: Signal<String>,
    awaiting_response: Signal<bool>,
) -> BoxedWidget {
    let submit = {
        let username = username.clone();
        let password = password.clone();
        let message = message.clone();
        let awaiting_response = awaiting_response.clone();
        move || {
            let result = if awaiting_response.get() {
                let response = password.peek();
                password.set_value("");
                client
                    .respond(response)
                    .and_then(|_| wait_for_authentication(&client))
            } else {
                awaiting_response.set(true);
                client
                    .begin_authentication(&username.peek())
                    .and_then(|_| wait_for_authentication(&client))
            };
            message.set(match result {
                Ok(value) => value
                    .get("status")
                    .and_then(|status| status.as_str())
                    .or_else(|| value.get("message").and_then(|prompt| prompt.as_str()))
                    .unwrap_or("Enter the requested authentication response.")
                    .to_owned(),
                Err(error) => format!("Authentication failed: {error}"),
            });
        }
    };
    Box::new(
        Flex::column()
            .gap(16.0)
            .align(Align::Center)
            .justify(Justify::Center)
            .child(Box::new(Text::new("Welcome").font_size(28.0)))
            .child(Box::new(Text::secondary(message.get())))
            .child(Box::new(
                TextInput::controlled(&username).placeholder("Username"),
            ))
            .child(Box::new(
                TextInput::controlled(&password)
                    .placeholder("Password")
                    .password()
                    .on_submit(submit.clone()),
            ))
            .child(Box::new(Button::new("Sign in", submit))),
    )
}

fn wait_for_authentication(client: &Client) -> anyhow::Result<serde_json::Value> {
    for _ in 0..20 {
        let state = client.state()?;
        match state.get("phase").and_then(|phase| phase.as_str()) {
            Some("authenticated") => {
                let sessions = client.sessions()?;
                let session = sessions
                    .get("sessions")
                    .and_then(|sessions| sessions.as_array())
                    .and_then(|sessions| sessions.first())
                    .and_then(|session| session.get("id"))
                    .and_then(|id| id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("no desktop sessions are available"))?;
                return client.start_session(session);
            }
            Some("authenticating") => {
                let status = client.status()?;
                if status.get("kind").and_then(|kind| kind.as_str()) == Some("prompt") {
                    return Ok(status);
                }
            }
            _ => return Ok(state),
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(serde_json::json!({ "status": "authentication_pending" }))
}
