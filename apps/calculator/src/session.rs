use crate::engine::{evaluate, Context};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Calculation {
    pub expression: String,
    pub result: f64,
    pub detail: String,
}

#[derive(Default)]
pub struct Session {
    pub context: Context,
    pub memory: Option<f64>,
    pub history: VecDeque<Calculation>,
}
impl Session {
    pub fn preview(&self, expression: &str) -> Result<f64, String> {
        evaluate(expression, self.context)
    }
    pub fn calculate(&mut self, expression: &str) -> Result<f64, String> {
        let result = self.preview(expression)?;
        self.history.push_front(Calculation {
            expression: expression.into(),
            result,
            detail: if self.context.angle == crate::engine::Angle::Degrees {
                "Deg"
            } else {
                "Rad"
            }
            .into(),
        });
        self.history.truncate(100);
        self.context.answer = result;
        Ok(result)
    }
    pub fn record(&mut self, expression: String, result: f64, detail: String) {
        self.history.push_front(Calculation {
            expression,
            result,
            detail,
        });
        self.history.truncate(100);
        self.context.answer = result;
    }
    pub fn store(&mut self, expression: &str, operation: Memory) -> Result<(), String> {
        if operation == Memory::Clear {
            self.memory = None;
            self.context.memory = 0.0;
            return Ok(());
        }
        let value = self.preview(expression)?;
        let value = match operation {
            Memory::Add => self.memory.unwrap_or(0.0) + value,
            Memory::Subtract => self.memory.unwrap_or(0.0) - value,
            _ => value,
        };
        if !value.is_finite() {
            return Err("Memory value is outside the finite number range".into());
        }
        self.memory = Some(value);
        self.context.memory = value;
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Memory {
    Clear,
    Store,
    Add,
    Subtract,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_calculation_preserves_registers_and_history() {
        let mut session = Session::default();
        session.calculate("6*7").unwrap();
        assert!(session.calculate("1/0").is_err());
        assert_eq!(session.context.answer, 42.0);
        assert_eq!(session.history.len(), 1);
        assert_eq!(session.calculate("ans+1").unwrap(), 43.0);
        for _ in 0..110 {
            session.calculate("1").unwrap();
        }
        assert_eq!(session.history.len(), 100);
    }
    #[test]
    fn memory_can_store_zero_and_rejects_invalid_updates() {
        let mut session = Session::default();
        session.store("0", Memory::Store).unwrap();
        assert_eq!(session.memory, Some(0.0));
        session.store("12", Memory::Add).unwrap();
        session.store("2", Memory::Subtract).unwrap();
        assert!(session.store("1/0", Memory::Store).is_err());
        assert_eq!(session.memory, Some(10.0));
        session.store("", Memory::Clear).unwrap();
        assert_eq!(session.memory, None);
        assert_eq!(session.context.memory, 0.0);
    }
}
