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
) -> BoxedWidget {
    let submit = {
        let username = username.clone();
        let password = password.clone();
        let message = message.clone();
        move || {
            let result = if password.peek().is_empty() {
                client.begin_authentication(&username.peek())
            } else {
                let response = password.peek();
                password.set_value("");
                client.respond(response)
            };
            message.set(match result {
                Ok(value) => value
                    .get("status")
                    .and_then(|status| status.as_str())
                    .or_else(|| value.get("message").and_then(|prompt| prompt.as_str()))
                    .unwrap_or("Authentication requires another response.")
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
