use creamui_core::{CursorIcon, Painter, Point, Rect, Widget};
use creamui_widgets::{RawTextInput, TextSelection};
use regex::Regex;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub start: (usize, u16),
    pub end: (usize, u16),
}

#[derive(Default)]
pub struct Search {
    pub query: String,
    pub regex: bool,
    pub cursor: usize,
    pub selection: TextSelection,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    pub error: Option<String>,
    history: usize,
}

impl Search {
    pub fn clear(&mut self) {
        self.matches.clear();
        self.current = None;
        self.error = None;
    }
    /// Work on a snapshot so searching never changes the live viewport.
    /// Join soft-wrapped rows and retain byte-to-cell positions for Unicode.
    pub fn refresh(&mut self, screen: &vt100::Screen) {
        self.matches.clear();
        self.error = None;
        if self.query.is_empty() {
            self.current = None;
            return;
        }
        let pattern = if self.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        let regex = match Regex::new(&pattern) {
            Ok(regex) => regex,
            Err(error) => {
                self.error = Some(error.to_string());
                self.current = None;
                return;
            }
        };
        let mut snapshot = screen.clone();
        snapshot.set_scrollback(usize::MAX);
        self.history = snapshot.scrollback();
        let (rows, cols) = snapshot.size();
        let total = self.history + rows as usize;
        let mut text = String::new();
        let mut cells = Vec::new();
        let mut ends = Vec::new();
        let mut base = 0;
        while base < total {
            let top = base.min(self.history);
            snapshot.set_scrollback(self.history - top);
            let first = base - top;
            let count = (rows as usize - first).min(total - base);
            for row in first..first + count {
                let absolute = top + row;
                let wrapped = snapshot.row_wrapped(row as u16);
                let mut line = Vec::new();
                for col in 0..cols {
                    let cell = snapshot.cell(row as u16, col).unwrap();
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    let value = if cell.has_contents() {
                        cell.contents()
                    } else {
                        " "
                    };
                    line.push((col, value, if cell.is_wide() { 2 } else { 1 }));
                }
                if !wrapped {
                    while line.last().is_some_and(|(_, value, _)| *value == " ") {
                        line.pop();
                    }
                }
                for (col, value, width) in line {
                    text.push_str(value);
                    cells.extend(std::iter::repeat_n((absolute, col), value.len()));
                    ends.extend(std::iter::repeat_n((absolute, col + width), value.len()));
                }
                if !wrapped || absolute + 1 == total {
                    for found in regex.find_iter(&text).filter(|m| !m.is_empty()) {
                        self.matches.push(Match {
                            start: cells[found.start()],
                            end: ends[found.end() - 1],
                        });
                    }
                    text.clear();
                    cells.clear();
                    ends.clear();
                }
            }
            base += count;
        }
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(
                self.current
                    .unwrap_or(self.matches.len() - 1)
                    .min(self.matches.len() - 1),
            )
        };
    }

    pub fn navigate(&mut self, screen: &mut vt100::Screen, previous: bool) {
        if self.matches.is_empty() {
            return;
        }
        let n = self.matches.len();
        self.current = Some(match self.current {
            Some(i) if previous => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None if previous => n - 1,
            None => 0,
        });
        self.reveal(screen);
    }

    pub fn reveal(&self, screen: &mut vt100::Screen) {
        if let Some(found) = self.current.and_then(|i| self.matches.get(i)) {
            let top = found.start.0.saturating_sub(screen.size().0 as usize / 2);
            let offset = self.history.saturating_sub(top);
            screen.set_scrollback(offset);
        }
    }

    pub fn highlight(&self, screen: &vt100::Screen, row: u16, col: u16) -> Option<bool> {
        let position = (
            self.history.saturating_sub(screen.scrollback()) + row as usize,
            col,
        );
        // Matches are sorted; only candidates starting before this cell can contain it.
        let end = self.matches.partition_point(|m| m.start <= position);
        let index = end.checked_sub(1)?;
        (position < self.matches[index].end).then_some(self.current == Some(index))
    }
}

