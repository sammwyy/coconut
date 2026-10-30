use std::f64::consts::{E, PI, TAU};

pub const MAX_EXPRESSION: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Angle {
    #[default]
    Degrees,
    Radians,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Context {
    pub angle: Angle,
    pub answer: f64,
    pub memory: f64,
}

pub fn evaluate(expression: &str, context: Context) -> Result<f64, String> {
    if expression.len() > MAX_EXPRESSION {
        return Err("Expression exceeds 4096 bytes".into());
    }
    let mut parser = Parser {
        source: expression,
        offset: 0,
        depth: 0,
        context,
    };
    let value = parser.expression(0)?;
    parser.space();
    if parser.offset != expression.len() {
        return Err(format!(
            "Unexpected input at position {}",
            parser.offset + 1
        ));
    }
    finite(value)
}

fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err("Result is outside the real finite number range".into())
    }
}

struct Parser<'a> {
    source: &'a str,
    offset: usize,
    depth: usize,
    context: Context,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.offset += c.len_utf8();
        Some(c)
    }
    fn take(&mut self, c: char) -> bool {
        self.space();
        if self.peek() == Some(c) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expression(&mut self, minimum: u8) -> Result<f64, String> {
        self.depth += 1;
        if self.depth > 128 {
            return Err("Expression is nested too deeply".into());
        }
        self.space();
        let mut left = if self.take('+') {
            self.expression(25)?
        } else if self.take('-') || self.take('−') {
            -self.expression(25)?
        } else {
            self.atom()?
        };
        loop {
            self.space();
            if minimum <= 40 && (self.peek() == Some('!') || self.peek() == Some('%')) {
                left = if self.bump() == Some('!') {
                    factorial(left)?
                } else {
                    left / 100.0
                };
                continue;
            }
            let rest = &self.source[self.offset..];
            let (operator, binding, length) = match self.peek() {
                Some('+') => ('+', 10, 1),
                Some('-' | '−') => ('-', 10, self.peek().unwrap().len_utf8()),
                Some('*' | '×') => ('*', 20, self.peek().unwrap().len_utf8()),
                Some('/' | '÷') => ('/', 20, self.peek().unwrap().len_utf8()),
                Some('^') => ('^', 30, 1),
                _ if rest.starts_with("mod") && !rest[3..].starts_with(char::is_alphabetic) => {
                    ('m', 20, 3)
                }
                Some('(' | 'π' | 'τ') => ('*', 20, 0),
                Some(c) if c.is_ascii_alphabetic() => ('*', 20, 0),
                _ => break,
            };
            if binding < minimum {
                break;
            }
            self.offset += length;
            let right = self.expression(if operator == '^' {
                binding
            } else {
                binding + 1
            })?;
            left = match operator {
                '+' => left + right,
                '-' => left - right,
                '*' => left * right,
                '/' | 'm' if right == 0.0 => return Err("Cannot divide by zero".into()),
                '/' => left / right,
                'm' => left % right,
                _ => left.powf(right),
            };
            left = finite(left)?;
        }
        self.depth -= 1;
        finite(left)
    }
    fn atom(&mut self) -> Result<f64, String> {
        self.space();
        if self.take('(') {
            let value = self.expression(0)?;
            if !self.take(')') {
                return Err("Missing closing parenthesis".into());
            }
            return Ok(value);
        }
        let start = self.offset;
        if self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                self.bump();
            }
            if self.peek().is_some_and(|c| c == 'e' || c == 'E') {
                let suffix = &self.source[self.offset + 1..];
                if suffix.starts_with(|c: char| c.is_ascii_digit() || c == '+' || c == '-') {
                    self.bump();
                    if self.peek().is_some_and(|c| c == '+' || c == '-') {
                        self.bump();
                    }
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.bump();
                    }
                }
            }
            return self.source[start..self.offset]
                .parse::<f64>()
                .map_err(|_| "Invalid number".into())
                .and_then(finite);
        }
        while self
            .peek()
            .is_some_and(|c| c.is_alphabetic() || c.is_ascii_digit())
        {
            self.bump();
        }
        let name = self.source[start..self.offset].to_lowercase();
        match name.as_str() {
            "pi" | "π" => return Ok(PI),
            "tau" | "τ" => return Ok(TAU),
            "e" => return Ok(E),
            "ans" => return Ok(self.context.answer),
            "mem" => return Ok(self.context.memory),
            "" => return Err("Enter a number or expression".into()),
            _ => {}
        }
        if !self.take('(') {
            return Err(format!("Unknown value '{name}'"));
        }
        let mut arguments = vec![self.expression(0)?];
        while self.take(',') {
            if arguments.len() == 64 {
                return Err("A function accepts at most 64 arguments".into());
            }
            arguments.push(self.expression(0)?);
        }
        if !self.take(')') {
            return Err("Missing closing parenthesis".into());
        }
        self.function(&name, &arguments)
    }
    fn function(&self, name: &str, args: &[f64]) -> Result<f64, String> {
        let x = args[0];
        let radians = |v: f64| {
            if self.context.angle == Angle::Degrees {
                v.to_radians()
            } else {
                v
            }
        };
        let angle = |v: f64| {
            if self.context.angle == Angle::Degrees {
                v.to_degrees()
            } else {
                v
            }
        };
        let value = match (name, args.len()) {
            ("sin", 1) => radians(x).sin(),
            ("cos", 1) => radians(x).cos(),
            ("tan", 1) => {
                let r = radians(x);
                if r.cos().abs() < 1e-15 {
                    return Err("Tangent is undefined at this angle".into());
                }
                r.tan()
            }
            ("asin", 1) => angle(x.asin()),
            ("acos", 1) => angle(x.acos()),
            ("atan", 1) => angle(x.atan()),
            ("sinh", 1) => x.sinh(),
            ("cosh", 1) => x.cosh(),
            ("tanh", 1) => x.tanh(),
            ("asinh", 1) => x.asinh(),
            ("acosh", 1) => x.acosh(),
            ("atanh", 1) => x.atanh(),
            ("reciprocal", 1) => 1.0 / x,
            ("sqrt", 1) => x.sqrt(),
            ("cbrt", 1) => x.cbrt(),
            ("ln", 1) => x.ln(),
            ("log" | "log10", 1) => x.log10(),
            ("log2", 1) => x.log2(),
            ("log", 2) if args[1] > 0.0 && args[1] != 1.0 => x.log(args[1]),
            ("exp", 1) => x.exp(),
            ("abs", 1) => x.abs(),
            ("floor", 1) => x.floor(),
            ("ceil", 1) => x.ceil(),
            ("round", 1) => x.round(),
            ("fact", 1) => factorial(x)?,
            ("pow", 2) => x.powf(args[1]),
            ("ncr" | "npr", 2) => combinations(x, args[1], name == "npr")?,
            ("min", _) => args.iter().copied().fold(f64::INFINITY, f64::min),
            ("max", _) => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            ("sum", _) => args.iter().sum(),
            ("mean", _) => args.iter().sum::<f64>() / args.len() as f64,
            _ => return Err(format!("Unknown function or invalid arguments: {name}")),
        };
        finite(value)
    }
}

