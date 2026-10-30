//! The consent dialog. Rendered on demand by the daemon for one pending
//! request; every button reports the verdict through `Decide` and exits.

use crate::daemon;
use creamui_render::{AppBuilder, RenderBackend, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::{ButtonVariant, Dialog};
use std::rc::Rc;

struct Description {
    subject: String,
    title: String,
    body: String,
    deny_label: String,
    grant_label: String,
    offer_always: bool,
}

fn proxy(
    connection: &zbus::blocking::Connection,
) -> Result<daemon::PermissionsProxyBlocking<'static>, String> {
    daemon::PermissionsProxyBlocking::new(connection)
        .map_err(|error| format!("permissions proxy failed: {error}"))
}

fn decide(proxy: &daemon::PermissionsProxyBlocking<'_>, id: &str, verdict: &str, remember: bool) {
    if let Err(error) = proxy.decide(id.to_owned(), verdict.to_owned(), remember) {
        eprintln!("permission-portal: Decide failed: {error}");
    }
}

pub fn prompt(id: &str) -> Result<(), String> {
    let connection = zbus::blocking::Connection::session()
        .map_err(|error| format!("session bus connect failed: {error}"))?;
    let proxy = proxy(&connection)?;
    let (subject, title, body, deny_label, grant_label, offer_always) = proxy
        .describe_request(id.to_owned())
        .map_err(|error| format!("DescribeRequest failed: {error}"))?;
    let description = Rc::new(Description {
        subject,
        title,
        body,
        deny_label,
        grant_label,
        offer_always,
    });
    if description.title.is_empty() && description.body.is_empty() {
        // The request vanished before the prompt could render (decided
        // elsewhere, cancelled by the caller, or daemon restarted).
        return Ok(());
    }
    let proxy = Rc::new(proxy);
    let request_id = id.to_owned();
    let options = WindowOptions {
        title: "Permission request".into(),
        width: 480,
        height: 240,
        decorations: false,
        resizable: false,
        theme: crate::appearance::load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let description = description.clone();
            let proxy = proxy.clone();
            let request_id = request_id.clone();
            let quit = app.clone();
            app.append_window(
                options,
                Color::rgba(24, 24, 28, 255),
                move |window| {
                    let _ = window;
                },
                move |_size| -> creamui_core::BoxedWidget {
                    let dismiss_proxy = proxy.clone();
                    let dismiss_id = request_id.clone();
                    let dismiss_quit = quit.clone();
                    let dialog = Dialog::new(
                        description.title.clone(),
                        format!("{}\n{}", description.body, description.subject),
                        move || {
                            decide(&dismiss_proxy, &dismiss_id, "deny", false);
                            dismiss_quit.exit();
                        },
                    )
                    .width(440.0);
                    let deny_proxy = proxy.clone();
                    let deny_id = request_id.clone();
                    let deny_quit = quit.clone();
                    let deny = description.deny_label.clone();
                    let dialog = dialog.action(deny, ButtonVariant::Secondary, move || {
                        decide(&deny_proxy, &deny_id, "deny", false);
                        deny_quit.exit();
                    });
                    let once_proxy = proxy.clone();
                    let once_id = request_id.clone();
                    let once_quit = quit.clone();
                    let grant = description.grant_label.clone();
                    let dialog = dialog.action(grant, ButtonVariant::Primary, move || {
                        decide(&once_proxy, &once_id, "allow", false);
                        once_quit.exit();
                    });
                    let dialog = if description.offer_always {
                        let always_proxy = proxy.clone();
                        let always_id = request_id.clone();
                        let always_quit = quit.clone();
                        dialog.action("Always allow", ButtonVariant::Tertiary, move || {
                            decide(&always_proxy, &always_id, "allow", true);
                            always_quit.exit();
                        })
                    } else {
                        dialog
                    };
                    Box::new(dialog)
                },
            );
        })
        .run();
    Ok(())
}
