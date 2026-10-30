use crate::document::MarkdownMode;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub source: Range<usize>,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: bool,
    pub strike: bool,
    pub muted: bool,
}
pub struct Line {
    pub spans: Vec<Span>,
    pub scale: f32,
    pub quote: bool,
    pub rule: bool,
}
pub fn line(source: &str, mode: MarkdownMode) -> Line {
    let trimmed = source.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    let heading = (1..=6).contains(&level) && trimmed.as_bytes().get(level) == Some(&b' ');
    let scale = if mode != MarkdownMode::Raw && heading {
        [1.7, 1.5, 1.3, 1.15, 1.05, 1.0][level - 1]
    } else {
        1.0
    };
    let quote = mode != MarkdownMode::Raw && trimmed.starts_with('>');
    let rule = mode == MarkdownMode::Preview && ["---", "***", "___"].contains(&trimmed.trim());
    if mode == MarkdownMode::Raw {
        return Line {
            spans: vec![plain(source, 0..source.len())],
            scale,
            quote,
            rule,
        };
    }
    let mut spans = Vec::new();
    let (mut bold, mut italic, mut strike, mut link) = (0usize, 0usize, 0usize, 0usize);
    let mut styled = Vec::new();
    for (event, range) in Parser::new_ext(
        source,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES,
    )
    .into_offset_iter()
    {
        match event {
            Event::Start(Tag::Strong) => bold += 1,
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::Start(Tag::Link { .. }) => link += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::End(TagEnd::Strikethrough) => strike = strike.saturating_sub(1),
            Event::End(TagEnd::Link) => link = link.saturating_sub(1),
            Event::Text(text) | Event::Code(text) => {
                let code = source
                    .get(range.clone())
                    .is_some_and(|s| s.starts_with('`'));
                let mut span = plain(&text, range);
                span.bold = bold > 0 || heading;
                span.italic = italic > 0;
                span.strike = strike > 0;
                span.link = link > 0;
                span.code = code;
                styled.push(span);
            }
            Event::TaskListMarker(checked) if mode == MarkdownMode::Preview => {
                styled.push(plain(if checked { "[x] " } else { "[ ] " }, range))
            }
            _ => {}
        }
    }
    if mode == MarkdownMode::Preview {
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            spans.push(plain("• ", 0..0));
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            spans.push(Span {
                muted: true,
                ..plain(trimmed, 0..source.len())
            });
        }
        spans.extend(styled);
    } else {
        let mut previous = 0;
        for mut span in styled {
            if span.source.start < previous {
                continue;
            }
            if span.source.start > previous {
                spans.push(Span {
                    muted: true,
                    ..plain(
                        &source[previous..span.source.start],
                        previous..span.source.start,
                    )
                });
            }
            span.text = source[span.source.clone()].into();
            previous = span.source.end;
            spans.push(span);
        }
        if previous < source.len() {
            spans.push(Span {
                muted: true,
                ..plain(&source[previous..], previous..source.len())
            });
        }
    }
    Line {
        spans,
        scale,
        quote,
        rule,
    }
}
fn plain(text: &str, source: Range<usize>) -> Span {
    Span {
        text: text.into(),
        source,
        bold: false,
        italic: false,
        code: false,
        link: false,
        strike: false,
        muted: false,
    }
}
#[derive(Default)]
pub struct BlockIndex {
    code: Vec<Range<usize>>,
}
impl BlockIndex {
    pub fn build(text: &ropey::Rope) -> Self {
        let mut code = Vec::new();
        let mut opening: Option<(char, usize, usize)> = None;
        let mut offset = 0;
        for line in text.lines() {
            let mut chars = line.chars().peekable();
            let spaces = chars.by_ref().take_while(|c| *c == ' ').count();
            if spaces <= 3 {
                let marker = line.get_char(spaces);
                if let Some(marker @ ('`' | '~')) = marker {
                    let count = line.chars_at(spaces).take_while(|c| *c == marker).count();
                    if count >= 3 {
                        if let Some((current, width, start)) = opening {
                            if marker == current
                                && count >= width
                                && line.chars_at(spaces + count).all(char::is_whitespace)
                            {
                                code.push(start..offset + line.len_chars());
                                opening = None;
                            }
                        } else {
                            opening = Some((marker, count, offset));
                        }
                    }
                }
            }
            offset += line.len_chars();
        }
        if let Some((_, _, start)) = opening {
            code.push(start..text.len_chars());
        }
        Self { code }
    }
    pub fn code_at(&self, offset: usize) -> bool {
        let index = self.code.partition_point(|range| range.end <= offset);
        self.code
            .get(index)
            .is_some_and(|range| range.contains(&offset))
    }
}
pub fn code_line(source: &str, mode: MarkdownMode) -> Line {
    let fence = source.trim_start().starts_with("```") || source.trim_start().starts_with("~~~");
    let span = Span {
        code: true,
        muted: fence,
        ..plain(
            if fence && mode == MarkdownMode::Preview {
                ""
            } else {
                source
            },
            0..source.len(),
        )
    };
    Line {
        spans: vec![span],
        scale: 1.0,
        quote: false,
        rule: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hybrid_keeps_source_and_preview_formats_it() {
        let source = "# Hello **world** and `code`";
        let hybrid = line(source, MarkdownMode::Hybrid);
        assert!(hybrid.scale > 1.0);
        assert_eq!(
            hybrid
                .spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>(),
            source
        );
        let preview = line(source, MarkdownMode::Preview);
        assert_eq!(
            preview
                .spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>(),
            "Hello world and code"
        );
        assert!(preview.spans.iter().any(|s| s.bold));
        assert!(preview.spans.iter().any(|s| s.code));
    }
}
