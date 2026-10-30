use crate::engine::MAX_EXPRESSION;

#[derive(Clone, Default)]
pub struct Entry {
    pub text: String,
    pub fresh: bool,
    pub previous: Option<String>,
}
impl Entry {
    pub fn expression(&self) -> &str {
        if self.text.trim().is_empty() {
            "0"
        } else {
            &self.text
        }
    }
    pub fn insert(&mut self, text: &str) -> Result<(), String> {
        let continuation = matches!(
            text,
            "+" | "-" | "−" | "×" | "*" | "÷" | "/" | "^" | "%" | "!" | " mod "
        );
        let replace = self.fresh && !continuation;
        if (if replace { 0 } else { self.text.len() }) + text.len() > MAX_EXPRESSION {
            return Err("Expression exceeds 4096 bytes".into());
        }
        if replace {
            self.text.clear();
        }
        self.previous = None;
        self.text.push_str(text);
        self.fresh = false;
        Ok(())
    }
    pub fn set(&mut self, text: impl Into<String>) -> Result<(), String> {
        let text = text.into();
        if text.len() > MAX_EXPRESSION {
            return Err("Expression exceeds 4096 bytes".into());
        }
        self.previous = None;
        self.text = text;
        self.fresh = false;
        Ok(())
    }
    pub fn result(&mut self, value: f64) {
        self.previous = Some(self.expression().to_owned());
        self.text = value.to_string();
        self.fresh = true;
    }
    pub fn clear(&mut self) {
        self.text.clear();
        self.previous = None;
        self.fresh = false;
    }
    pub fn backspace(&mut self) {
        self.previous = None;
        self.text.pop();
        self.fresh = false;
    }
    pub fn wrap(&mut self, prefix: &str, suffix: &str) -> Result<(), String> {
        self.set(format!("{prefix}{}{suffix}", self.text))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculator_entry_replaces_results_continues_operators_and_deletes_unicode() {
        let mut entry = Entry::default();
        entry.result(42.0);
        entry.insert("+").unwrap();
        entry.insert("8").unwrap();
        assert_eq!(entry.text, "42+8");
        entry.result(50.0);
        entry.insert("3").unwrap();
        assert_eq!(entry.text, "3");
        entry.insert("×π").unwrap();
        entry.backspace();
        assert_eq!(entry.text, "3×");
        assert!(entry.set("1".repeat(MAX_EXPRESSION + 1)).is_err());
        assert_eq!(entry.text, "3×");
    }
}
