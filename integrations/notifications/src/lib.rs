use coconut_api::notifications::{
    Notification, NotificationEvent, NotificationImage, NotificationListener,
    NotificationsIntegration,
};
use dbus::{
    arg::{PropMap, RefArg},
    blocking::{stdintf::org_freedesktop_dbus::RequestNameReply, Connection},
    channel::{MatchingReceiver, Sender},
    message::MatchRule,
    Message,
};
use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::CString,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = SERVICE;

enum Command {
    Close(u32, u32),
    Invoke(u32, String),
    Stop,
}

#[derive(Default)]
pub struct Notifications {
    server: RefCell<Option<Server>>,
}

struct Server {
    commands: mpsc::Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl NotificationsIntegration for Notifications {
    fn start(&self) -> Option<NotificationListener> {
        if self.server.borrow().is_some() {
            return None;
        }
        let (events_tx, events_rx) = mpsc::channel();
        let (commands_tx, commands_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            let result = serve(events_tx, commands_rx, ready_tx);
            // Another desktop notification daemon already owning this name is
            // expected.  The shell simply runs without its own notification
            // service in that case.
            match result {
                Err(error) if !is_name_already_owned(&error) => {
                    eprintln!("notifications: {error}");
                }
                _ => {}
            }
        });
        if !ready_rx.recv().unwrap_or(false) {
            let _ = thread.join();
            return None;
        }
        *self.server.borrow_mut() = Some(Server {
            commands: commands_tx,
            thread: Some(thread),
        });
        Some(NotificationListener(Arc::new(Mutex::new(events_rx))))
    }

    fn invoke(&self, id: u32, action: &str) {
        if let Some(server) = self.server.borrow().as_ref() {
            let _ = server.commands.send(Command::Invoke(id, action.to_owned()));
        }
    }

    fn close(&self, id: u32, reason: u32) {
        if let Some(server) = self.server.borrow().as_ref() {
            let _ = server.commands.send(Command::Close(id, reason));
        }
    }
}

fn is_name_already_owned(error: &str) -> bool {
    error == "another notification service already owns the session bus name"
}

fn serve(
    events: mpsc::Sender<NotificationEvent>,
    commands: mpsc::Receiver<Command>,
    ready: mpsc::SyncSender<bool>,
) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let owner = connection
        .request_name(SERVICE, false, false, true)
        .map_err(|error| error.to_string())?;
    if owner != RequestNameReply::PrimaryOwner {
        let _ = ready.send(false);
        return Err("another notification service already owns the session bus name".into());
    }
    let active = Arc::new(Mutex::new(HashSet::new()));
    let handler_active = active.clone();
    let mut next_id = 0u32;
    let mut rule = MatchRule::new_method_call();
    rule.path = Some(PATH.into());
    connection.start_receive(
        rule,
        Box::new(move |message, connection| {
            let reply = match message.interface().as_deref() {
                Some("org.freedesktop.DBus.Introspectable") => {
                    Ok(message.method_return().append1(INTROSPECTION))
                }
                Some(INTERFACE) => match message.member().as_deref() {
                    Some("GetCapabilities") => {
                        Ok(message
                            .method_return()
                            .append1(vec!["body", "icon-static", "actions"]))
                    }
                    Some("GetServerInformation") => Ok(message
                        .method_return()
                        .append2("Coconut", "Coconut")
                        .append2(env!("CARGO_PKG_VERSION"), "1.2")),
                    Some("Notify") => {
                        type Args = (
                            String,
                            u32,
                            String,
                            String,
                            String,
                            Vec<String>,
                            PropMap,
                            i32,
                        );
                        match message.read_all::<Args>() {
                            Ok((
                                app_name,
                                replaces_id,
                                icon,
                                summary,
                                body,
                                actions,
                                hints,
                                expire_timeout,
                            )) => {
                                let mut active = handler_active.lock().unwrap();
                                let id = if replaces_id != 0 && active.contains(&replaces_id) {
                                    replaces_id
                                } else {
                                    loop {
                                        next_id = next_id.wrapping_add(1).max(1);
                                        if !active.contains(&next_id) {
                                            break next_id;
                                        }
                                    }
                                };
                                active.insert(id);
                                let image = ["image-data", "image_data", "icon_data"]
                                    .iter()
                                    .find_map(|key| {
                                        hints.get(*key).and_then(|v| decode_image(v.0.as_ref()))
                                    });
                                let icon = hints
                                    .get("image-path")
                                    .or_else(|| hints.get("image_path"))
                                    .and_then(|v| v.0.as_str())
                                    .map(str::to_owned)
                                    .unwrap_or(icon);
                                let urgency =
                                    hints.get("urgency").and_then(|v| v.0.as_u64()).unwrap_or(1)
                                        as u8;
                                let resident = hints
                                    .get("resident")
                                    .and_then(|v| v.0.as_i64())
                                    .unwrap_or(0)
                                    != 0;
                                let actions = actions
                                    .chunks_exact(2)
                                    .map(|pair| (pair[0].clone(), pair[1].clone()))
                                    .collect();
                                let notification = Notification {
                                    id,
                                    app_name,
                                    icon,
                                    image,
                                    summary,
                                    body,
                                    urgency,
                                    actions,
                                    resident,
                                    expire_timeout,
                                };
                                if events.send(NotificationEvent::Show(notification)).is_err() {
                                    return false;
                                }
                                Ok(message.method_return().append1(id))
                            }
                            Err(error) => {
                                Err(("org.freedesktop.DBus.Error.InvalidArgs", error.to_string()))
                            }
                        }
                    }
                    Some("CloseNotification") => match message.read1::<u32>() {
                        Ok(id) if handler_active.lock().unwrap().remove(&id) => {
                            closed(connection, id, 3);
                            let _ = events.send(NotificationEvent::Close(id));
                            Ok(message.method_return())
                        }
                        Ok(_) => Err((
                            "org.freedesktop.DBus.Error.InvalidArgs",
                            "Unknown notification".into(),
                        )),
                        Err(error) => {
                            Err(("org.freedesktop.DBus.Error.InvalidArgs", error.to_string()))
                        }
                    },
                    _ => Err((
                        "org.freedesktop.DBus.Error.UnknownMethod",
                        "Unknown method".into(),
                    )),
                },
                _ => Err((
                    "org.freedesktop.DBus.Error.UnknownInterface",
                    "Unknown interface".into(),
                )),
            };
            let reply = reply.unwrap_or_else(|(name, error)| {
                message.error(
                    &name.into(),
                    &CString::new(error.replace('\0', "")).unwrap(),
                )
            });
            let _ = connection.send(reply);
            true
        }),
    );
    let _ = ready.send(true);
    loop {
        for command in commands.try_iter() {
            match command {
                Command::Stop => return Ok(()),
                Command::Invoke(id, action) => {
                    if active.lock().unwrap().contains(&id) {
                        if let Ok(signal) = Message::new_signal(PATH, INTERFACE, "ActionInvoked") {
                            let _ = connection.send(signal.append2(id, action));
                        }
                    }
                }
                Command::Close(id, reason) => {
                    if active.lock().unwrap().remove(&id) {
                        closed(&connection, id, reason);
                    }
                }
            }
        }
        connection
            .process(Duration::from_millis(50))
            .map_err(|error| error.to_string())?;
    }
}

