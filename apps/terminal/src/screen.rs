use crate::{config::TerminalTheme, terminal::Session, ui::CLEAR};
use creamui_core::{layout, CursorIcon, Painter, Point, Rect, TextAlign, Widget};
use creamui_reactive::Signal;
use creamui_theme::{use_theme, Color, ColorScheme};
use std::{cell::RefCell, rc::Rc};

pub const FONT: &str = "Cterm Mono, DejaVu Sans Mono, Liberation Mono";
pub const PADDING: f32 = 14.0;
const TOP_PADDING: f32 = 6.0;
#[derive(Clone, Copy, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
}
impl Metrics {
    pub fn load(family: &str, font_size: u8) -> Self {
        use creamui_fonts::{FontWeight, LayoutSettings};
        crate::backends::install_terminal_font(family);
        let font_size = font_size as f32;
        let face = creamui_fonts::resolve(FONT, FontWeight::Regular);
        let shaped =
            creamui_fonts::cached_layout(&face, "M", font_size, &LayoutSettings::default());
        let width = shaped.chars.first().map(|c| c.advance).unwrap_or(8.5);
        // The underline is drawn on the bottom 3px of the cell.
        let height = (face.line_metrics(font_size).line_height + 3.0).ceil();
        Self {
            width,
            height,
            font_size,
        }
    }
    fn cell_rect(&self, origin: Rect, row: u16, start: u16, end: u16, scale: f32) -> Rect {
        let snap = |value: f32| (value * scale).round() / scale;
        let x = snap(origin.x + PADDING + start as f32 * self.width);
        let right = snap(origin.x + PADDING + end as f32 * self.width);
        let y = snap(origin.y + TOP_PADDING + row as f32 * self.height);
        let bottom = snap(origin.y + TOP_PADDING + (row + 1) as f32 * self.height);
        Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
    pub fn size(&self, width: f32, height: f32) -> (u16, u16) {
        (
            ((height - TOP_PADDING * 2.0) / self.height)
                .floor()
                .clamp(1.0, 1000.0) as u16,
            ((width - PADDING * 2.0) / self.width)
                .floor()
                .clamp(2.0, 1000.0) as u16,
        )
    }
}
pub type Position = (u16, u16);

pub fn target_at(screen: &vt100::Screen, row: u16, col: u16) -> Option<String> {
    let (_, columns) = screen.size();
    if row >= screen.size().0 || col >= columns {
        return None;
    }
    let mut cells = Vec::new();
    for column in 0..columns {
        let cell = screen.cell(row, column)?;
        if cell.is_wide_continuation() {
            continue;
        }
        let value = if cell.has_contents() {
            cell.contents()
        } else {
            " "
        };
        cells.push((column, column + if cell.is_wide() { 2 } else { 1 }, value));
    }
    let index = cells
        .iter()
        .position(|(start, end, _)| (*start..*end).contains(&col))?;
    let is_separator = |value: &str| value.chars().all(char::is_whitespace);
    if is_separator(cells[index].2) {
        return None;
    }
    let first = (0..=index)
        .rev()
        .find(|&index| index == 0 || is_separator(cells[index - 1].2))?;
    let last = (index..cells.len())
        .find(|&index| is_separator(cells[index].2))
        .unwrap_or(cells.len());
    let token = cells[first..last]
        .iter()
        .map(|(_, _, value)| *value)
        .collect::<String>();
    let token = token
        .trim_start_matches(['(', '[', '{', '"', '\''])
        .trim_end_matches(['.', ',', ';', ':', ')', ']', '}', '"', '\'']);
    if token.starts_with("https://")
        || token.starts_with("http://")
        || token.starts_with("file://")
        || token.starts_with('/')
        || token.starts_with("~/")
    {
        Some(token.into())
    } else {
        None
    }
}
#[derive(Clone, Copy)]
pub struct Selection {
    pub anchor: Position,
    pub end: Position,
}
impl Selection {
    pub fn ordered(self) -> (Position, Position) {
        if self.anchor <= self.end {
            (self.anchor, self.end)
        } else {
            (self.end, self.anchor)
        }
    }
    pub fn contains(self, row: u16, col: u16) -> bool {
        let (a, b) = self.ordered();
        (row, col) >= a && (row, col) < b
    }
    pub fn text(self, screen: &vt100::Screen) -> String {
        let (a, b) = self.ordered();
        let mut result = String::new();
        let (_, cols) = screen.size();
        for row in a.0..=b.0 {
            let start = if row == a.0 { a.1 } else { 0 };
            let end = if row == b.0 { b.1 } else { cols };
            let mut line = String::new();
            for col in start..end {
                if let Some(cell) = screen.cell(row, col) {
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    line.push_str(if cell.has_contents() {
                        cell.contents()
                    } else {
                        " "
                    });
                }
            }
            result.push_str(line.trim_end());
            if row < b.0 && !screen.row_wrapped(row) {
                result.push('\n');
            }
        }
        result
    }
}
pub struct TerminalScreen {
    pub session: Rc<RefCell<Session>>,
    pub selection: Signal<Option<Selection>>,
    pub revision: Signal<u64>,
    pub metrics: Metrics,
    pub ctrl_held: Signal<bool>,
    pub open_target: Option<Rc<dyn Fn(String)>>,
    pub focused: bool,
    pub on_focus: Option<Rc<dyn Fn()>>,
    pub ligatures: bool,
    pub background_opacity: u8,
}
impl TerminalScreen {
    fn position(metrics: Metrics, p: Point, screen: &vt100::Screen) -> Position {
        let (rows, cols) = screen.size();
        (
            ((p.y - TOP_PADDING) / metrics.height)
                .floor()
                .clamp(0.0, (rows - 1) as f32) as u16,
            ((p.x - PADDING) / metrics.width)
                .floor()
                .clamp(0.0, cols as f32) as u16,
        )
    }
}
impl Widget for TerminalScreen {
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
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let theme = use_theme();
        let session = self.session.borrow();
        let screen = session.parser.screen();
        let selection = self.selection.get();
        let m = self.metrics;
        let terminal = terminal_colors(session.theme, theme.colors);
        painter.push_clip(rect);
        if session.theme != TerminalTheme::Default {
            painter.fill_rect(
                rect,
                with_opacity(terminal.background, self.background_opacity),
                0.0,
            );
        }
        let (rows, cols) = screen.size();
        let scale = painter.scale_factor();
        let selected = |row, col| selection.is_some_and(|s| s.contains(row, col));
        // Paint backgrounds before any text, merging adjacent equal colors.
        // Shared pixel-aligned edges avoid antialiased seams even at fractional scale.
        for row in 0..rows {
            let mut start = 0;
            while start < cols {
                let background = |col| {
                    screen
                        .cell(row, col)
                        .map(|cell| cell_colors(cell, selected(row, col), theme.colors, terminal).1)
                        .unwrap_or(terminal.default_background)
                };
                let background = |col| match session.search.highlight(screen, row, col) {
                    Some(true) => theme.colors.accent,
                    Some(false) => terminal.background.mix(theme.colors.accent, 0.35),
                    None => background(col),
                };
                let bg = with_opacity(background(start), self.background_opacity);
                let mut end = start + 1;
                while end < cols && with_opacity(background(end), self.background_opacity) == bg {
                    end += 1;
                }
                if bg != CLEAR {
                    painter.fill_rect(m.cell_rect(rect, row, start, end, scale), bg, 0.0);
                }
                start = end;
            }
        }
        for row in 0..rows {
            let mut col = 0;
            while col < cols {
                let Some(cell) = screen.cell(row, col) else {
                    col += 1;
                    continue;
                };
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }
                let (mut fg, _) = cell_colors(cell, selected(row, col), theme.colors, terminal);
                if session.search.highlight(screen, row, col) == Some(true) {
                    fg = terminal.background;
                }
                let span = if cell.is_wide() { 2 } else { 1 };
                if self.ligatures
                    && !cell.is_wide()
                    && cell.has_contents()
                    && cell.contents().is_ascii()
                {
                    let bold = cell.bold();
                    let italic = cell.italic();
                    let mut end = col;
                    let mut text = String::new();
                    while end < cols {
                        let Some(next) = screen.cell(row, end) else {
                            break;
                        };
                        if next.is_wide()
                            || next.is_wide_continuation()
                            || !next.has_contents()
                            || !next.contents().is_ascii()
                            || next.bold() != bold
                            || next.italic() != italic
                        {
                            break;
                        }
                        let (mut next_fg, _) =
                            cell_colors(next, selected(row, end), theme.colors, terminal);
                        if session.search.highlight(screen, row, end) == Some(true) {
                            next_fg = terminal.background;
                        }
                        if next_fg != fg {
                            break;
                        }
                        text.push_str(next.contents());
                        end += 1;
                    }
                    if end > col + 1 {
                        let text_rect = m.cell_rect(rect, row, col, end, scale);
                        painter.fill_text_font(
                            Rect {
                                width: text_rect.width.max(m.width * (end - col) as f32),
                                height: m.height,
                                ..text_rect
                            },
                            &text,
                            fg,
                            m.font_size,
                            TextAlign::Start,
                            Some(FONT),
                            bold,
                            italic,
                        );
                        col = end;
                        continue;
                    }
                }
                let cell_rect = m.cell_rect(rect, row, col, (col + span).min(cols), scale);
                if cell.has_contents()
                    && !crate::terminal_graphics::paint(painter, cell.contents(), cell_rect, fg)
                {
                    let text_rect = Rect {
                        width: cell_rect.width.max(m.width * span as f32),
                        height: m.height,
                        ..cell_rect
                    };
                    painter.fill_text_font(
                        text_rect,
                        cell.contents(),
                        fg,
                        m.font_size,
                        TextAlign::Start,
                        Some(FONT),
                        cell.bold(),
                        cell.italic(),
                    );
                }
                col += span;
            }
        }
        for row in 0..rows {
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() || !cell.underline() {
                    continue;
                }
                let (mut fg, _) = cell_colors(cell, selected(row, col), theme.colors, terminal);
                if session.search.highlight(screen, row, col) == Some(true) {
                    fg = terminal.background;
                }
                let span = if cell.is_wide() { 2 } else { 1 };
                let cell_rect = m.cell_rect(rect, row, col, (col + span).min(cols), scale);
                painter.fill_rect(
                    Rect {
                        y: ((cell_rect.y + cell_rect.height - 3.0) * scale).round() / scale,
                        height: scale.round().max(1.0) / scale,
                        ..cell_rect
                    },
                    fg,
                    0.0,
                );
            }
        }
        if screen.scrollback() == 0 && !screen.hide_cursor() && session.exited.is_none() {
            let (r, c) = screen.cursor_position();
            if c < cols {
                let t = painter.animation_time();
                if ((t * 1.8) as u64).is_multiple_of(2) {
                    painter.fill_rect(
                        Rect {
                            width: 2.0,
                            ..m.cell_rect(rect, r, c, c + 1, scale)
                        },
                        theme.colors.accent,
                        1.0,
                    );
                }
            }
        }
        if self.focused {
            painter.stroke_rect_inside(rect, theme.colors.accent, 1.0, 0.0);
        }
        painter.pop_clip();
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let session = self.session.clone();
        let selection = self.selection.clone();
        let metrics = self.metrics;
        let ctrl_held = self.ctrl_held.clone();
        let open_target = self.open_target.clone();
        let on_focus = self.on_focus.clone();
        Some(Rc::new(move |p, _| {
            if let Some(focus) = &on_focus {
                focus();
            }
            let screen = session.borrow();
            let pos = Self::position(metrics, p, screen.parser.screen());
            if ctrl_held.peek() {
                if let Some(target) = target_at(screen.parser.screen(), pos.0, pos.1) {
                    if let Some(open) = &open_target {
                        open(target);
                        selection.set(None);
                        return;
                    }
                }
            }
            drop(screen);
            selection.set(Some(Selection {
                anchor: pos,
                end: pos,
            }));
        }))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let session = self.session.clone();
        let selection = self.selection.clone();
        let metrics = self.metrics;
        Some(Rc::new(move |p, _| {
            let pos = Self::position(metrics, p, session.borrow().parser.screen());
            selection.update(|s| {
                if let Some(s) = s {
                    s.end = pos;
                }
            });
        }))
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        let session = self.session.clone();
        let selection = self.selection.clone();
        let revision = self.revision.clone();
        Some(Rc::new(move |delta| {
            let mut s = session.borrow_mut();
            if s.parser.screen().alternate_screen() {
                return;
            }
            let old = s.parser.screen().scrollback();
            let lines = (delta / 20.0).round() as isize;
            s.parser
                .screen_mut()
                .set_scrollback(old.saturating_add_signed(-lines));
            drop(s);
            creamui_reactive::batch(|| {
                selection.set(None);
                revision.update(|v| *v = v.wrapping_add(1));
            });
        }))
    }
}
#[derive(Clone, Copy)]
struct TerminalColors {
    foreground: Color,
    background: Color,
    default_background: Color,
    ansi: [Color; 16],
}

