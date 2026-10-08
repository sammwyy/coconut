use crate::common::{group, row, section};
use coconut_core::modules::{load_module_value, save_module_value};
use coconut_plugin_kit::{ConfigField, FieldKind, NumberPresentation, NumberRange};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_widgets::layout::Align;
use creamui_widgets::{Slider, Switch, Text, TextController, TextInput, TextSize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Keeps text editing state alive while Settings rebuilds its immediate-mode
/// widget tree. In particular, a numeric field must be allowed to temporarily
/// contain an empty or out-of-range value while it is being edited.
#[derive(Clone, Default)]
pub struct State {
    inputs: Rc<RefCell<HashMap<(&'static str, &'static str), TextController>>>,
}

impl State {
    fn input(&self, id: &'static str, key: &'static str, initial: String) -> TextController {
        self.inputs
            .borrow_mut()
            .entry((id, key))
            .or_insert_with(|| TextController::new(initial))
            .clone()
    }
}

pub fn build(
    _: Size,
    id: &'static str,
    label: &str,
    schema: &[ConfigField],
    state: &State,
) -> BoxedWidget {
    let table = load_table(id);
    let rows = schema
        .iter()
        .map(|field| field_row(id, field, &table, state))
        .collect();
    section(label, "", vec![group(rows)])
}

fn load_table(id: &str) -> toml::value::Table {
    match load_module_value(id) {
        toml::Value::Table(table) => table,
        _ => toml::value::Table::new(),
    }
}

fn field_row(
    id: &'static str,
    field: &ConfigField,
    table: &toml::value::Table,
    state: &State,
) -> BoxedWidget {
    match field.kind {
        FieldKind::Toggle => {
            let default = matches!(field.default, coconut_plugin_kit::ConfigValue::Bool(true));
            let checked = table
                .get(field.key)
                .and_then(toml::Value::as_bool)
                .unwrap_or(default);
            let key = field.key;
            let switch: BoxedWidget = Box::new(Switch::new(checked, move || {
                update_field(id, key, toml::Value::Boolean(!checked));
            }));
            row(field.label, switch)
        }
        FieldKind::Number {
            range,
            presentation,
            suffix,
        } => {
            let default = field.default.as_f64().unwrap_or(0.0);
            let value = table
                .get(field.key)
                .and_then(toml::Value::as_float)
                .or_else(|| {
                    table
                        .get(field.key)
                        .and_then(toml::Value::as_integer)
                        .map(|v| v as f64)
                })
                .unwrap_or(default);
            match presentation {
                NumberPresentation::Slider => {
                    slider_row(id, field.key, field.label, value, range, suffix)
                }
                NumberPresentation::Input => {
                    input_row(id, field.key, field.label, value, range, suffix, state)
                }
            }
        }
    }
}

fn slider_row(
    id: &'static str,
    key: &'static str,
    label: &str,
    value: f64,
    range: NumberRange,
    suffix: &str,
) -> BoxedWidget {
    let normalized = ((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0) as f32;
    let slider: BoxedWidget = Box::new(Slider::new(normalized, move |normalized: f32| {
        let raw = range.min + normalized as f64 * (range.max - range.min);
        update_field(id, key, toml::Value::Float(snap(raw, range)));
    }));
    let value_text = format!("{}{suffix}", format_number(value, range.step));
    let control: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={10.0}>
            {slider}
            {Box::new(Text::secondary(value_text).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    });
    row(label, control)
}

fn input_row(
    id: &'static str,
    key: &'static str,
    label: &str,
    value: f64,
    range: NumberRange,
    _suffix: &str,
    state: &State,
) -> BoxedWidget {
    let text = format_number(value, range.step);
    let controller = state.input(id, key, text);
    controller.on_change(move |_, value| {
        if let Ok(parsed) = value.parse::<f64>() {
            if parsed.is_finite() && (range.min..=range.max).contains(&parsed) {
                update_field(id, key, toml::Value::Float(parsed));
            }
        }
        // Do not clamp while the user is typing. For example, changing `60`
        // to `70` necessarily passes through `6` (and often an empty value).
        // The controller keeps that draft and its cursor across rebuilds;
        // only an in-range number is written to the configuration.
        Some(value.to_owned())
    });
    let input: BoxedWidget = Box::new(TextInput::controlled(&controller));
    row(label, input)
}

fn snap(raw: f64, range: NumberRange) -> f64 {
    let stepped = if range.step > 0.0 {
        (raw / range.step).round() * range.step
    } else {
        raw
    };
    stepped.clamp(range.min, range.max)
}

fn format_number(value: f64, step: f64) -> String {
    if step.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn update_field(id: &str, key: &str, value: toml::Value) {
    let mut table = load_table(id);
    table.insert(key.to_owned(), value);
    if let Err(error) = save_module_value(id, &toml::Value::Table(table)) {
        eprintln!("settings: failed to save modules/{id}.toml: {error}");
        return;
    }
    if let Err(error) = coconut_core::ipc::publish_module_config_changed() {
        eprintln!("settings: failed to notify the desktop process about the update: {error}");
    }
}
