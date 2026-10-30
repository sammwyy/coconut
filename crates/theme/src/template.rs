use std::collections::BTreeMap;
use toml::Value;

pub type Context = BTreeMap<String, Value>;

pub fn flatten(prefix: &str, value: &Value, output: &mut Context) {
    if let Value::Table(table) = value {
        for (key, value) in table {
            flatten(&format!("{prefix}.{key}"), value, output);
        }
    } else {
        output.insert(prefix.into(), value.clone());
    }
}

/// Substitute known token references only. No evaluation or process execution.
pub fn render_text(source: &str, context: &Context) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let end = tail.find('}').ok_or("unterminated theme variable")?;
        let name = &tail[..end];
        let value = context
            .get(name)
            .ok_or_else(|| format!("unknown theme variable ${{{name}}}"))?;
        output.push_str(&match value {
            Value::String(v) => v.clone(),
            Value::Integer(v) => v.to_string(),
            Value::Float(v) => v.to_string(),
            Value::Boolean(v) => v.to_string(),
            _ => return Err(format!("{name} is not a scalar token")),
        });
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

pub fn render_value(value: &Value, context: &Context) -> Result<Value, String> {
    Ok(match value {
        Value::String(text) => {
            if let Some(name) = text.strip_prefix("${").and_then(|v| v.strip_suffix('}')) {
                if !name.contains(['{', '}']) {
                    return context
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("unknown theme variable {text}"));
                }
            }
            Value::String(render_text(text, context)?)
        }
        Value::Table(table) => Value::Table(
            table
                .iter()
                .map(|(k, v)| Ok((k.clone(), render_value(v, context)?)))
                .collect::<Result<_, String>>()?,
        ),
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|v| render_value(v, context))
                .collect::<Result<_, _>>()?,
        ),
        other => other.clone(),
    })
}

pub(crate) fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Table(base), Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(previous) = base.get_mut(&key) {
                    merge(previous, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}