fn with_opacity(color: Color, opacity: u8) -> Color {
    Color::rgba(
        color.r,
        color.g,
        color.b,
        (color.a as u16 * opacity as u16 / 100) as u8,
    )
}

fn terminal_colors(theme: TerminalTheme, colors: ColorScheme) -> TerminalColors {
    const DEFAULT_ANSI: [[u8; 3]; 16] = [
        [40, 44, 52],
        [205, 80, 86],
        [100, 165, 100],
        [194, 157, 73],
        [80, 135, 204],
        [163, 108, 190],
        [70, 157, 170],
        [205, 210, 218],
        [93, 100, 113],
        [238, 111, 118],
        [140, 199, 124],
        [230, 194, 112],
        [124, 174, 232],
        [199, 147, 225],
        [118, 199, 207],
        [239, 241, 245],
    ];
    const DRACULA: [[u8; 3]; 16] = [
        [33, 34, 44],
        [255, 85, 85],
        [80, 250, 123],
        [241, 250, 140],
        [189, 147, 249],
        [255, 121, 198],
        [139, 233, 253],
        [248, 248, 242],
        [98, 114, 164],
        [255, 110, 110],
        [105, 255, 148],
        [255, 255, 165],
        [214, 172, 255],
        [255, 146, 218],
        [164, 255, 255],
        [255, 255, 255],
    ];
    const SOLARIZED_DARK: [[u8; 3]; 16] = [
        [7, 54, 66],
        [220, 50, 47],
        [133, 153, 0],
        [181, 137, 0],
        [38, 139, 210],
        [211, 54, 130],
        [42, 161, 152],
        [238, 232, 213],
        [0, 43, 54],
        [203, 75, 22],
        [88, 110, 117],
        [101, 123, 131],
        [131, 148, 150],
        [108, 113, 196],
        [147, 161, 161],
        [253, 246, 227],
    ];
    const SOLARIZED_LIGHT: [[u8; 3]; 16] = [
        [238, 232, 213],
        [220, 50, 47],
        [133, 153, 0],
        [181, 137, 0],
        [38, 139, 210],
        [211, 54, 130],
        [42, 161, 152],
        [7, 54, 66],
        [253, 246, 227],
        [203, 75, 22],
        [147, 161, 161],
        [131, 148, 150],
        [101, 123, 131],
        [108, 113, 196],
        [88, 110, 117],
        [0, 43, 54],
    ];
    let (foreground, background, ansi) = match theme {
        TerminalTheme::Default => (colors.text_primary, colors.surface, DEFAULT_ANSI),
        TerminalTheme::Dracula => (Color::rgb(248, 248, 242), Color::rgb(40, 42, 54), DRACULA),
        TerminalTheme::SolarizedDark => (
            Color::rgb(131, 148, 150),
            Color::rgb(0, 43, 54),
            SOLARIZED_DARK,
        ),
        TerminalTheme::SolarizedLight => (
            Color::rgb(101, 123, 131),
            Color::rgb(253, 246, 227),
            SOLARIZED_LIGHT,
        ),
    };
    TerminalColors {
        foreground,
        background,
        default_background: if theme == TerminalTheme::Default {
            CLEAR
        } else {
            background
        },
        ansi: ansi.map(|[r, g, b]| Color::rgb(r, g, b)),
    }
}