/// The terminal intercepts native keys, so its search editor always displays
/// its caret and receives editing keys directly, independently of button focus.
pub struct SearchInput(pub RawTextInput);
impl Widget for SearchInput {
    fn style(&self) -> creamui_core::Style {
        self.0.style()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.0.paint(painter, rect);
    }
    fn paint_content(&self, painter: &mut dyn Painter, rect: Rect, content: Rect) {
        self.0.paint_content(painter, rect, content);
        self.0
            .paint_focused_overlay_with_content(painter, rect, content, true);
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(creamui_core::KeyInput)>> {
        self.0.on_key()
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.0.on_drag_start()
    }
    fn on_drag_start_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.0.on_drag_start_with_content(content)
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.0.on_drag()
    }
    fn on_drag_with_content(&self, content: Rect) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.0.on_drag_with_content(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_entire_history_and_wraps_navigation_without_mutating_snapshot_source() {
        let mut parser = vt100::Parser::new(3, 20, 100);
        for i in 0..12 {
            parser.process(format!("entry {i}\r\n").as_bytes());
        }
        let mut search = Search {
            query: "entry".into(),
            ..Default::default()
        };
        search.refresh(parser.screen());
        assert_eq!(parser.screen().scrollback(), 0);
        assert_eq!(search.matches.len(), 12);
        search.navigate(parser.screen_mut(), false);
        assert_eq!(search.current, Some(0));
        assert!(parser.screen().scrollback() > 0);
        assert_eq!(search.highlight(parser.screen(), 0, 0), Some(true));
        search.navigate(parser.screen_mut(), true);
        assert_eq!(search.current, Some(11));
        assert!(parser.screen().contents().contains("entry 11"));
    }

    #[test]
    fn literal_regex_invalid_patterns_and_zero_width_matches() {
        let mut parser = vt100::Parser::new(4, 30, 100);
        parser.process(b"a.b aXb 123\r\n456");
        let mut search = Search {
            query: "a.b".into(),
            ..Default::default()
        };
        search.refresh(parser.screen());
        assert_eq!(search.matches.len(), 1);
        search.regex = true;
        search.refresh(parser.screen());
        assert_eq!(search.matches.len(), 2);
        search.query = "[".into();
        search.refresh(parser.screen());
        assert!(search.error.is_some());
        assert!(search.matches.is_empty());
        search.query = "^|$".into();
        search.refresh(parser.screen());
        assert!(search.error.is_none());
        assert!(search.matches.is_empty());
        search.query = r"\d+".into();
        search.refresh(parser.screen());
        assert_eq!(search.matches.len(), 2);
    }

    #[test]
    fn unicode_and_soft_wrapped_matches_map_to_terminal_cells() {
        let mut parser = vt100::Parser::new(3, 6, 100);
        parser.process("ab界e\u{301}XYZ".as_bytes());
        let mut search = Search {
            query: "界e\u{301}XY".into(),
            ..Default::default()
        };
        search.refresh(parser.screen());
        assert_eq!(
            search.matches,
            vec![Match {
                start: (0, 2),
                end: (1, 1)
            }]
        );
        assert_eq!(search.highlight(parser.screen(), 0, 3), Some(true));
        assert_eq!(search.highlight(parser.screen(), 1, 0), Some(true));
        assert_eq!(search.highlight(parser.screen(), 1, 1), None);
        search.clear();
        assert_eq!(search.highlight(parser.screen(), 0, 2), None);
    }

    #[test]
    fn alternate_screen_search_does_not_include_normal_history() {
        let mut parser = vt100::Parser::new(3, 10, 100);
        parser.process(b"normal\r\nnormal\r\nnormal\r\n\x1b[?1049hother");
        let mut search = Search {
            query: "normal".into(),
            ..Default::default()
        };
        search.refresh(parser.screen());
        assert!(search.matches.is_empty());
        search.query = "other".into();
        search.refresh(parser.screen());
        assert_eq!(search.matches.len(), 1);
    }
}