fn closed(connection: &Connection, id: u32, reason: u32) {
    if let Ok(signal) = Message::new_signal(PATH, INTERFACE, "NotificationClosed") {
        let _ = connection.send(signal.append2(id, reason));
    }
}

fn decode_image(value: &dyn RefArg) -> Option<NotificationImage> {
    let fields: Vec<_> = value.as_iter()?.collect();
    if fields.len() != 7 {
        return None;
    }
    let width = u32::try_from(fields[0].as_i64()?).ok()?;
    let height = u32::try_from(fields[1].as_i64()?).ok()?;
    let stride = usize::try_from(fields[2].as_i64()?).ok()?;
    let alpha = fields[3].as_i64()? != 0;
    let bits = fields[4].as_i64()?;
    let channels = usize::try_from(fields[5].as_i64()?).ok()?;
    if width == 0
        || height == 0
        || width > 2048
        || height > 2048
        || bits != 8
        || channels != if alpha { 4 } else { 3 }
        || stride < width as usize * channels
    {
        return None;
    }
    let data: Vec<u8> = fields[6]
        .as_iter()?
        .take(16 * 1024 * 1024 + 1)
        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect::<Option<_>>()?;
    let required = stride
        .checked_mul(height as usize - 1)?
        .checked_add(width as usize * channels)?;
    if required > data.len() || data.len() > 16 * 1024 * 1024 {
        return None;
    }
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let start = y * stride + x * channels;
            pixels.extend_from_slice(&data[start..start + 3]);
            pixels.push(if alpha { data[start + 3] } else { 255 });
        }
    }
    Some(NotificationImage {
        width,
        height,
        pixels,
    })
}

