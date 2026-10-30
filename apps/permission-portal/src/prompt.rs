use crate::daemon;
use creamui_core::{
    layout::{Dimension, Size as LayoutSize, Style},
    BoxedWidget, Size,
};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, AppHandle, RenderBackend, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, Dialog, DialogContent, DialogFooter,
    DialogHeader, DialogMessage, ModalActions, ScrollController, ScrollView, Select,
    SelectController,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

type Submit = Rc<dyn Fn(&'static str, bool)>;

struct Choice {
    id: String,
    label: String,
    options: Vec<(String, String)>,
    controller: SelectController,
}

struct PromptView {
    subject: String,
    title: String,
    body: String,
    deny_label: String,
    grant_label: String,
    offer_always: bool,
    choices: Vec<Choice>,
    busy: Signal<bool>,
    error: Signal<String>,
    scroll: ScrollController,
}

impl PromptView {
    fn build(&self, size: Size, submit: Submit) -> BoxedWidget {
        let dismiss = submit.clone();
        let mut content: Vec<BoxedWidget> = vec![Box::new(DialogMessage::new(format!(
            "{}\n{}",
            self.body, self.subject
        )))];
        for choice in &self.choices {
            content.push(Box::new(DialogMessage::new(choice.label.clone())));
            let options: Vec<_> = choice
                .options
                .iter()
                .map(|(_, label)| label.as_str())
                .collect();
            content.push(Box::new(Select::controlled(
                &options,
                choice.controller.clone(),
            )));
        }
        let error = self.error.get();
        if !error.is_empty() {
            content.push(Box::new(DialogMessage::new(error)));
        }
        let busy = self.busy.get();
        let mut actions = ModalActions::new();
        for (label, variant, verdict, remember) in [
            (
                self.deny_label.clone(),
                ButtonVariant::Secondary,
                "deny",
                false,
            ),
            (
                self.grant_label.clone(),
                ButtonVariant::Primary,
                "allow",
                false,
            ),
            (
                "Always allow".into(),
                ButtonVariant::Tertiary,
                "allow",
                true,
            ),
        ] {
            if remember && !self.offer_always {
                continue;
            }
            let submit = submit.clone();
            actions = actions.child(
                Button::styled(
                    variant,
                    ButtonSize::Md,
                    label,
                    ButtonState::Normal,
                    move || submit(verdict, remember),
                )
                .disabled(busy),
            );
        }
        Box::new(
            Dialog::custom(move || dismiss("deny", false))
                .width((size.width - 48.0).clamp(160.0, 520.0))
                .header(Box::new(DialogHeader::new(self.title.clone()).compact()))
                .content(Box::new(
                    ScrollView::controlled(
                        Style {
                            size: LayoutSize {
                                width: Dimension::Percent(1.0),
                                height: Dimension::Auto,
                            },
                            max_size: LayoutSize {
                                width: Dimension::Percent(1.0),
                                height: Dimension::Length((size.height - 180.0).max(40.0)),
                            },
                            min_size: LayoutSize {
                                width: Dimension::Length(0.0),
                                height: Dimension::Length(0.0),
                            },
                            flex_shrink: 1.0,
                            ..Default::default()
                        },
                        self.scroll.clone(),
                    )
                    .child(Box::new(
                        DialogContent::new().compact().with_children(content),
                    )),
                ))
                .footer(Box::new(
                    DialogFooter::new().compact().child(Box::new(actions)),
                )),
        )
    }
}

#[derive(Clone)]
struct Prompt {
    proxy: daemon::PermissionsProxyBlocking<'static>,
    id: String,
    view: Rc<PromptView>,
}

impl Prompt {
    fn submit(&self, app: AppHandle, verdict: &'static str, remember: bool) {
        if self.view.busy.get() {
            return;
        }
        creamui_reactive::batch(|| {
            self.view.busy.set(true);
            self.view.error.set(String::new());
        });
        let proxy = self.proxy.clone();
        let id = self.id.clone();
        let choices = self
            .view
            .choices
            .iter()
            .map(|choice| {
                (
                    choice.id.clone(),
                    choice.options[choice.controller.selected()].0.clone(),
                )
            })
            .collect();
        let done = self.clone();
        let quit = app.clone();
        app.spawn_background(
            move || proxy.decide(id, verdict.into(), remember, choices),
            move |result| {
                creamui_reactive::batch(|| {
                    done.view.busy.set(false);
                    match result {
                        Ok(()) => quit.exit(),
                        Err(error) => {
                            eprintln!("permission-portal: Decide failed: {error}");
                            done.view
                                .error
                                .set(format!("Could not apply the decision: {error}"));
                        }
                    }
                });
            },
        );
    }
}

pub fn prompt(id: &str) -> Result<(), String> {
    let connection = zbus::blocking::Connection::session().map_err(|error| error.to_string())?;
    let proxy =
        daemon::PermissionsProxyBlocking::new(&connection).map_err(|error| error.to_string())?;
    let (subject, title, body, deny_label, grant_label, offer_always, choices) = proxy
        .describe_request(id.into())
        .map_err(|error| error.to_string())?;
    let id = id.to_owned();
    let height = (320 + choices.len() as u32 * 80).min(720);
    let options = WindowOptions {
        title: "Permission request".into(),
        width: 560,
        height,
        decorations: false,
        resizable: true,
        theme: crate::appearance::load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let choices = choices
                .into_iter()
                .map(|(id, label, mut options, selected)| {
                    if options.is_empty() {
                        options =
                            vec![("false".into(), "No".into()), ("true".into(), "Yes".into())];
                    }
                    let index = options
                        .iter()
                        .position(|(id, _)| id == &selected)
                        .unwrap_or(0);
                    Choice {
                        id,
                        label,
                        options,
                        controller: SelectController::new(index),
                    }
                })
                .collect();
            let prompt = Prompt {
                proxy,
                id,
                view: Rc::new(PromptView {
                    subject,
                    title,
                    body,
                    deny_label,
                    grant_label,
                    offer_always,
                    choices,
                    busy: Signal::new(false),
                    error: Signal::new(String::new()),
                    scroll: ScrollController::default(),
                }),
            };
            let app_handle = app.clone();
            let ready = prompt.clone();
            let ready_app = app.clone();
            let window = Rc::new(RefCell::new(None));
            let ready_window = window.clone();
            app.append_window(
                options,
                Color::rgba(24, 24, 28, 255),
                move |window| {
                    *ready_window.borrow_mut() = Some(window.clone());
                    window.on_keyboard_input(move |event, _| {
                        if event.pressed && event.key == creamui_render::platform::Key::Escape {
                            ready.submit(ready_app.clone(), "deny", false);
                            true
                        } else {
                            false
                        }
                    });
                },
                move |size| {
                    let next = prompt.clone();
                    let app = app_handle.clone();
                    prompt.view.build(
                        size,
                        Rc::new(move |verdict, remember| {
                            next.submit(app.clone(), verdict, remember)
                        }),
                    )
                },
            );
            crate::appearance::watch_theme_reloads(
                app,
                Arc::new(Mutex::new(creamui_theme_watch::ThemeListener::start())),
                window,
            );
        })
        .run();
    Ok(())
}

#[cfg(test)]
mod tests;
