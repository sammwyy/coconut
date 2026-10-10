use crate::{
    components::{group, section, section_label},
    routes::destination::Section,
};
use coconut_api::settings::{Action, Snapshot, Value};
use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_widgets::{
    Button, Icon, RawView, Select, Slider, Switch, Symbol, Text, TextController, TextInput,
    TextSize,
};

use crate::{routes::detail::Page, services::settings::State};

impl State {
    fn note(&self) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let status = self.status.get();
        let busy = self.busy.get();
        if status.is_empty() && !busy {
            return empty();
        }
        Box::new(
            Text::secondary(if !snapshot.loaded {
                "Reading system services…".into()
            } else if status.is_empty() {
                "Refreshing…".into()
            } else {
                status
            })
            .size(TextSize::Sm),
        )
    }
    fn control(&self, key: &str) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let Some(pref) = snapshot.preferences.get(key) else {
            return value(if snapshot.loaded {
                "Not supported"
            } else {
                "Loading…"
            });
        };
        if !pref.writable {
            return value(match &pref.value {
                Some(Value::Bool(v)) => {
                    if *v {
                        "On".into()
                    } else {
                        "Off".into()
                    }
                }
                Some(Value::Number(v)) => format!("{v:.2}"),
                Some(Value::Text(v)) => pref
                    .choices
                    .iter()
                    .find(|choice| choice.id == *v)
                    .map(|choice| choice.label.clone())
                    .unwrap_or_else(|| v.clone()),
                None => "Unavailable".into(),
            });
        }
        let state = self.clone();
        let key = key.to_owned();
        if !pref.choices.is_empty() {
            let current = pref.value.as_ref().and_then(|v| {
                if let Value::Text(v) = v {
                    Some(v.as_str())
                } else {
                    None
                }
            });
            let selected = pref
                .choices
                .iter()
                .position(|c| Some(c.id.as_str()) == current);
            let mut choices = pref.choices.clone();
            let index = selected.unwrap_or_else(|| {
                choices.insert(
                    0,
                    coconut_api::settings::Choice {
                        id: String::new(),
                        label: "Choose…".into(),
                    },
                );
                0
            });
            let labels: Vec<_> = choices.iter().map(|c| c.label.as_str()).collect();
            let controller = self.select(&key, index);
            return Box::new(
                Select::controlled(&labels, controller)
                    .searchable()
                    .width(176.0)
                    .height(34.0)
                    .on_select(move |index| {
                        if let Some(choice) = choices.get(index).filter(|c| !c.id.is_empty()) {
                            state.apply(if key == "browser" {
                                Action::SetDefaultApp {
                                    mime: "x-scheme-handler/https".into(),
                                    desktop_id: choice.id.clone(),
                                }
                            } else {
                                Action::Set {
                                    key: key.clone(),
                                    value: Value::Text(choice.id.clone()),
                                }
                            });
                        }
                    }),
            );
        }
        match &pref.value {
            Some(Value::Bool(checked)) => {
                let checked = *checked;
                Box::new(Switch::new(checked, move || {
                    state.apply(Action::Set {
                        key: key.clone(),
                        value: Value::Bool(!checked),
                    })
                }))
            }
            Some(Value::Number(number))
                if matches!(key.as_str(), "volume" | "brightness" | "text-scale") =>
            {
                let (base, scale) = if key == "text-scale" {
                    (0.75, 1.25)
                } else {
                    (0.0, 1.0)
                };
                Box::new(Slider::new(
                    ((*number - base) / scale).clamp(0.0, 1.0) as f32,
                    move |v| {
                        state.apply(Action::Set {
                            key: key.clone(),
                            value: Value::Number(base + v as f64 * scale),
                        })
                    },
                ))
            }
            Some(Value::Number(v)) => value(format!("{v:.0} seconds")),
            Some(Value::Text(v)) => value(v),
            None => value("Unavailable"),
        }
    }
    pub(crate) fn preference(&self, label: &str, key: &str, hint: &str) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let reason = snapshot
            .preferences
            .get(key)
            .map(|p| p.reason.as_str())
            .unwrap_or("");
        item(
            label,
            if reason.is_empty() { hint } else { reason },
            self.control(key),
            || {},
        )
    }
}

pub(crate) fn empty() -> BoxedWidget {
    Box::new(RawView::new(Style::default()))
}

