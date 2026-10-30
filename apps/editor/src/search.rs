use regex::RegexBuilder;
use ropey::Rope;
use std::ops::Range;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}
pub fn find(
    text: &Rope,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<Range<usize>>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if options.case_sensitive && !options.regex && !options.whole_word {
        return Ok(literal(text, query));
    }
    let expression = expression(query, options)?;
    let source = text.to_string();
    Ok(expression
        .find_iter(&source)
        .map(|m| text.byte_to_char(m.start())..text.byte_to_char(m.end()))
        .collect())
}
fn expression(query: &str, options: &SearchOptions) -> Result<regex::Regex, String> {
    let pattern = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    let pattern = if options.whole_word {
        format!(r"\b(?:{pattern})\b")
    } else {
        pattern
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|e| e.to_string())
}
fn literal(text: &Rope, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().collect();
    let mut prefix = vec![0; needle.len()];
    let mut matched = 0;
    for i in 1..needle.len() {
        while matched > 0 && needle[i] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[i] == needle[matched] {
            matched += 1;
        }
        prefix[i] = matched;
    }
    let mut result = Vec::new();
    matched = 0;
    for (i, c) in text.chars().enumerate() {
        while matched > 0 && c != needle[matched] {
            matched = prefix[matched - 1];
        }
        if c == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            result.push(i + 1 - matched..i + 1);
            matched = 0;
        }
    }
    result
}
pub fn replace_all(
    text: &Rope,
    query: &str,
    replacement: &str,
    options: &SearchOptions,
) -> Result<(Rope, usize), String> {
    if query.is_empty() {
        return Ok((text.clone(), 0));
    }
    if options.regex {
        let expression = expression(query, options)?;
        let source = text.to_string();
        let mut builder = ropey::RopeBuilder::new();
        let mut previous = 0;
        let mut count = 0;
        let mut expanded = String::new();
        for captures in expression.captures_iter(&source) {
            let matched = captures.get(0).expect("full regex capture");
            builder.append(&source[previous..matched.start()]);
            expanded.clear();
            captures.expand(replacement, &mut expanded);
            builder.append(&expanded);
            previous = matched.end();
            count += 1;
        }
        builder.append(&source[previous..]);
        return Ok((builder.finish(), count));
    }
    let matches = find(text, query, options)?;
    let count = matches.len();
    let mut builder = ropey::RopeBuilder::new();
    let mut previous = 0;
    for range in matches {
        for chunk in text.slice(previous..range.start).chunks() {
            builder.append(chunk);
        }
        builder.append(replacement);
        previous = range.end;
    }
    for chunk in text.slice(previous..).chunks() {
        builder.append(chunk);
    }
    Ok((builder.finish(), count))
}
pub fn replace_match(
    text: &Rope,
    range: Range<usize>,
    query: &str,
    replacement: &str,
    options: &SearchOptions,
) -> Result<Rope, String> {
    let mut expanded = String::new();
    let value = if options.regex {
        let expression = expression(query, options)?;
        let source = text.to_string();
        let start = text.char_to_byte(range.start);
        let end = text.char_to_byte(range.end);
        let captures = expression
            .captures_at(&source, start)
            .filter(|captures| {
                captures
                    .get(0)
                    .is_some_and(|matched| matched.start() == start && matched.end() == end)
            })
            .ok_or_else(|| "The selected match is stale. Search again.".to_string())?;
        captures.expand(replacement, &mut expanded);
        expanded.as_str()
    } else {
        replacement
    };
    let mut result = text.clone();
    result.remove(range.clone());
    result.insert(range.start, value);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunk_boundaries_and_unicode_offsets() {
        let source = format!("{}é😀needle\nneedle", "a".repeat(2047));
        let rope = Rope::from_str(&source);
        let matches = find(
            &rope,
            "😀needle",
            &SearchOptions {
                case_sensitive: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(matches, vec![2048..2055]);
        let (text, count) = replace_all(&rope, "needle", "x", &SearchOptions::default()).unwrap();
        assert_eq!(count, 2);
        assert!(text.to_string().ends_with("😀x\nx"));
    }
    #[test]
    fn regex_groups_and_zero_length_matches() {
        let (text, count) = replace_all(
            &Rope::from_str("a1 a2"),
            r"a(\d)",
            "$1!",
            &SearchOptions {
                regex: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(text.to_string(), "1! 2!");
        assert_eq!(count, 2);
        assert_eq!(
            find(
                &Rope::from_str("ab"),
                "^",
                &SearchOptions {
                    regex: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            vec![0..0]
        );
    }
    #[test]
    fn single_regex_replacement_keeps_surrounding_context() {
        let text = Rope::from_str("xword word");
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let output = replace_match(&text, 6..10, r"\b(word)\b", "$1!", &options).unwrap();
        assert_eq!(output.to_string(), "xword word!");
        assert!(replace_match(&text, 1..5, r"\bword\b", "x", &options).is_err());
    }
}
