use crate::{gtk, INTERFACE, PATH};
use coconut_theme::ResolvedTheme;
use dbus::{
    arg::{PropMap, RefArg, Variant},
    blocking::Connection,
    channel::Sender,
    message::{MatchRule, Message},
};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::CString,
    sync::{Arc, Mutex},
    time::Duration,
};

const GTK: &str = "org.freedesktop.impl.portal.desktop.gtk";
const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME: &str = "org.gnome.desktop.interface";
type WireSettings = HashMap<String, PropMap>;
pub(crate) type Settings = BTreeMap<String, BTreeMap<String, Value>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Uint(u32),
    Text(String),
    Bool(bool),
    Float(f64),
    Rgb((f64, f64, f64)),
}
impl Value {
    fn variant(&self) -> Variant<Box<dyn RefArg>> {
        Variant(match self {
            Self::Uint(value) => Box::new(*value),
            Self::Text(value) => Box::new(value.clone()),
            Self::Bool(value) => Box::new(*value),
            Self::Float(value) => Box::new(*value),
            Self::Rgb(value) => Box::new(*value),
        })
    }
}

pub(crate) fn settings(a: &ResolvedTheme, theme_name: &str) -> Settings {
    let c = a.tokens.colors.accent;
    BTreeMap::from([
        (
            // Extension for toolkits that understand named variants. Standard
            // consumers continue using org.freedesktop.appearance.color-scheme.
            "org.creamui.appearance".into(),
            BTreeMap::from([
                ("variant".into(), Value::Text(a.variant_id.clone())),
                ("theme-id".into(), Value::Text(a.theme_id.clone())),
            ]),
        ),
        (
            APPEARANCE.into(),
            BTreeMap::from([
                (
                    "color-scheme".into(),
                    Value::Uint(if gtk::dark(a) { 1 } else { 2 }),
                ),
                (
                    "accent-color".into(),
                    Value::Rgb((
                        f64::from(c.r) / 255.0,
                        f64::from(c.g) / 255.0,
                        f64::from(c.b) / 255.0,
                    )),
                ),
                (
                    "contrast".into(),
                    Value::Uint(u32::from(a.selection.accessibility.high_contrast)),
                ),
            ]),
        ),
        (
            GNOME.into(),
            BTreeMap::from([
                ("gtk-theme".into(), Value::Text(theme_name.into())),
                (
                    "color-scheme".into(),
                    Value::Text(
                        if gtk::dark(a) {
                            "prefer-dark"
                        } else {
                            "prefer-light"
                        }
                        .into(),
                    ),
                ),
                ("font-name".into(), Value::Text(gtk::font_name(a))),
                (
                    "text-scaling-factor".into(),
                    Value::Float(f64::from(a.selection.accessibility.text_scale)),
                ),
            ]),
        ),
        (
            "org.gnome.desktop.a11y.interface".into(),
            BTreeMap::from([(
                "high-contrast".into(),
                Value::Bool(a.selection.accessibility.high_contrast),
            )]),
        ),
    ])
}

fn namespace_matches(namespace: &str, patterns: &[String]) -> bool {
    patterns.is_empty()
        || patterns.iter().any(|pattern| {
            pattern.is_empty()
                || pattern == namespace
                || pattern
                    .strip_suffix('*')
                    .is_some_and(|prefix| namespace.starts_with(prefix))
        })
}

// GTK retains responsibility for settings Coconut doesn't own (cursor,
// icons, animations, font rendering, etc.). Delegation avoids publishing
// duplicate/conflicting appearance values from two Settings backends.
fn gtk_read_all(namespaces: &[String]) -> WireSettings {
    let result = (|| {
        let connection = Connection::new_session()?;
        let proxy = connection.with_proxy(GTK, PATH, Duration::from_secs(2));
        let (values,): (WireSettings,) = proxy.method_call(INTERFACE, "ReadAll", (namespaces,))?;
        Ok::<_, dbus::Error>(values)
    })();
    result.unwrap_or_default()
}

fn gtk_read(namespace: &str, key: &str) -> Result<Variant<Box<dyn RefArg>>, dbus::Error> {
    let connection = Connection::new_session()?;
    let proxy = connection.with_proxy(GTK, PATH, Duration::from_secs(2));
    let (value,): (Variant<Box<dyn RefArg>>,) =
        proxy.method_call(INTERFACE, "Read", (namespace, key))?;
    Ok(value)
}

pub(crate) fn dispatch(message: &Message, settings: &Settings) -> Message {
    dispatch_result(message, settings).unwrap_or_else(|(name, reason)| {
        message.error(
            &name.into(),
            &CString::new(reason.replace('\0', "")).unwrap(),
        )
    })
}

