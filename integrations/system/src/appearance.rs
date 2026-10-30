//! Desktop-owned accessibility applies under every compositor.
use super::*;

pub fn supports(key: &str) -> bool {
    matches!(key, "contrast" | "text-scale")
}

pub fn discover(snapshot: &mut Snapshot) {
    match coconut_theme::Store::default().selection() {
        Ok(selection) => {
            snapshot.preferences.insert(
                "contrast".into(),
                boolean(selection.accessibility.high_contrast, true),
            );
            snapshot.preferences.insert(
                "text-scale".into(),
                number(f64::from(selection.accessibility.text_scale), true),
            );
        }
        Err(error) => snapshot.errors.push(error),
    }
}

pub fn set(key: &str, value: Value) -> Result<(), String> {
    let store = coconut_theme::Store::default();
    let mut selection = store.selection()?;
    match (key, value) {
        ("contrast", Value::Bool(value)) => selection.accessibility.high_contrast = value,
        ("text-scale", Value::Number(value))
            if value.is_finite() && (0.75..=2.0).contains(&value) =>
        {
            selection.accessibility.text_scale = value as f32
        }
        _ => return Err("Invalid accessibility preference".into()),
    }
    store.apply(&selection)?;
    coconut_core::ipc::publish_theme_reload()
}