const INTROSPECTION: &str = r#"<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
<method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
<signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface></node>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use dbus::arg::Variant;

    #[test]
    fn recognizes_an_existing_notification_daemon() {
        assert!(is_name_already_owned(
            "another notification service already owns the session bus name"
        ));
        assert!(!is_name_already_owned("failed to connect to the session bus"));
    }

    #[test]
    fn decodes_padded_rgb_and_rejects_truncated_images() {
        let image = (
            2i32,
            2i32,
            8i32,
            false,
            8i32,
            3i32,
            vec![255u8, 0, 0, 0, 255, 0, 0, 0, 0, 0, 255, 255, 255, 255],
        );
        let decoded = decode_image(&image).unwrap();
        assert_eq!(
            decoded.pixels,
            vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]
        );
        assert!(decode_image(&(2i32, 2i32, 8i32, true, 8i32, 4i32, vec![0u8; 15])).is_none());
        assert!(decode_image(&(-1i32, 2i32, 8i32, false, 8i32, 3i32, vec![0u8; 16])).is_none());
        assert!(decode_image(&(1i32, 1i32, 3i32, true, 8i32, 3i32, vec![0u8; 3])).is_none());
    }

    #[test]
    #[ignore = "run under an isolated dbus-run-session"]
    fn session_bus_notifications_replace_close_and_invoke_actions() {
        let backend = Notifications::default();
        let listener = backend.start().expect("notification service");
        let competitor = Notifications::default();
        assert!(competitor.start().is_none());
        let connection = Connection::new_session().unwrap();
        let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
        let (capabilities,): (Vec<String>,) =
            proxy.method_call(INTERFACE, "GetCapabilities", ()).unwrap();
        assert!(capabilities.contains(&"actions".to_owned()));
        let (name, _, _, spec): (String, String, String, String) = proxy
            .method_call(INTERFACE, "GetServerInformation", ())
            .unwrap();
        assert_eq!((name.as_str(), spec.as_str()), ("Coconut", "1.2"));
        let closed_events = Arc::new(Mutex::new(Vec::new()));
        let captured = closed_events.clone();
        connection
            .add_match(
                MatchRule::new_signal(INTERFACE, "NotificationClosed"),
                move |args: (u32, u32), _, _| {
                    captured.lock().unwrap().push(args);
                    true
                },
            )
            .unwrap();
        let invoked_events = Arc::new(Mutex::new(Vec::new()));
        let captured = invoked_events.clone();
        connection
            .add_match(
                MatchRule::new_signal(INTERFACE, "ActionInvoked"),
                move |args: (u32, String), _, _| {
                    captured.lock().unwrap().push(args);
                    true
                },
            )
            .unwrap();
        let notify = |replaces_id, summary: &str| {
            let mut hints = PropMap::new();
            hints.insert("urgency".into(), Variant(Box::new(2u8)));
            hints.insert(
                "image-data".into(),
                Variant(Box::new((
                    1i32,
                    1i32,
                    4i32,
                    true,
                    8i32,
                    4i32,
                    vec![20u8, 40, 60, 255],
                ))),
            );
            let (id,): (u32,) = proxy
                .method_call(
                    INTERFACE,
                    "Notify",
                    (
                        "Test",
                        replaces_id,
                        "dialog-information",
                        summary,
                        "Notification body",
                        vec!["open", "Open"],
                        hints,
                        0i32,
                    ),
                )
                .unwrap();
            id
        };
        let receive = || {
            listener
                .0
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
        };
        let id = notify(0u32, "First");
        assert_ne!(id, 0);
        match receive() {
            NotificationEvent::Show(notification) => {
                assert_eq!(notification.summary, "First");
                assert_eq!(notification.image.unwrap().pixels, vec![20, 40, 60, 255]);
                assert_eq!(notification.actions, vec![("open".into(), "Open".into())]);
                assert_eq!(notification.urgency, 2);
            }
            _ => panic!("expected Notify event"),
        }
        assert_eq!(notify(id, "Updated"), id);
        assert!(
            matches!(receive(), NotificationEvent::Show(n) if n.id == id && n.summary == "Updated")
        );
        backend.invoke(id, "open");
        for _ in 0..20 {
            connection.process(Duration::from_millis(50)).unwrap();
            if !invoked_events.lock().unwrap().is_empty() {
                break;
            }
        }
        assert_eq!(*invoked_events.lock().unwrap(), vec![(id, "open".into())]);
        let result: Result<(), _> = proxy.method_call(INTERFACE, "CloseNotification", (id,));
        result.unwrap();
        assert!(matches!(receive(), NotificationEvent::Close(n) if n == id));
        for _ in 0..20 {
            connection.process(Duration::from_millis(50)).unwrap();
            if !closed_events.lock().unwrap().is_empty()
                && !invoked_events.lock().unwrap().is_empty()
            {
                break;
            }
        }
        assert_eq!(*closed_events.lock().unwrap(), vec![(id, 3)]);
        assert_eq!(*invoked_events.lock().unwrap(), vec![(id, "open".into())]);
        let id = notify(0, "Expires");
        let _ = receive();
        backend.close(id, 1);
        for _ in 0..20 {
            connection.process(Duration::from_millis(50)).unwrap();
            if closed_events.lock().unwrap().len() == 2 {
                break;
            }
        }
        assert_eq!(closed_events.lock().unwrap()[1], (id, 1));
    }
}