fn dispatch_result(
    message: &Message,
    settings: &Settings,
) -> Result<Message, (&'static str, String)> {
    let invalid = |error: dbus::arg::TypeMismatchError| {
        ("org.freedesktop.DBus.Error.InvalidArgs", error.to_string())
    };
    match (message.interface().as_deref(), message.member().as_deref()) {
        (Some(INTERFACE), Some("Read")) => {
            let (namespace, key): (String, String) = message.read2().map_err(invalid)?;
            let value =
                if let Some(value) = settings.get(&namespace).and_then(|values| values.get(&key)) {
                    value.variant()
                } else {
                    gtk_read(&namespace, &key).map_err(|_| {
                        (
                            "org.freedesktop.portal.Error.NotFound",
                            format!("Unknown setting {namespace}.{key}"),
                        )
                    })?
                };
            Ok(message.method_return().append1(value))
        }
        (Some(INTERFACE), Some("ReadAll")) => {
            let namespaces: Vec<String> = message.read1().map_err(invalid)?;
            let mut values = gtk_read_all(&namespaces);
            for (namespace, entries) in settings {
                if !namespace_matches(namespace, &namespaces) {
                    continue;
                }
                let namespace_values = values.entry(namespace.clone()).or_default();
                for (key, value) in entries {
                    namespace_values.insert(key.clone(), value.variant());
                }
            }
            Ok(message.method_return().append1(values))
        }
        (Some("org.freedesktop.DBus.Properties"), Some("Get")) => {
            let (interface, key): (String, String) = message.read2().map_err(invalid)?;
            if interface == INTERFACE && key == "version" {
                Ok(message.method_return().append1(Variant(1u32)))
            } else {
                Err(("org.freedesktop.DBus.Error.UnknownProperty", key))
            }
        }
        (Some("org.freedesktop.DBus.Properties"), Some("GetAll")) => {
            let interface: String = message.read1().map_err(invalid)?;
            if interface != INTERFACE {
                return Err(("org.freedesktop.DBus.Error.UnknownInterface", interface));
            }
            let values: PropMap =
                HashMap::from([("version".into(), Variant(Box::new(1u32) as Box<dyn RefArg>))]);
            Ok(message.method_return().append1(values))
        }
        (Some("org.freedesktop.DBus.Introspectable"), Some("Introspect")) => {
            Ok(message.method_return().append1(INTROSPECTION))
        }
        (Some("org.freedesktop.DBus.Peer"), Some("Ping")) => Ok(message.method_return()),
        _ => Err((
            "org.freedesktop.DBus.Error.UnknownMethod",
            "Unsupported method".into(),
        )),
    }
}

pub(crate) fn publish_changes(
    connection: &Connection,
    old: &Settings,
    next: &Settings,
) -> Result<(), String> {
    for (namespace, values) in next {
        for (key, value) in values {
            if old.get(namespace).and_then(|values| values.get(key)) == Some(value) {
                continue;
            }
            let signal = Message::new_signal(PATH, INTERFACE, "SettingChanged")?.append3(
                namespace,
                key,
                value.variant(),
            );
            connection
                .send(signal)
                .map_err(|_| "Could not publish appearance change".to_owned())?;
        }
    }
    Ok(())
}

pub(crate) fn forward_gtk_changes(
    connection: &Connection,
    settings: Arc<Mutex<Settings>>,
) -> Result<(), String> {
    let rule = MatchRule::new_signal(INTERFACE, "SettingChanged")
        .with_sender(GTK)
        .with_path(PATH);
    connection
        .add_match(
            rule,
            move |(namespace, key, value): (String, String, Variant<Box<dyn RefArg>>),
                  connection,
                  _| {
                if !settings
                    .lock()
                    .unwrap()
                    .get(&namespace)
                    .is_some_and(|values| values.contains_key(&key))
                {
                    if let Ok(signal) = Message::new_signal(PATH, INTERFACE, "SettingChanged") {
                        let _ = connection.send(signal.append3(namespace, key, value));
                    }
                }
                true
            },
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

const INTROSPECTION: &str = r#"<node>
<interface name="org.freedesktop.impl.portal.Settings">
  <method name="ReadAll"><arg name="namespaces" type="as" direction="in"/><arg name="value" type="a{sa{sv}}" direction="out"/></method>
  <method name="Read"><arg name="namespace" type="s" direction="in"/><arg name="key" type="s" direction="in"/><arg name="value" type="v" direction="out"/></method>
  <signal name="SettingChanged"><arg name="namespace" type="s"/><arg name="key" type="s"/><arg name="value" type="v"/></signal>
  <property name="version" type="u" access="read"/>
</interface>
<interface name="org.freedesktop.DBus.Properties">
  <method name="Get"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
  <method name="GetAll"><arg type="s" direction="in"/><arg type="a{sv}" direction="out"/></method>
</interface>
<interface name="org.freedesktop.DBus.Introspectable"><method name="Introspect"><arg type="s" direction="out"/></method></interface>
</node>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_theme::{Package, Selection};
    fn appearance(variant: &str) -> ResolvedTheme {
        Package::builtin()
            .unwrap()
            .resolve(&Selection {
                variant: variant.into(),
                ..Selection::default()
            })
            .unwrap()
    }
    #[test]
    fn portal_exports_declared_mode_and_standard_wire_types() {
        for (variant, expected) in [("light", 2), ("dark", 1), ("midnight", 1)] {
            let values = settings(&appearance(variant), "Coconut");
            assert_eq!(values[APPEARANCE]["color-scheme"], Value::Uint(expected));
            assert_eq!(
                values["org.creamui.appearance"]["variant"],
                Value::Text(variant.into())
            );
            let mut call = Message::new_method_call(crate::SERVICE, PATH, INTERFACE, "Read")
                .unwrap()
                .append2(APPEARANCE, "color-scheme");
            call.set_serial(42);
            let value: Variant<u32> = dispatch(&call, &values).read1().unwrap();
            assert_eq!(value.0, expected);
            let mut call = Message::new_method_call(crate::SERVICE, PATH, INTERFACE, "Read")
                .unwrap()
                .append2(APPEARANCE, "accent-color");
            call.set_serial(43);
            let _: Variant<(f64, f64, f64)> = dispatch(&call, &values).read1().unwrap();
        }
    }
    #[test]
    fn namespace_filter_handles_empty_exact_and_trailing_glob() {
        assert!(namespace_matches(APPEARANCE, &[]));
        assert!(namespace_matches(APPEARANCE, &["".into()]));
        assert!(namespace_matches(APPEARANCE, &["org.freedesktop.*".into()]));
        assert!(namespace_matches(APPEARANCE, &[APPEARANCE.into()]));
        assert!(!namespace_matches(APPEARANCE, &["org.gnome.*".into()]));
    }
}