fn integer(value: f64, maximum: f64) -> Result<u32, String> {
    if value < 0.0 || value > maximum || value.fract() != 0.0 {
        return Err(format!("Expected a whole number between 0 and {maximum}"));
    }
    Ok(value as u32)
}
fn factorial(value: f64) -> Result<f64, String> {
    let n = integer(value, 170.0)?;
    Ok((1..=n).fold(1.0, |value, n| value * n as f64))
}
fn combinations(n: f64, r: f64, permutations: bool) -> Result<f64, String> {
    let n = integer(n, 10000.0)?;
    let r = integer(r, n as f64)?;
    let r = if permutations { r } else { r.min(n - r) };
    let mut value = 1.0;
    for i in 0..r {
        value *= if permutations {
            (n - i) as f64
        } else {
            (n - i) as f64 / (i + 1) as f64
        };
        value = finite(value)?;
    }
    Ok(value)
}

pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() >= 1e15 || value.abs() < 1e-8 {
        return format!("{value:.12e}");
    }
    let decimals = (14 - value.abs().log10().floor() as i32).clamp(0, 22) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').into()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(expression: &str, expected: f64) {
        let result = evaluate(expression, Context::default()).unwrap();
        assert!(
            (result - expected).abs() < 1e-10 * expected.abs().max(1.0),
            "{expression}: {result} != {expected}"
        );
    }
    #[test]
    fn precedence_and_notation() {
        for (expression, expected) in [
            ("2+3*4", 14.0),
            ("(2+3)*4", 20.0),
            ("2^3^2", 512.0),
            ("-2^2", -4.0),
            ("2^-3", 0.125),
            ("3!^2", 36.0),
            ("200*10%", 20.0),
            ("2(3+4)", 14.0),
            ("2pi", 2.0 * PI),
            ("1e-3 + 2E2", 200.001),
            ("7 mod 4", 3.0),
            ("8÷2×3−1", 11.0),
        ] {
            check(expression, expected);
        }
    }
    #[test]
    fn scientific_functions() {
        for (expression, expected) in [
            ("sin(30)", 0.5),
            ("asin(0.5)", 30.0),
            ("cos(60)", 0.5),
            ("tan(45)", 1.0),
            ("ln(e)", 1.0),
            ("log(100)", 2.0),
            ("log(8,2)", 3.0),
            ("sqrt(81)+cbrt(-8)", 7.0),
            ("ncr(52,5)", 2598960.0),
            ("ncr(1028,514)", 7.156051054877897e307),
            ("npr(10,3)", 720.0),
            ("0!", 1.0),
            ("mean(2,4,6)", 4.0),
            ("max(-2,5,3)", 5.0),
            ("abs(-5)+floor(2.9)", 7.0),
            ("log2(8)", 3.0),
            ("sinh(0)+cosh(0)", 1.0),
        ] {
            check(expression, expected);
        }
        let context = Context {
            angle: Angle::Radians,
            answer: 42.0,
            memory: 8.0,
        };
        assert_eq!(evaluate("sin(pi/2)+ans+mem", context).unwrap(), 51.0);
    }
    #[test]
    fn invalid_inputs_and_work_limits() {
        for expression in [
            "",
            "1/0",
            "0 mod 0",
            "sqrt(-1)",
            "ln(0)",
            "asin(2)",
            "tan(90)",
            "171!",
            "(-1)!",
            "2.5!",
            "1e999",
            "1e+",
            "2..3",
            "(2+3",
            "2+3)",
            "unknown(1)",
            "sin(1,2)",
            "ncr(3,4)",
            "log(2,1)",
            "2 3",
        ] {
            assert!(
                evaluate(expression, Context::default()).is_err(),
                "{expression}"
            );
        }
        assert!(evaluate(&"(".repeat(200), Context::default()).is_err());
        assert!(evaluate(&"1".repeat(MAX_EXPRESSION + 1), Context::default()).is_err());
        assert!(evaluate(
            &format!("sum({})", vec!["1"; 65].join(",")),
            Context::default()
        )
        .is_err());
    }
    #[test]
    fn display_preserves_small_values() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(120.5), "120.5");
        assert!(format_number(1e-20).contains('e'));
        assert!(!format_number(1e20).is_empty());
    }
}
