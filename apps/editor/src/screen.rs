use crate::app::Tab;
use crate::{
    document::{MarkdownMode, ViewMode},
    editor::Editor,
    markdown,
};
use creamui_core::{layout, CursorIcon, Painter, Point, Rect, TextAlign, Widget};
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use std::cell::{Cell, RefCell};
use std::{ops::Range, rc::Rc};
use unicode_width::UnicodeWidthChar;

const FONT: &str = "DejaVu Sans Mono, Liberation Mono, Cascadia Mono, Consolas, monospace";
const INSET: f32 = 12.0;
pub struct Highlights {
    pub tab: u64,
    pub revision: u64,
    pub ranges: Rc<Vec<Range<usize>>>,
}
pub struct EditorScreen {
    pub revision: Signal<u64>,
    pub focused: Rc<Cell<bool>>,
    pub matches: Rc<RefCell<Option<Highlights>>>,
    pub on_change: Rc<dyn Fn()>,
    pub drag_axis: Rc<Cell<u8>>,
    pub tab: Tab,
    pub id: u64,
}
#[derive(Clone, Copy)]
struct Metrics {
    cell: f32,
    height: f32,
    gutter: f32,
    font: f32,
}
impl Metrics {
    fn new(editor: &Editor) -> Self {
        let face = creamui_fonts::resolve(FONT, creamui_fonts::FontWeight::Regular);
        let shaped = creamui_fonts::cached_layout(
            &face,
            "M",
            editor.font_size,
            &creamui_fonts::LayoutSettings::default(),
        );
        let cell = shaped
            .chars
            .first()
            .map(|c| c.advance)
            .unwrap_or(editor.font_size * 0.6);
        let gutter = if editor.line_numbers {
            (if editor.document.view.bytes() {
                10.0
            } else {
                editor.line_count().to_string().len() as f32 + 2.0
            }) * cell
                + INSET
        } else {
            INSET
        };
        Self {
            cell,
            height: (editor.font_size
                * if editor.document.markdown_mode() == MarkdownMode::Raw
                    || editor.document.view.bytes()
                {
                    1.65
                } else {
                    2.1
                })
            .ceil(),
            gutter,
            font: editor.font_size,
        }
    }
}
impl EditorScreen {
    fn horizontal_extent(editor: &Editor) -> usize {
        if editor.document.view.bytes() {
            return 16;
        }
        (editor.top..(editor.top + editor.rows).min(editor.line_count()))
            .map(|line| editor.document.line_end(line) - editor.document.text.line_to_char(line))
            .max()
            .unwrap_or(0)
    }
    fn target(editor: &Editor, p: Point, metrics: Metrics) -> usize {
        let row =
            (editor.top + (p.y / metrics.height).max(0.0) as usize).min(editor.line_count() - 1);
        if editor.document.view.bytes() {
            let group = if editor.document.view == ViewMode::Hex {
                3.0
            } else {
                9.0
            };
            return (row * 16
                + editor.left
                + ((p.x - metrics.gutter - INSET) / (metrics.cell * group))
                    .max(0.0)
                    .round() as usize)
                .min((row * 16 + 16).min(editor.document.text.len_chars()));
        }
        let start = editor.document.text.line_to_char(row);
        let end = editor.document.line_end(row);
        let left = editor.left.min(end - start);
        let source = editor
            .document
            .text
            .slice(start + left..(start + left + editor.columns + 8).min(end))
            .to_string();
        let line = if editor.document.markdown_mode() != MarkdownMode::Raw
            && editor.document.markdown_index.code_at(start)
        {
            markdown::code_line(&source, editor.document.markdown_mode())
        } else {
            markdown::line(&source, editor.document.markdown_mode())
        };
        let mut x = metrics.gutter + INSET;
        let scale = line.scale;
        let mut position = start + left;
        for c in source.chars() {
            let width =
                advance(c, position - start, editor.tab_width) as f32 * metrics.cell * scale;
            if p.x < x + width / 2.0 {
                return position;
            }
            x += width;
            position += 1;
        }
        position
    }
}
impl Widget for EditorScreen {
    fn style(&self) -> creamui_core::Style {
        layout::Style {
            flex_grow: 1.0,
            flex_basis: layout::Dimension::Length(0.0),
            min_size: layout::Size {
                width: layout::Dimension::Length(0.0),
                height: layout::Dimension::Length(0.0),
            },
            ..Default::default()
        }
        .into()
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(creamui_core::KeyInput)>> {
        let focused = self.focused.clone();
        Some(Rc::new(move |_| focused.set(true)))
    }
    fn accepts_text_input(&self) -> bool {
        true
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }
    fn reports_layout(&self) -> bool {
        true
    }
    fn on_layout(&self, rect: Rect) -> bool {
        let mut editor = self.tab.borrow_mut();
        let metrics = Metrics::new(&editor);
        editor.rows = (rect.height / metrics.height).floor().max(1.0) as usize;
        let group = if editor.document.view == ViewMode::Hex {
            3.0
        } else if editor.document.view == ViewMode::Binary {
            9.0
        } else if editor.document.markdown_mode() == MarkdownMode::Hybrid {
            1.7
        } else {
            1.0
        };
        editor.columns = ((rect.width - metrics.gutter - INSET * 2.0 - 12.0)
            / (metrics.cell * group))
            .floor()
            .max(1.0) as usize;
        false
    }
    fn on_focus_change(&self) -> Option<Rc<dyn Fn(bool)>> {
        let focused = self.focused.clone();
        Some(Rc::new(move |value| focused.set(value)))
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let _ = self.revision.get();
        let editor = self.tab.borrow();
        let metrics = Metrics::new(&editor);
        let colors = use_theme().colors;
        painter.push_clip(rect);
        painter.fill_rect(rect, colors.surface, 0.0);
        let selection = editor.selection();
        let cursor_line = editor.line();
        let matches = self.matches.borrow();
        let matches = matches
            .as_ref()
            .filter(|m| m.tab == self.id && m.revision == editor.document.revision);
        for visible in 0..(rect.height / metrics.height).ceil() as usize {
            let row = editor.top + visible;
            if row >= editor.line_count() {
                break;
            }
            let y = rect.y + visible as f32 * metrics.height;
            let line_rect = Rect {
                x: rect.x,
                y,
                width: rect.width,
                height: metrics.height,
            };
            if row == cursor_line && editor.document.markdown_mode() != MarkdownMode::Preview {
                painter.fill_rect(line_rect, colors.surface_hover, 0.0);
            }
            if editor.line_numbers {
                let number = if editor.document.view.bytes() {
                    format!("{:08X}", row * 16)
                } else {
                    (row + 1).to_string()
                };
                painter.fill_text_font(
                    Rect {
                        x: rect.x + 4.0,
                        y,
                        width: metrics.gutter - INSET,
                        height: metrics.height,
                    },
                    &number,
                    if row == cursor_line {
                        colors.accent
                    } else {
                        colors.text_secondary
                    },
                    metrics.font * 0.82,
                    TextAlign::End,
                    Some(FONT),
                    false,
                    false,
                );
            }
            if editor.document.view.bytes() {
                let begin = row * 16;
                let end = (begin + 16).min(editor.document.text.len_chars());
                let group = if editor.document.view == ViewMode::Hex {
                    3.0
                } else {
                    9.0
                };
                let mut x = rect.x + metrics.gutter + INSET;
                for offset in editor.left..16 {
                    let index = begin + offset;
                    if index > end {
                        break;
                    }
                    let cell_rect = Rect {
                        x,
                        y,
                        width: metrics.cell * group,
                        height: metrics.height,
                    };
                    if selection.contains(&index) {
                        painter.fill_rect(cell_rect, colors.accent.mix(colors.surface, 0.7), 2.0);
                    } else if matches.is_some_and(|m| contains(&m.ranges, index)) {
                        painter.fill_rect(cell_rect, colors.accent.mix(colors.surface, 0.85), 2.0);
                    }
                    if index < end {
                        let byte = editor.document.text.char(index) as u8;
                        let text = if editor.document.view == ViewMode::Hex {
                            format!("{byte:02X}")
                        } else {
                            format!("{byte:08b}")
                        };
                        painter.fill_text_font(
                            cell_rect,
                            &text,
                            colors.text_primary,
                            metrics.font,
                            TextAlign::Start,
                            Some(FONT),
                            false,
                            false,
                        );
                    }
                    if editor.cursor == index && self.focused.get() {
                        painter.fill_rect(
                            Rect {
                                width: 2.0,
                                ..cell_rect
                            },
                            colors.accent,
                            0.0,
                        );
                    }
                    x += metrics.cell * group;
                    if x > rect.x + rect.width - INSET {
                        break;
                    }
                }
                let ascii_x = rect.x + metrics.gutter + INSET + 16.0 * metrics.cell * group + 20.0;
                if ascii_x + 16.0 * metrics.cell < rect.x + rect.width {
                    let ascii: String = editor
                        .document
                        .text
                        .slice(begin..end)
                        .chars()
                        .map(|c| {
                            if c.is_ascii_graphic() || c == ' ' {
                                c
                            } else {
                                '·'
                            }
                        })
                        .collect();
                    painter.fill_text_font(
                        Rect {
                            x: ascii_x,
                            y,
                            width: 16.0 * metrics.cell,
                            height: metrics.height,
                        },
                        &ascii,
                        colors.text_secondary,
                        metrics.font,
                        TextAlign::Start,
                        Some(FONT),
                        false,
                        false,
                    );
                }
                continue;
            }
            let begin = editor.document.text.line_to_char(row);
            let end = editor.document.line_end(row);
            let left = editor.left.min(end - begin);
            let start = begin + left;
            let source = editor
                .document
                .text
                .slice(start..(start + editor.columns.max(1) * 2 + 16).min(end))
                .to_string();
            let line = if editor.document.markdown_mode() != MarkdownMode::Raw
                && editor.document.markdown_index.code_at(start)
            {
                markdown::code_line(&source, editor.document.markdown_mode())
            } else {
                markdown::line(&source, editor.document.markdown_mode())
            };
            let mut x = rect.x + metrics.gutter + INSET;
            if line.quote {
                painter.fill_rect(
                    Rect {
                        x: x - 6.0,
                        y: y + 3.0,
                        width: 2.0,
                        height: metrics.height - 6.0,
                    },
                    colors.accent,
                    1.0,
                );
            }
            if line.rule {
                painter.fill_rect(
                    Rect {
                        x,
                        y: y + metrics.height / 2.0,
                        width: rect.width - metrics.gutter - INSET * 2.0,
                        height: 1.0,
                    },
                    colors.text_secondary,
                    0.0,
                );
            }
            let mut position = start;
            for span in line.spans {
                let span_color = if span.muted {
                    colors.text_secondary
                } else if span.link || span.code {
                    colors.accent
                } else {
                    colors.text_primary
                };
                let mut buffer = String::new();
                let mut run_x = x;
                for c in span.text.chars() {
                    let width = advance(c, position - begin, editor.tab_width) as f32
                        * metrics.cell
                        * line.scale;
                    let cell = Rect {
                        x,
                        y,
                        width: width.max(1.0),
                        height: metrics.height,
                    };
                    let selected = editor.document.markdown_mode() != MarkdownMode::Preview
                        && selection.contains(&position);
                    let found = editor.document.markdown_mode() != MarkdownMode::Preview
                        && matches.is_some_and(|m| contains(&m.ranges, position));
                    if selected || found || span.code {
                        painter.fill_rect(
                            cell,
                            if selected {
                                colors.accent.mix(colors.surface, 0.65)
                            } else {
                                colors.accent.mix(colors.surface, 0.9)
                            },
                            2.0,
                        );
                    }
                    if position == editor.cursor
                        && editor.document.markdown_mode() != MarkdownMode::Preview
                        && self.focused.get()
                    {
                        painter.fill_rect(
                            Rect {
                                width: if editor.overwrite { width } else { 2.0 },
                                height: if editor.overwrite {
                                    2.0
                                } else {
                                    metrics.height
                                },
                                y: if editor.overwrite {
                                    y + metrics.height - 3.0
                                } else {
                                    y
                                },
                                ..cell
                            },
                            colors.accent,
                            0.0,
                        );
                    }
                    if c == '\t' {
                        draw_run(
                            painter,
                            Rect {
                                x: run_x,
                                y,
                                width: x - run_x,
                                height: metrics.height,
                            },
                            &buffer,
                            span_color,
                            metrics.font * line.scale,
                            &span,
                        );
                        buffer.clear();
                        x += width;
                        run_x = x;
                    } else if c.is_control() {
                        buffer.push('·');
                        x += width;
                    } else {
                        buffer.push(c);
                        x += width;
                    }
                    position += 1;
                    if x > rect.x + rect.width {
                        break;
                    }
                }
                draw_run(
                    painter,
                    Rect {
                        x: run_x,
                        y,
                        width: (x - run_x).max(1.0),
                        height: metrics.height,
                    },
                    &buffer,
                    span_color,
                    metrics.font * line.scale,
                    &span,
                );
                if x > rect.x + rect.width {
                    break;
                }
            }
            if position == editor.cursor
                && editor.document.markdown_mode() != MarkdownMode::Preview
                && self.focused.get()
            {
                painter.fill_rect(
                    Rect {
                        x,
                        y,
                        width: 2.0,
                        height: metrics.height,
                    },
                    colors.accent,
                    0.0,
                );
            }
            if selection.start <= end
                && selection.end > end
                && editor.document.markdown_mode() != MarkdownMode::Preview
            {
                painter.fill_rect(
                    Rect {
                        x,
                        y,
                        width: metrics.cell,
                        height: metrics.height,
                    },
                    colors.accent.mix(colors.surface, 0.65),
                    0.0,
                );
            }
        }
        let count = editor.line_count();
        if count > editor.rows {
            let height = (rect.height * editor.rows as f32 / count as f32).max(24.0);
            let top =
                (rect.height - height) * editor.top as f32 / (count - editor.rows).max(1) as f32;
            painter.fill_rect(
                Rect {
                    x: rect.x + rect.width - 8.0,
                    y: rect.y + top,
                    width: 5.0,
                    height,
                },
                colors.text_secondary,
                2.5,
            );
        }
        let extent = Self::horizontal_extent(&editor).max(editor.left + editor.columns);
        if extent > editor.columns {
            let width = (rect.width * editor.columns as f32 / extent as f32).max(24.0);
            let left =
                (rect.width - width) * editor.left as f32 / (extent - editor.columns).max(1) as f32;
            painter.fill_rect(
                Rect {
                    x: rect.x + left,
                    y: rect.y + rect.height - 7.0,
                    width,
                    height: 4.0,
                },
                colors.text_secondary,
                2.0,
            );
        }
        painter.pop_clip();
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let on_change = self.on_change.clone();
        let focused = self.focused.clone();
        let drag_axis = self.drag_axis.clone();
        let tab = self.tab.clone();
        Some(Rc::new(move |point, rect| {
            focused.set(true);
            let mut editor = tab.borrow_mut();
            let metrics = Metrics::new(&editor);
            if point.y >= rect.height - 12.0 {
                drag_axis.set(2);
                let count = Self::horizontal_extent(&editor).saturating_sub(editor.columns);
                editor.left = ((point.x / rect.width).clamp(0.0, 1.0) * count as f32) as usize;
            } else if point.x >= rect.width - 12.0 {
                drag_axis.set(1);
                let count = editor.line_count().saturating_sub(editor.rows);
                editor.top = ((point.y / rect.height).clamp(0.0, 1.0) * count as f32) as usize;
            } else {
                drag_axis.set(0);
                let target = Self::target(&editor, point, metrics);
                editor.move_to(target, false);
            }
            drop(editor);
            on_change();
        }))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let drag_axis = self.drag_axis.clone();
        let on_change = self.on_change.clone();
        let tab = self.tab.clone();
        Some(Rc::new(move |point, rect| {
            let mut editor = tab.borrow_mut();
            let metrics = Metrics::new(&editor);
            if drag_axis.get() == 2 {
                let count = Self::horizontal_extent(&editor).saturating_sub(editor.columns);
                editor.left = ((point.x / rect.width).clamp(0.0, 1.0) * count as f32) as usize;
            } else if drag_axis.get() == 1 {
                let count = editor.line_count().saturating_sub(editor.rows);
                editor.top = ((point.y / rect.height).clamp(0.0, 1.0) * count as f32) as usize;
            } else {
                if point.y < 0.0 {
                    editor.top = editor.top.saturating_sub(1);
                } else if point.y > rect.height {
                    editor.top =
                        (editor.top + 1).min(editor.line_count().saturating_sub(editor.rows));
                }
                let target = Self::target(&editor, point, metrics);
                editor.move_to(target, true);
            }
            drop(editor);
            on_change();
        }))
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        let on_change = self.on_change.clone();
        let tab = self.tab.clone();
        Some(Rc::new(move |delta| {
            let mut editor = tab.borrow_mut();
            let line_delta = (delta / Metrics::new(&editor).height).round() as isize;
            editor.top = editor
                .top
                .saturating_add_signed(line_delta)
                .min(editor.line_count().saturating_sub(editor.rows));
            drop(editor);
            on_change();
        }))
    }
}
fn contains(ranges: &[Range<usize>], position: usize) -> bool {
    let index = ranges.partition_point(|range| range.end <= position);
    ranges
        .get(index)
        .is_some_and(|range| range.contains(&position))
}
fn advance(c: char, column: usize, tab_width: usize) -> usize {
    if c == '\t' {
        tab_width - column % tab_width
    } else {
        c.width().unwrap_or(1)
    }
}
fn draw_run(
    painter: &mut dyn Painter,
    rect: Rect,
    text: &str,
    color: creamui_theme::Color,
    font: f32,
    span: &markdown::Span,
) {
    if text.is_empty() {
        return;
    }
    painter.fill_text_font(
        rect,
        text,
        color,
        font,
        TextAlign::Start,
        Some(FONT),
        span.bold,
        span.italic,
    );
    if span.strike {
        painter.fill_rect(
            Rect {
                y: rect.y + rect.height / 2.0,
                height: 1.0,
                ..rect
            },
            color,
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use creamui_core::{Renderer, Size};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Color, Theme, ThemeProvider};

    fn screen(document: Document) -> EditorScreen {
        EditorScreen {
            tab: Rc::new(RefCell::new(Editor::new(document))),
            id: 1,
            revision: Signal::new(0),
            focused: Rc::new(Cell::new(true)),
            matches: Rc::new(RefCell::new(None)),
            on_change: Rc::new(|| {}),
            drag_axis: Rc::new(Cell::new(0)),
        }
    }
    #[derive(Default)]
    struct Counter {
        chars: usize,
        calls: usize,
    }
    impl Painter for Counter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
            self.chars += text.chars().count();
            self.calls += 1;
        }
    }
    #[test]
    fn huge_documents_paint_only_the_viewport() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
            let viewport = Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 600.0,
            };
            for text in ["x\n".repeat(1_000_000), "a".repeat(1_000_000)] {
                let document = Document {
                    text: ropey::Rope::from_str(&text),
                    ..Document::default()
                };
                let screen = screen(document);
                screen.tab.borrow_mut().move_to(900_000, false);
                screen.on_layout(viewport);
                let mut painter = Counter::default();
                screen.paint(&mut painter, viewport);
                assert!(
                    painter.chars < 10_000,
                    "painted {} characters",
                    painter.chars
                );
                assert!(painter.calls < 100);
            }
        });
    }
    #[test]
    fn markdown_pointer_positions_match_heading_scale() {
        let mut document = Document::default();
        document.replace(0..0, "# Header\nbody").unwrap();
        document.markdown = MarkdownMode::Hybrid;
        document.language = crate::document::Language::Markdown;
        let editor = Editor::new(document);
        let metrics = Metrics::new(&editor);
        let position = EditorScreen::target(
            &editor,
            Point {
                x: metrics.gutter + INSET + metrics.cell * 1.7 * 3.0,
                y: 4.0,
            },
            metrics,
        );
        assert_eq!(position, 3);
    }
    #[test]
    fn editor_modes_render_in_both_themes() {
        let sample="# Coconut editor\n\nA **bold** idea, *emphasis*, and `inline code`.\n\n- [x] Build something useful\n- [ ] Keep it simple\n\n> Make room for the words.\n\n```rust\n# This stays inside the code block\nlet greeting = \"Hello, world!\";\n```\n\n[Project](https://example.com) and ~~old text~~.\n\nUnicode: café · 日本語 · 😀\n";
        for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                for (mode, view) in [
                    (MarkdownMode::Raw, ViewMode::Utf8),
                    (MarkdownMode::Hybrid, ViewMode::Utf8),
                    (MarkdownMode::Preview, ViewMode::Utf8),
                    (MarkdownMode::Raw, ViewMode::Hex),
                    (MarkdownMode::Raw, ViewMode::Binary),
                ] {
                    let mut document = Document::default();
                    document.replace(0..0, sample).unwrap();
                    document.markdown = mode;
                    document.language = crate::document::Language::Markdown;
                    document.set_view(view).unwrap();
                    document.markdown_index =
                        std::sync::Arc::new(markdown::BlockIndex::build(&document.text));
                    let screen = screen(document);
                    let size = Size {
                        width: 1000.0,
                        height: 650.0,
                    };
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(1000, 650, 1.0, theme.colors.surface, theme.colors);
                    let root = crate::ui::view(
                        layout::Style {
                            size: layout::Size {
                                width: layout::Dimension::Length(size.width),
                                height: layout::Dimension::Length(size.height),
                            },
                            ..crate::ui::column(0.0)
                        },
                        vec![Box::new(screen)],
                    );
                    let scene = Renderer::new().render(root, size, &mut recorder);
                    assert!(scene.focus_hit_test(Point { x: 300.0, y: 200.0 }).is_some());
                    if let Some(directory) = std::env::var_os("CEDIT_SNAPSHOT_DIR") {
                        let mut raster = Rasterizer::new(1000, 650);
                        raster.render(&recorder.finish(), &Damage::Full);
                        std::fs::create_dir_all(&directory).unwrap();
                        raster
                            .pixmap()
                            .save_png(std::path::PathBuf::from(directory).join(format!(
                                "{name}-{}-{}.png",
                                mode.label(),
                                view.label()
                            )))
                            .unwrap();
                    }
                }
            });
        }
    }
}
