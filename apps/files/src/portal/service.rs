//! XDG Desktop Portal backend for File Explorer.

use super::{
    options::{self, invalid_arguments},
    PortalSelection, BACKEND_SERVICE,
};
use crate::{app::show_window, window::WindowRequest};
use dbus::{
    arg::{PropMap, Variant},
    blocking::{stdintf::org_freedesktop_dbus::RequestNameReply, Connection},
    channel::{MatchingReceiver, Sender},
    message::{MatchRule, Message},
};
use std::{collections::HashMap, ffi::CString, time::Duration};
use url::Url;

const PATH: &str = "/org/freedesktop/portal/desktop";
const FILE_CHOOSER: &str = "org.freedesktop.impl.portal.FileChooser";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";
const VERSION: u32 = 4;

const INTROSPECTION: &str = r#"<node>
  <interface name="org.freedesktop.impl.portal.FileChooser">
    <method name="OpenFile"><arg type="o" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="u" direction="out"/><arg type="a{sv}" direction="out"/></method>
    <method name="SaveFile"><arg type="o" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="u" direction="out"/><arg type="a{sv}" direction="out"/></method>
    <property name="version" type="u" access="read"/>
  </interface>
</node>"#;

#[derive(Clone, Copy)]
enum RequestKind {
    Open,
    Save,
}

pub fn run() -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let owner = connection
        .request_name(BACKEND_SERVICE, false, false, true)
        .map_err(|error| error.to_string())?;
    if owner != RequestNameReply::PrimaryOwner {
        return Err(format!("{BACKEND_SERVICE} is already running"));
    }
    let mut rule = MatchRule::new_method_call();
    rule.path = Some(PATH.into());
    connection.start_receive(
        rule,
        Box::new(|message, connection| {
            let reply = dispatch(&message).unwrap_or_else(|(name, detail)| {
                message.error(
                    &name.into(),
                    &CString::new(detail.replace('\0', "")).expect("NUL bytes removed"),
                )
            });
            let _ = connection.send(reply);
            true
        }),
    );
    loop {
        connection
            .process(Duration::from_millis(250))
            .map_err(|error| error.to_string())?;
    }
}

fn dispatch(message: &Message) -> Result<Message, (&'static str, String)> {
    match (message.interface().as_deref(), message.member().as_deref()) {
        (Some(FILE_CHOOSER), Some("OpenFile")) => choose(message, RequestKind::Open),
        (Some(FILE_CHOOSER), Some("SaveFile")) => choose(message, RequestKind::Save),
        (Some(PROPERTIES), Some("Get")) => property_get(message),
        (Some(PROPERTIES), Some("GetAll")) => property_get_all(message),
        (Some(INTROSPECTABLE), Some("Introspect")) => {
            Ok(message.method_return().append1(INTROSPECTION))
        }
        (Some(FILE_CHOOSER), _) => Err((
            "org.freedesktop.DBus.Error.UnknownMethod",
            "Unsupported file chooser method".into(),
        )),
        _ => Err((
            "org.freedesktop.DBus.Error.UnknownInterface",
            "Unsupported D-Bus interface".into(),
        )),
    }
}

fn choose(message: &Message, kind: RequestKind) -> Result<Message, (&'static str, String)> {
    choose_with(message, kind, show_window)
}

fn choose_with(
    message: &Message,
    kind: RequestKind,
    show: impl FnOnce(WindowRequest) -> Option<PortalSelection>,
) -> Result<Message, (&'static str, String)> {
    type Arguments = (dbus::Path<'static>, String, String, String, PropMap);
    let (_handle, app_id, parent_window, title, options) =
        message.read_all::<Arguments>().map_err(invalid_arguments)?;
    let request = options::request(
        title,
        app_id,
        parent_window,
        matches!(kind, RequestKind::Save),
        &options,
    )?;
    let mut results: PropMap = HashMap::new();
    let status = if let Some(selection) = show(request) {
        let uri: String = Url::from_file_path(&selection.path)
            .map_err(|_| {
                (
                    "org.freedesktop.portal.Error.Failed",
                    format!("Cannot create a file URI for {}", selection.path.display()),
                )
            })?
            .into();
        results.insert("uris".into(), Variant(Box::new(vec![uri])));
        if let Some(filter) = selection.current_filter {
            results.insert("current_filter".into(), Variant(Box::new(filter.wire())));
        }
        0_u32
    } else {
        1_u32
    };
    Ok(message.method_return().append2(status, results))
}

fn property_get(message: &Message) -> Result<Message, (&'static str, String)> {
    let (interface, property): (String, String) = message.read2().map_err(invalid_arguments)?;
    if interface == FILE_CHOOSER && property == "version" {
        Ok(message.method_return().append1(Variant(VERSION)))
    } else {
        Err((
            "org.freedesktop.DBus.Error.UnknownProperty",
            format!("Unknown property {interface}.{property}"),
        ))
    }
}

fn property_get_all(message: &Message) -> Result<Message, (&'static str, String)> {
    let interface: String = message.read1().map_err(invalid_arguments)?;
    if interface != FILE_CHOOSER {
        return Err(("org.freedesktop.DBus.Error.UnknownInterface", interface));
    }
    let mut values: PropMap = HashMap::new();
    values.insert("version".into(), Variant(Box::new(VERSION)));
    Ok(message.method_return().append1(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::{FileFilter, FilterRule, PortalType};
    use crate::window::WindowMode;
    fn call() -> Message {
        let mut message =
            Message::new_method_call(BACKEND_SERVICE, PATH, FILE_CHOOSER, "OpenFile").unwrap();
        message.set_serial(42);
        message.append_all((
            dbus::Path::new("/org/freedesktop/portal/desktop/request/test/1").unwrap(),
            "org.test.App",
            "",
            "Open image",
            PropMap::new(),
        ));
        message
    }
    #[test]
    fn chooser_response_preserves_uri_and_selected_filter() {
        let filter = FileFilter {
            name: "Images".into(),
            rules: vec![FilterRule::Glob("*.png".into())],
        };
        let reply = choose_with(&call(), RequestKind::Open, |request| {
            let WindowMode::Portal(portal) = request.mode else {
                panic!("expected portal")
            };
            assert_eq!(portal.kind, PortalType::OpenFile);
            assert_eq!(portal.app_id, "org.test.App");
            Some(PortalSelection {
                path: "/tmp/a b.png".into(),
                current_filter: Some(filter.clone()),
            })
        })
        .unwrap();
        let (status, results): (u32, PropMap) = reply.read2().unwrap();
        assert_eq!(status, 0);
        assert_eq!(
            results["uris"]
                .0
                .as_iter()
                .unwrap()
                .next()
                .unwrap()
                .as_str(),
            Some("file:///tmp/a%20b.png")
        );
        let decoded = options::request("".into(), "".into(), "".into(), false, &results).unwrap();
        let WindowMode::Portal(portal) = decoded.mode else {
            panic!("expected portal")
        };
        assert_eq!(portal.current_filter, Some(filter));
        let reply = choose_with(&call(), RequestKind::Open, |_| None).unwrap();
        let (status, results): (u32, PropMap) = reply.read2().unwrap();
        assert_eq!(status, 1);
        assert!(results.is_empty());
    }
}
