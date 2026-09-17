/// A parsed [`coconut_core::SectionConfig::gap`]: spacing between the
/// islands inside a dock section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gap {
    /// A fixed length in logical pixels, e.g. `"10px"`.
    Fixed(f32),
    /// A fraction (0.0-1.0) of the section's length, e.g. `"30%"` -> `0.3`.
    Percent(f32),
    /// CSS-flexbox-style `justify-content: space-between`.
    Between,
    /// CSS-flexbox-style `justify-content: space-evenly`.
    Evenly,
}

/// Parses a `SectionConfig::gap` string. Accepts a fixed length (`"10px"`),
/// a percentage (`"30%"`), or the keywords `"between"`/`"evenly"`. Invalid
/// input logs a warning and falls back to `Gap::Fixed(10.0)` — the same
/// house style as `ShellConfig::load`'s own fallback-on-parse-error
/// (`crates/core/src/lib.rs`) and `warn_unknown_islands`: never panic or
/// silently misbehave on a malformed `shell.toml`, warn and use a sane
/// default instead.
pub fn parse_gap(gap: &str) -> Gap {
    let trimmed = gap.trim();
    if trimmed.eq_ignore_ascii_case("between") {
        return Gap::Between;
    }
    if trimmed.eq_ignore_ascii_case("evenly") {
        return Gap::Evenly;
    }
    if let Some(percent) = trimmed.strip_suffix('%') {
        if let Ok(value) = percent.trim().parse::<f32>() {
            return Gap::Percent(value / 100.0);
        }
    } else if let Some(pixels) = trimmed.strip_suffix("px") {
        if let Ok(value) = pixels.trim().parse::<f32>() {
            return Gap::Fixed(value);
        }
    }
    eprintln!("plugin-kit: invalid gap '{gap}' in shell.toml; falling back to 10px");
    Gap::Fixed(10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixed_pixels() {
        assert_eq!(parse_gap("10px"), Gap::Fixed(10.0));
    }

    #[test]
    fn parses_percent() {
        assert_eq!(parse_gap("30%"), Gap::Percent(0.3));
    }

    #[test]
    fn parses_between_keyword() {
        assert_eq!(parse_gap("between"), Gap::Between);
    }

    #[test]
    fn parses_evenly_keyword() {
        assert_eq!(parse_gap("evenly"), Gap::Evenly);
    }

    #[test]
    fn invalid_input_falls_back_to_fixed_ten() {
        assert_eq!(parse_gap("not a gap"), Gap::Fixed(10.0));
        assert_eq!(parse_gap(""), Gap::Fixed(10.0));
        assert_eq!(parse_gap("10"), Gap::Fixed(10.0));
    }
}
