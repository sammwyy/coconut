use crate::document::{Document, MarkdownMode, Snapshot};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct History {
    snapshot: Snapshot,
    cursor: usize,
    anchor: usize,
}
pub struct Editor {
    pub document: Document,
    pub cursor: usize,
    pub anchor: usize,
    pub top: usize,
    pub left: usize,
    pub font_size: f32,
    pub line_numbers: bool,
    pub overwrite: bool,
    pub tab_width: usize,
    pub insert_spaces: bool,
    pub rows: usize,
    pub columns: usize,
    preferred_column: Option<usize>,
    undo: Vec<History>,
    redo: Vec<History>,
    typing: Option<(Instant, usize, u64)>,
}
impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            cursor: 0,
            anchor: 0,
            top: 0,
            left: 0,
            font_size: 15.0,
            line_numbers: true,
            overwrite: false,
            tab_width: 4,
            insert_spaces: true,
            rows: 30,
            columns: 100,
            preferred_column: None,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: None,
        }
    }
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
    pub fn selected_text(&self) -> String {
        self.document.text.slice(self.selection()).to_string()
    }
    fn history(&self) -> History {
        History {
            snapshot: self.document.snapshot(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }
    pub fn checkpoint(&mut self) {
        self.typing = None;
        self.undo.push(self.history());
        if self.undo.len() > 512 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self) {
        self.typing = None;
        if let Some(history) = self.undo.pop() {
            self.redo.push(self.history());
            self.restore(history);
        }
    }
    pub fn redo(&mut self) {
        self.typing = None;
        if let Some(history) = self.redo.pop() {
            self.undo.push(self.history());
            self.restore(history);
        }
    }
    fn restore(&mut self, history: History) {
        self.document.restore(history.snapshot);
        self.cursor = history.cursor;
        self.anchor = history.anchor;
        self.reveal();
    }
    pub fn editable(&self) -> bool {
        self.document.markdown_mode() != MarkdownMode::Preview || self.document.view.bytes()
    }
    pub fn insert(&mut self, text: &str) -> Result<(), String> {
        if !self.editable() {
            return Err("Switch to Raw or Hybrid to edit Markdown.".into());
        }
        let mut range = self.selection();
        if self.overwrite && range.is_empty() && !text.contains('\n') {
            range.end = (range.end + text.chars().count()).min(if self.document.view.bytes() {
                self.document.text.len_chars()
            } else {
                self.document.line_end(self.line())
            });
        }
        self.replace(range, text)
    }
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        if !self.editable() {
            return Err("Preview is read-only.".into());
        }
        let now = Instant::now();
        let can_merge = text.chars().count() == 1
            && self.selection().is_empty()
            && !self.overwrite
            && self.typing.is_some_and(|(time, cursor, revision)| {
                now.duration_since(time) < Duration::from_millis(750)
                    && cursor == self.cursor
                    && revision == self.document.revision
                    && self.document.saved_revision != revision
            });
        if can_merge {
            self.document.replace(self.cursor..self.cursor, text)?;
            self.cursor += text.chars().count();
            self.anchor = self.cursor;
            self.reveal();
        } else {
            self.insert(text)?;
        }
        self.typing = Some((now, self.cursor, self.document.revision));
        Ok(())
    }
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Result<(), String> {
        if !self.editable() {
            return Err("Preview is read-only.".into());
        }
        let mut document = self.document.clone();
        document.replace(range.clone(), text)?;
        self.checkpoint();
        self.document = document;
        self.cursor = range.start + text.chars().count();
        self.anchor = self.cursor;
        self.preferred_column = None;
        self.reveal();
        Ok(())
    }
    pub fn line(&self) -> usize {
        if self.document.view.bytes() {
            self.cursor / 16
        } else {
            self.document.text.char_to_line(self.cursor)
        }
    }
    pub fn column(&self) -> usize {
        if self.document.view.bytes() {
            self.cursor % 16
        } else {
            self.cursor - self.document.text.line_to_char(self.line())
        }
    }
    pub fn line_count(&self) -> usize {
        if self.document.view.bytes() {
            self.document.text.len_chars() / 16 + 1
        } else {
            self.document.text.len_lines()
        }
    }
    pub fn select(&mut self, range: Range<usize>) {
        self.anchor = range.start;
        self.cursor = range.end;
        self.reveal();
    }
    pub fn move_to(&mut self, target: usize, extend: bool) {
        self.typing = None;
        self.cursor = target.min(self.document.text.len_chars());
        if !extend {
            self.anchor = self.cursor;
        }
        self.preferred_column = None;
        self.reveal();
    }
    pub fn horizontal(&mut self, right: bool, word: bool, extend: bool) {
        if !extend && !self.selection().is_empty() {
            self.move_to(
                if right {
                    self.selection().end
                } else {
                    self.selection().start
                },
                false,
            );
            return;
        }
        let mut target = if right {
            (self.cursor + 1).min(self.document.text.len_chars())
        } else {
            self.cursor.saturating_sub(1)
        };
        if word && !self.document.view.bytes() {
            let len = self.document.text.len_chars();
            if right && target < len {
                let category = word_char(self.document.text.char(target - 1));
                for c in self.document.text.chars_at(target) {
                    if word_char(c) != category {
                        break;
                    }
                    target += 1;
                }
            } else if !right && target > 0 {
                let category = word_char(self.document.text.char(target));
                for c in self.document.text.chars_at(target).reversed() {
                    if word_char(c) != category {
                        break;
                    }
                    target -= 1;
                }
            }
        }
        if !self.document.view.bytes()
            && !word
            && target > 0
            && target < self.document.text.len_chars()
            && self.document.text.char(target - 1) == '\r'
            && self.document.text.char(target) == '\n'
        {
            if right {
                target += 1;
            } else {
                target -= 1;
            }
        }
        self.move_to(target, extend);
    }
    pub fn vertical(&mut self, delta: isize, extend: bool) {
        let column = self.preferred_column.unwrap_or(self.column());
        let line = self
            .line()
            .saturating_add_signed(delta)
            .min(self.line_count() - 1);
        let target = if self.document.view.bytes() {
            (line * 16 + column).min(self.document.text.len_chars())
        } else {
            (self.document.text.line_to_char(line) + column).min(self.document.line_end(line))
        };
        self.move_to(target, extend);
        self.preferred_column = Some(column);
    }
    pub fn home_end(&mut self, end: bool, whole: bool, extend: bool) {
        let target = if whole {
            if end {
                self.document.text.len_chars()
            } else {
                0
            }
        } else if self.document.view.bytes() {
            if end {
                (self.line() * 16 + 16).min(self.document.text.len_chars())
            } else {
                self.line() * 16
            }
        } else if end {
            self.document.line_end(self.line())
        } else {
            self.document.text.line_to_char(self.line())
        };
        self.move_to(target, extend);
    }
    pub fn delete(&mut self, backwards: bool, word: bool) -> Result<(), String> {
        if self.selection().is_empty() {
            let anchor = self.cursor;
            self.horizontal(!backwards, word, true);
            self.anchor = anchor;
            if !self.document.view.bytes() {
                let range = self.selection();
                if backwards
                    && range.start > 0
                    && self.document.text.get_char(range.start) == Some('\n')
                    && self.document.text.char(range.start - 1) == '\r'
                {
                    self.cursor = range.start - 1;
                }
                if !backwards
                    && self.document.text.get_char(range.start) == Some('\r')
                    && self.document.text.get_char(range.end) == Some('\n')
                {
                    self.cursor = range.end + 1;
                }
            }
        }
        if self.selection().is_empty() {
            return Ok(());
        }
        self.insert("")
    }
    pub fn newline(&mut self) -> Result<(), String> {
        if self.document.view.bytes() {
            return Ok(());
        }
        let start = self.document.text.line_to_char(self.line());
        let indent: String = self
            .document
            .text
            .slice(start..self.cursor)
            .chars()
            .take_while(|c| matches!(c, ' ' | '\t'))
            .collect();
        self.insert(&format!("{}{indent}", self.document.newline))
    }
    pub fn indent(&mut self, outdent: bool) -> Result<(), String> {
        if !self.editable() {
            return Err("Preview is read-only.".into());
        }
        if self.document.view.bytes() {
            return Ok(());
        }
        if self.selection().is_empty() && !outdent {
            return self.insert(&if self.insert_spaces {
                " ".repeat(self.tab_width - self.column() % self.tab_width)
            } else {
                "\t".into()
            });
        }
        let range = self.selected_lines();
        let first = self.document.text.char_to_line(range.start);
        let last = self.document.text.char_to_line(range.end.saturating_sub(1));
        self.checkpoint();
        let mut failure = None;
        for line in (first..=last).rev() {
            let start = self.document.text.line_to_char(line);
            let result = if outdent {
                let count = if self.document.text.get_char(start) == Some('\t') {
                    1
                } else {
                    self.document
                        .text
                        .line(line)
                        .chars()
                        .take(self.tab_width)
                        .take_while(|c| *c == ' ')
                        .count()
                };
                self.document.replace(start..start + count, "")
            } else {
                self.document.replace(
                    start..start,
                    &if self.insert_spaces {
                        " ".repeat(self.tab_width)
                    } else {
                        "\t".into()
                    },
                )
            };
            if let Err(error) = result {
                failure = Some(error);
                break;
            }
        }
        self.anchor = self.document.text.line_to_char(first);
        self.cursor = if last + 1 < self.document.text.len_lines() {
            self.document.text.line_to_char(last + 1)
        } else {
            self.document.text.len_chars()
        };
        self.reveal();
        failure.map_or(Ok(()), Err)
    }
    pub fn selected_lines(&self) -> Range<usize> {
        let selection = self.selection();
        if self.document.view.bytes() {
            let last = if selection.is_empty() {
                selection.end
            } else {
                selection.end - 1
            };
            return selection.start / 16 * 16
                ..((last / 16 + 1) * 16).min(self.document.text.len_chars());
        }
        let first = self.document.text.char_to_line(selection.start);
        let last = self.document.text.char_to_line(if selection.is_empty() {
            selection.end
        } else {
            selection.end.saturating_sub(1)
        });
        self.document.text.line_to_char(first)..if last + 1 < self.document.text.len_lines() {
            self.document.text.line_to_char(last + 1)
        } else {
            self.document.text.len_chars()
        }
    }
    pub fn duplicate_line(&mut self) -> Result<(), String> {
        let range = self.selected_lines();
        let mut text = self.document.text.slice(range.clone()).to_string();
        if !self.document.view.bytes() && !text.ends_with('\n') {
            text.insert_str(0, self.document.newline);
        }
        self.replace(range.end..range.end, &text)
    }
    pub fn delete_line(&mut self) -> Result<(), String> {
        let range = self.selected_lines();
        self.replace(range, "")
    }
    pub fn move_line(&mut self, down: bool) -> Result<(), String> {
        let range = self.selected_lines();
        let len = self.document.text.len_chars();
        if (!down && range.start == 0) || (down && range.end == len) {
            return Ok(());
        }
        let other = if self.document.view.bytes() {
            if down {
                range.end..(range.end + 16).min(len)
            } else {
                range.start.saturating_sub(16)..range.start
            }
        } else if down {
            range.end
                ..(range.end
                    + self
                        .document
                        .text
                        .line(self.document.text.char_to_line(range.end))
                        .len_chars())
        } else {
            self.document
                .text
                .line_to_char(self.document.text.char_to_line(range.start) - 1)
                ..range.start
        };
        let a = self.document.text.slice(range.clone()).to_string();
        let b = self.document.text.slice(other.clone()).to_string();
        let (mut first, mut second) = if down { (b, a) } else { (a, b) };
        if !self.document.view.bytes() && !first.ends_with('\n') {
            first.push_str(self.document.newline);
            second = second.trim_end_matches(['\r', '\n']).to_string();
        }
        let combined = format!("{first}{second}");
        self.replace(
            range.start.min(other.start)..range.end.max(other.end),
            &combined,
        )
    }
    pub fn wrap_selection(&mut self, marker: &str) -> Result<(), String> {
        let text = self.selected_text();
        self.insert(&format!("{marker}{text}{marker}"))?;
        self.cursor = self.cursor.saturating_sub(marker.chars().count());
        self.anchor = self.cursor.saturating_sub(text.chars().count());
        Ok(())
    }
    pub fn reveal(&mut self) {
        let line = self.line();
        let column = self.column();
        if line < self.top {
            self.top = line;
        } else if line >= self.top + self.rows.max(1) {
            self.top = line + 1 - self.rows.max(1);
        }
        if column < self.left {
            self.left = column;
        } else if column >= self.left + self.columns.max(1) {
            self.left = column + 1 - self.columns.max(1);
        }
    }
}
fn word_char(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_tracks_saved_revisions_and_branches() {
        let mut editor = Editor::new(Document::default());
        editor.insert("hello").unwrap();
        editor.document.saved_revision = editor.document.revision;
        editor.insert(" world").unwrap();
        editor.undo();
        assert!(!editor.document.dirty());
        editor.redo();
        assert!(editor.document.dirty());
        editor.undo();
        editor.insert("!").unwrap();
        editor.redo();
        assert_eq!(editor.document.text.to_string(), "hello!");
    }
    #[test]
    fn unicode_selection_and_crlf_delete() {
        let mut editor = Editor::new(Document::default());
        editor.insert("é😀\r\nx").unwrap();
        editor.move_to(4, false);
        editor.delete(true, false).unwrap();
        assert_eq!(editor.document.text.to_string(), "é😀x");
        editor.undo();
        editor.select(0..2);
        editor.insert("a").unwrap();
        assert_eq!(editor.document.text.to_string(), "a\r\nx");
    }
    #[test]
    fn indent_is_one_undo_step() {
        let mut editor = Editor::new(Document::default());
        editor.insert("one\ntwo\n").unwrap();
        editor.select(0..8);
        editor.indent(false).unwrap();
        assert_eq!(editor.document.text.to_string(), "    one\n    two\n");
        editor.undo();
        assert_eq!(editor.document.text.to_string(), "one\ntwo\n");
    }
    #[test]
    fn million_lines_and_a_million_character_line() {
        let document = Document {
            text: ropey::Rope::from_str(&"x\n".repeat(1_000_000)),
            ..Document::default()
        };
        let mut editor = Editor::new(document);
        editor.move_to(1_500_000, false);
        editor.insert("é").unwrap();
        assert_eq!(editor.line(), 750_000);
        assert_eq!(editor.document.text.len_lines(), 1_000_001);
        editor.undo();
        editor.document.text = ropey::Rope::from_str(&"a".repeat(1_000_000));
        editor.move_to(999_999, false);
        editor.insert("😀").unwrap();
        assert_eq!(editor.document.text.len_chars(), 1_000_001);
    }
    #[test]
    fn typing_groups_stop_at_navigation_and_save_boundaries() {
        let mut e = Editor::new(Document::default());
        e.type_text("a").unwrap();
        e.type_text("b").unwrap();
        e.undo();
        assert_eq!(e.document.text.to_string(), "");
        e.redo();
        e.document.saved_revision = e.document.revision;
        e.type_text("c").unwrap();
        e.undo();
        assert_eq!(e.document.text.to_string(), "ab");
        assert!(!e.document.dirty());
        e.move_to(0, false);
        e.type_text("x").unwrap();
        e.undo();
        assert_eq!(e.document.text.to_string(), "ab");
    }
    #[test]
    fn line_moves_keep_the_unterminated_final_line() {
        for (position, down) in [(0, true), (2, false)] {
            let mut e = Editor::new(Document::default());
            e.insert("a\nb").unwrap();
            e.move_to(position, false);
            e.move_line(down).unwrap();
            assert_eq!(e.document.text.to_string(), "b\na");
            e.undo();
            assert_eq!(e.document.text.to_string(), "a\nb");
        }
    }
    #[test]
    fn preview_rejects_edits_and_crlf_is_one_navigation_step() {
        let mut e = Editor::new(Document::default());
        e.insert("a\r\nb").unwrap();
        e.move_to(1, false);
        e.horizontal(true, false, false);
        assert_eq!(e.cursor, 3);
        e.horizontal(false, false, false);
        assert_eq!(e.cursor, 1);
        e.document.language = crate::document::Language::Markdown;
        e.document.markdown = MarkdownMode::Preview;
        assert!(e.indent(false).is_err());
        assert!(e.insert("x").is_err());
    }
    #[test]
    fn byte_line_commands_use_fixed_width_rows() {
        let mut e = Editor::new(Document::default());
        e.insert(&"a".repeat(32)).unwrap();
        e.document.set_view(crate::document::ViewMode::Hex).unwrap();
        e.move_to(0, false);
        e.delete_line().unwrap();
        assert_eq!(e.document.text.len_chars(), 16);
        e.duplicate_line().unwrap();
        assert_eq!(e.document.text.len_chars(), 32);
    }
}