fn cell_colors(
    cell: &vt100::Cell,
    selected: bool,
    colors: ColorScheme,
    terminal: TerminalColors,
) -> (Color, Color) {
    let mut fg = color_with_palette(cell.fgcolor(), terminal.foreground, terminal.ansi);
    let mut bg = color_with_palette(cell.bgcolor(), terminal.default_background, terminal.ansi);
    if cell.inverse() {
        std::mem::swap(&mut fg, &mut bg);
        if fg == CLEAR {
            fg = terminal.background;
        }
    }
    if cell.dim() {
        fg = fg.mix(terminal.background, 0.45);
    }
    if selected {
        bg = colors.selection_background;
        fg = terminal.foreground;
    }
    (fg, bg)
}

#[cfg(test)]
pub fn color(value: vt100::Color, default: Color) -> Color {
    color_with_palette(
        value,
        default,
        terminal_colors(TerminalTheme::Default, ColorScheme::dark()).ansi,
    )
}

fn color_with_palette(value: vt100::Color, default: Color, ansi: [Color; 16]) -> Color {
    match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color::rgb(r, g, b),
        vt100::Color::Idx(n) => {
            let rgb = if n < 16 {
                return ansi[n as usize];
            } else if n < 232 {
                let n = n - 16;
                let v = |x: u8| if x == 0 { 0 } else { 55 + 40 * x };
                [v(n / 36), v(n / 6 % 6), v(n % 6)]
            } else {
                [8 + 10 * (n - 232); 3]
            };
            Color::rgb(rgb[0], rgb[1], rgb[2])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligatures_shape_compatible_terminal_runs_and_opacity_only_fades_backgrounds() {
        use creamui_core::runtime::{PaintOp, PaintPrimitive, RecordingPainter};
        use creamui_theme::{Theme, ThemeProvider};
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let text_runs = |ligatures| {
                let mut session = Session::spawn(
                    1,
                    &crate::terminal::idle_cwd(),
                    &crate::terminal::idle_command(),
                    tx.clone(),
                )
                .unwrap();
                session.resize(2, 20).unwrap();
                session.process(b"==> flag").unwrap();
                let screen = TerminalScreen {
                    session: Rc::new(RefCell::new(session)),
                    selection: Signal::new(None),
                    revision: Signal::new(0),
                    metrics: Metrics {
                        width: 8.0,
                        height: 16.0,
                        font_size: 14.0,
                    },
                    ctrl_held: Signal::new(false),
                    open_target: None,
                    focused: false,
                    on_focus: None,
                    ligatures,
                    background_opacity: 100,
                };
                let mut painter = RecordingPainter::new(theme.colors);
                screen.paint(
                    &mut painter,
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 180.0,
                        height: 48.0,
                    },
                );
                painter
                    .into_ops()
                    .into_iter()
                    .filter_map(|op| match op {
                        PaintOp::Primitive(PaintPrimitive::Text(text)) => {
                            Some(text.text.to_string())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let ligature_runs = text_runs(true);
            let plain_runs = text_runs(false);
            assert!(ligature_runs.iter().any(|text| text == "==> flag"));
            assert!(ligature_runs.len() < plain_runs.len());
            assert_eq!(
                with_opacity(Color::rgba(1, 2, 3, 200), 75),
                Color::rgba(1, 2, 3, 150)
            );
        });
    }

    #[test]
    fn target_at_extracts_urls_and_file_paths() {
        let mut parser = vt100::Parser::new(2, 80, 0);
        let line = "Open (https://example.test/docs?q=1), then /tmp/output.txt";
        parser.process(line.as_bytes());
        assert_eq!(
            target_at(parser.screen(), 0, line.find("example").unwrap() as u16),
            Some("https://example.test/docs?q=1".into())
        );
        assert_eq!(
            target_at(parser.screen(), 0, line.find("/tmp").unwrap() as u16),
            Some("/tmp/output.txt".into())
        );
        assert_eq!(target_at(parser.screen(), 0, 0), None);
    }

    #[test]
    fn ctrl_drag_opens_a_target_without_starting_selection() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(128);
        let mut session = Session::spawn(
            1,
            &crate::terminal::idle_cwd(),
            &crate::terminal::idle_command(),
            tx,
        )
        .unwrap();
        session.resize(2, 80).unwrap();
        session.process(b"https://example.test/docs").unwrap();
        let opened = Rc::new(RefCell::new(None));
        let callback_target = opened.clone();
        let metrics = Metrics {
            width: 8.0,
            height: 16.0,
            font_size: 14.0,
        };
        let selection = Signal::new(None);
        let screen = TerminalScreen {
            session: Rc::new(RefCell::new(session)),
            selection: selection.clone(),
            revision: Signal::new(0),
            metrics,
            ctrl_held: Signal::new(true),
            open_target: Some(Rc::new(move |target| {
                *callback_target.borrow_mut() = Some(target)
            })),
            focused: false,
            on_focus: None,
            ligatures: true,
            background_opacity: 100,
        };
        Widget::on_drag_start(&screen).unwrap()(
            Point {
                x: PADDING + 8.0 * metrics.width + 1.0,
                y: TOP_PADDING + 1.0,
            },
            Rect::default(),
        );
        assert_eq!(*opened.borrow(), Some("https://example.test/docs".into()));
        assert!(selection.peek().is_none());
    }

    #[test]
    fn terminal_themes_override_ansi_and_default_colors() {
        let base = ColorScheme::dark();
        let dracula = terminal_colors(TerminalTheme::Dracula, base);
        let solarized = terminal_colors(TerminalTheme::SolarizedLight, base);
        assert_eq!(dracula.background, Color::rgb(40, 42, 54));
        assert_eq!(
            color_with_palette(vt100::Color::Idx(1), dracula.foreground, dracula.ansi),
            Color::rgb(255, 85, 85)
        );
        assert_eq!(solarized.background, Color::rgb(253, 246, 227));
        assert_eq!(
            color_with_palette(vt100::Color::Default, solarized.foreground, solarized.ansi),
            solarized.foreground
        );
    }
    #[test]
    fn terminal_bars_and_backgrounds_tile_in_cpu_and_gpu_renderers() {
        use creamui_render::{Damage, HeadlessGpu, Rasterizer, SceneRecorder};
        use creamui_theme::{Theme, ThemeProvider};
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let mut session = Session::spawn(
                1,
                &crate::terminal::idle_cwd(),
                &crate::terminal::idle_command(),
                tx,
            )
            .unwrap();
            session.resize(8, 12).unwrap();
            session.process(b"\x1b[?25l\x1b[44m            ").unwrap();
            session.process(b"\x1b[2;1H").unwrap();
            for col in 0..12 {
                session
                    .process(if col % 2 == 0 {
                        b"\x1b[42m "
                    } else {
                        b"\x1b[44m "
                    })
                    .unwrap();
            }
            for (row, text) in [
                (3, "████████████"),
                (4, "────────────"),
                (5, "│"),
                (6, "│"),
                (7, "▀▄▌▐▁▂▃▅▆▇▉▏"),
                (8, "▖▗▘▙▚▛▜▝▞▟░▒"),
            ] {
                session
                    .process(format!("\x1b[0m\x1b[{row};1H{text}").as_bytes())
                    .unwrap();
            }
            let metrics = Metrics {
                width: 8.43,
                height: 23.0,
                font_size: 14.0,
            };
            let screen = TerminalScreen {
                session: Rc::new(RefCell::new(session)),
                selection: Signal::new(None),
                revision: Signal::new(0),
                metrics,
                ctrl_held: Signal::new(false),
                open_target: None,
                focused: false,
                on_focus: None,
                ligatures: true,
                background_opacity: 100,
            };
            let mut gpu = HeadlessGpu::new().ok();
            if let Some(gpu) = &gpu {
                eprintln!("terminal pixel tests: {}", gpu.adapter_name());
            }
            let origin = Rect {
                x: 0.0,
                y: 0.0,
                width: 160.0,
                height: 210.0,
            };
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let width = (origin.width * scale) as u32;
                let height = (origin.height * scale) as u32;
                let mut recorder = SceneRecorder::new();
                recorder.begin(width, height, scale, theme.colors.surface, theme.colors);
                screen.paint(&mut recorder, origin);
                let list = recorder.finish();
                let mut raster = Rasterizer::new(width, height);
                raster.render(&list, &Damage::Full);
                let pixel = |x: u32, y: u32| {
                    let p = raster.pixmap().pixel(x, y).unwrap();
                    [p.red(), p.green(), p.blue()]
                };
                let rgb = |c: Color| [c.r, c.g, c.b];
                // A full block covers every pixel in its cell, including its bearings and leading.
                let full = metrics.cell_rect(origin, 2, 0, 12, scale);
                for y in
                    (full.y * scale).round() as u32..((full.y + full.height) * scale).round() as u32
                {
                    for x in (full.x * scale).round() as u32
                        ..((full.x + full.width) * scale).round() as u32
                    {
                        assert_eq!(
                            pixel(x, y),
                            rgb(theme.colors.text_primary),
                            "block gap at {scale}: {x},{y}"
                        );
                    }
                }
                // Distinct adjoining backgrounds meet without a line of the canvas showing through.
                for col in 0..12 {
                    let cell = metrics.cell_rect(origin, 1, col, col + 1, scale);
                    for x in (cell.x * scale).round() as u32
                        ..((cell.x + cell.width) * scale).round() as u32
                    {
                        assert_eq!(
                            pixel(x, ((cell.y + cell.height / 2.0) * scale) as u32),
                            rgb(color(
                                vt100::Color::Idx(if col % 2 == 0 { 2 } else { 4 }),
                                CLEAR
                            )),
                            "colored seam at {scale}: {x}"
                        );
                    }
                }
                let horizontal = metrics.cell_rect(origin, 3, 0, 12, scale);
                let mut previous = None;
                for x in (horizontal.x * scale).round() as u32
                    ..((horizontal.x + horizontal.width) * scale).round() as u32
                {
                    let stroke = ((horizontal.y * scale).round() as u32
                        ..((horizontal.y + horizontal.height) * scale).round() as u32)
                        .filter(|y| pixel(x, *y) == rgb(theme.colors.text_primary))
                        .collect::<Vec<_>>();
                    assert!(
                        !stroke.is_empty(),
                        "horizontal line is disconnected at {scale}: {x}"
                    );
                    if let Some(prev) = &previous {
                        assert_eq!(&stroke, prev);
                    }
                    previous = Some(stroke);
                }
                let vertical = metrics.cell_rect(origin, 4, 0, 1, scale);
                let bottom = metrics.cell_rect(origin, 5, 0, 1, scale);
                for y in (vertical.y * scale).round() as u32
                    ..((bottom.y + bottom.height) * scale).round() as u32
                {
                    assert!(
                        ((vertical.x * scale).round() as u32
                            ..((vertical.x + vertical.width) * scale).round() as u32)
                            .any(|x| pixel(x, y) == rgb(theme.colors.text_primary)),
                        "vertical line is disconnected at {scale}: {y}"
                    );
                }
                if let Some(gpu) = gpu.as_mut() {
                    assert_eq!(
                        gpu.render_to_pixels(&list),
                        raster.pixmap().data(),
                        "CPU/GPU cell rendering differs at scale {scale}"
                    );
                }
                if let Some(dir) = std::env::var_os("CTERM_SNAPSHOT_DIR") {
                    raster
                        .pixmap()
                        .save_png(
                            std::path::PathBuf::from(dir)
                                .join(format!("terminal-cells-{scale}.png")),
                        )
                        .unwrap();
                }
            }
        });
    }

    #[test]
    fn colored_cell_backgrounds_have_no_seams_at_fractional_scales() {
        use creamui_render::{Damage, Rasterizer, SceneRecorder};
        use creamui_theme::{Theme, ThemeProvider};
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark();
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let (tx, _rx) = std::sync::mpsc::sync_channel(128);
            let mut session = Session::spawn(
                1,
                &crate::terminal::idle_cwd(),
                &crate::terminal::idle_command(),
                tx,
            )
            .unwrap();
            session.resize(3, 12).unwrap();
            session.process(b"\x1b[?25l\x1b[44m            ").unwrap();
            let screen = TerminalScreen {
                session: Rc::new(RefCell::new(session)),
                selection: Signal::new(None),
                revision: Signal::new(0),
                metrics: Metrics {
                    width: 8.43,
                    height: 23.0,
                    font_size: 14.0,
                },
                ctrl_held: Signal::new(false),
                open_target: None,
                focused: false,
                on_focus: None,
                ligatures: true,
                background_opacity: 100,
            };
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let width = (160.0 * scale) as u32;
                let height = (100.0 * scale) as u32;
                let mut recorder = SceneRecorder::new();
                recorder.begin(width, height, scale, theme.colors.surface, theme.colors);
                screen.paint(
                    &mut recorder,
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 160.0,
                        height: 100.0,
                    },
                );
                let mut raster = Rasterizer::new(width, height);
                raster.render(&recorder.finish(), &Damage::Full);
                let expected = color(vt100::Color::Idx(4), CLEAR);
                for x in ((PADDING * scale).ceil() as u32 + 1)
                    ..(((PADDING + 12.0 * 8.43) * scale).floor() as u32 - 1)
                {
                    let pixel = raster
                        .pixmap()
                        .pixel(x, ((TOP_PADDING + 10.0) * scale) as u32)
                        .unwrap();
                    assert_eq!(
                        [pixel.red(), pixel.green(), pixel.blue()],
                        [expected.r, expected.g, expected.b],
                        "background seam at scale {scale}, x={x}"
                    );
                }
            }
        });
    }

    #[test]
    fn selection_respects_wide_cells_and_line_wrap() {
        let mut p = vt100::Parser::new(3, 6, 0);
        p.process("abc界def".as_bytes());
        assert_eq!(
            Selection {
                anchor: (0, 0),
                end: (1, 2)
            }
            .text(p.screen()),
            "abc界def"
        );
    }
}