pub(crate) fn item(
    label: impl Into<String>,
    hint: impl Into<String>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let label = label.into();
    let glyph = match label.as_str() {
        "Displays" => {
            r#"<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>"#
        }
        "Brightness" => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>"#
        }
        "Sound output" | "Input device" => {
            r#"<path d="M3 9h4l5-4v14l-5-4H3zM16 9a5 5 0 0 1 0 6m3-9a9 9 0 0 1 0 12"/>"#
        }
        "Mouse & touchpad" => {
            r#"<rect x="6" y="2" width="12" height="20" rx="6"/><path d="M12 2v7"/>"#
        }
        "Cursor" => r#"<path d="m4 3 7 18 3-7 7-3Z"/>"#,
        "Keyboard" => {
            r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M10 13h.01M14 13h.01M18 13h.01M7 16h10"/>"#
        }
        "Connected devices" => {
            r#"<path d="M12 3v14a3 3 0 1 0 0 6m0-20-3 4m3-4 3 4M12 14l-5-4V7m5 11 5-5v-3"/><circle cx="7" cy="5" r="2"/><rect x="15" y="6" width="4" height="4"/>"#
        }
        "Printers" => r#"<path d="M6 9V3h12v6M6 18H3V9h18v9h-3M6 14h12v8H6zM17 11h.01"/>"#,
        "Power mode" => {
            r#"<rect x="2" y="6" width="18" height="12" rx="2"/><path d="M23 10v4M11 8l-3 5h6l-3 4"/>"#
        }
        "Wallpaper" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="1"/><path d="m3 17 6-6 5 5 3-3 4 4"/>"#
        }
        "Theme" => r#"<path d="m14 4 6 6M3 21l4-1L21 6l-3-3L4 17Z"/>"#,
        "Icon pack" => {
            r#"<circle cx="6" cy="17" r="3"/><rect x="14" y="14" width="6" height="6" rx="1"/><path d="m12 3 4 6H8Z"/>"#
        }
        "Fonts" => r#"<path d="M4 3h16M12 3v18M8 21h8"/>"#,
        "Sound pack" | "Notifications" => {
            r#"<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/>"#
        }
        "Top panel" => r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 8h18"/>"#,
        "Dock position" => {
            r#"<rect x="2" y="3" width="20" height="18" rx="2"/><rect x="6" y="16" width="12" height="3" rx="1"/>"#
        }
        "Widgets" | "Installed applications" => {
            r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#
        }
        "Web browser" | "Location services" => {
            r#"<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a18 18 0 0 1 0 20 18 18 0 0 1 0-20"/>"#
        }
        "Storage" => {
            r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 13h18M7 16h.01M11 16h.01"/>"#
        }
        "Activity monitor" => r#"<path d="M2 12h4l3-8 6 16 3-8h4"/>"#,
        "Updates" => {
            r#"<path d="M21 3v6h-6M3 21v-6h6M4 8a8 8 0 0 1 13-4l4 5M3 15l4 5a8 8 0 0 0 13-4"/>"#
        }
        "Date & time" | "Automatic date & time" => {
            r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#
        }
        "Camera permissions" => {
            r#"<path d="M3 6h4l2-3h6l2 3h4v14H3z"/><circle cx="12" cy="13" r="4"/>"#
        }
        "Microphone permissions" => {
            r#"<rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8"/>"#
        }
        "Password" | "Require password after sleep" => {
            r#"<rect x="4" y="10" width="16" height="12" rx="2"/><path d="M8 10V6a4 4 0 0 1 8 0v4"/>"#
        }
        _ => "",
    };
    let badge = Some(crate::components::icon_badge(
        if glyph.is_empty() {
            crate::components::setting_icon(&label)
        } else {
            crate::icons::outline(glyph)
        },
        crate::components::category_color(),
    ));
    crate::components::setting_row::icon_item(label, hint, badge, trailing, click)
}

pub(crate) fn value(text: impl Into<String>) -> BoxedWidget {
    Box::new(Text::secondary(text.into()).size(TextSize::Sm))
}

pub(crate) fn key_value(label: impl Into<String>, text: impl Into<String>) -> BoxedWidget {
    item(
        label,
        "",
        Box::new(
            Text::secondary(text.into())
                .size(TextSize::Sm)
                .max_width(320.0),
        ),
        || {},
    )
}

pub(crate) fn action(label: &str, click: impl Fn() + 'static) -> BoxedWidget {
    Box::new(Button::new(label, click))
}

pub(crate) fn card(label: &str, rows: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(
        jsx! { <Flex direction={FlexDirection::Column} gap={8.0}> {section_label(label)} {group(rows)} </Flex> },
    )
}

pub(crate) fn page(body: Vec<BoxedWidget>, state: &State) -> BoxedWidget {
    let mut body = body;
    body.push(state.note());
    section("", "", body)
}

pub(crate) fn link(
    label: &str,
    hint: impl Into<String>,
    target: Section,
    nav: &creamui_router::Router,
) -> BoxedWidget {
    let nav = nav.clone();
    let theme = creamui_theme::use_theme();
    item(
        label,
        hint,
        Box::new(Icon::new(Symbol::ChevronRight, theme.colors.text_secondary).size(16.0)),
        move || crate::routes::navigate(&nav, &target),
    )
}

pub(crate) fn detail_link(
    label: &str,
    hint: impl Into<String>,
    target: Page,
    nav: &creamui_router::Router,
) -> BoxedWidget {
    link(label, hint, Section::Detail(target), nav)
}

pub(crate) fn unsupported(label: &str, reason: &str) -> BoxedWidget {
    item(label, reason, value("Not supported"), || {})
}

pub(crate) fn input(controller: &TextController, placeholder: &str) -> BoxedWidget {
    Box::new(crate::components::form_input(
        TextInput::controlled(controller).placeholder(placeholder),
        240.0,
    ))
}

pub(crate) fn entries(state: &State, key: &str, empty_message: &str) -> BoxedWidget {
    let s = state.snapshot.get();
    let rows = s
        .entries(key)
        .iter()
        .map(|e| item(&e.name, &e.description, empty(), || {}))
        .collect::<Vec<_>>();
    group(if rows.is_empty() {
        vec![item(
            if s.loaded {
                empty_message
            } else {
                "Loading…"
            },
            "",
            empty(),
            || {},
        )]
    } else {
        rows
    })
}

pub(crate) fn collection_summary(snapshot: &Snapshot, key: &str, fallback: &str) -> String {
    let entries = snapshot.entries(key);
    if entries.is_empty() {
        fallback.into()
    } else {
        entries
            .iter()
            .map(|e| e.name.as_str())
            .take(2)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

pub(crate) fn detail_page(mut body: Vec<BoxedWidget>, state: &State) -> BoxedWidget {
    let refresh = state.clone();
    body.push(action("Refresh", move || refresh.refresh()));
    page(body, state)
}
