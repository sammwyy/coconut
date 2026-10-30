use creamui_render::platform::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Close,
    Quit,
    NextTab,
    PreviousTab,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Replace,
    NextMatch,
    PreviousMatch,
    GoTo,
    Indent,
    Outdent,
    DuplicateLine,
    DeleteLine,
    MoveLineUp,
    MoveLineDown,
    Bold,
    Italic,
    InlineCode,
    Markdown,
    Bytes,
    PlainText,
    Raw,
    Preview,
    Hybrid,
    Utf8,
    Ascii,
    Hex,
    Binary,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    LineNumbers,
    Overwrite,
    Spaces,
    LineEnding,
    MatchCase,
    WholeWord,
    Regex,
    ReplaceOne,
    ReplaceAll,
    Palette,
    Help,
    Menu,
}
pub struct Entry {
    pub command: Command,
    pub label: &'static str,
    pub binding: &'static str,
}
macro_rules! entries { ($( $command:ident, $label:literal, $binding:literal; )*) => { pub const ENTRIES: &[Entry] = &[$(Entry { command: Command::$command, label: $label, binding: $binding },)*]; }; }
entries! {
    New, "New document", "Ctrl+N"; Open, "Open file", "Ctrl+O"; Save, "Save", "Ctrl+S"; SaveAs, "Save as", "Ctrl+Shift+S";
    Close, "Close tab", "Ctrl+W"; Quit, "Quit", "Ctrl+Q"; NextTab, "Next tab", "Ctrl+Tab"; PreviousTab, "Previous tab", "Ctrl+Shift+Tab";
    Undo, "Undo", "Ctrl+Z"; Redo, "Redo", "Ctrl+Shift+Z"; Cut, "Cut", "Ctrl+X"; Copy, "Copy", "Ctrl+C"; Paste, "Paste", "Ctrl+V";
    SelectAll, "Select all", "Ctrl+A"; Find, "Find", "Ctrl+F"; Replace, "Find and replace", "Ctrl+H";
    NextMatch, "Next match", "F3"; PreviousMatch, "Previous match", "Shift+F3"; GoTo, "Go to line / byte offset", "Ctrl+G";
    Indent, "Indent", "Tab"; Outdent, "Outdent", "Shift+Tab"; DuplicateLine, "Duplicate line", "Ctrl+Shift+D"; DeleteLine, "Delete line", "Ctrl+Shift+K";
    MoveLineUp, "Move line up", "Alt+Up"; MoveLineDown, "Move line down", "Alt+Down";
    Bold, "Markdown: bold", "Ctrl+B"; Italic, "Markdown: italic", "Ctrl+I"; InlineCode, "Markdown: inline code", "Ctrl+E";
    Markdown, "Render: Markdown", "Ctrl+Alt+4";
    Bytes, "Render: bytes", "Ctrl+Alt+5";
    PlainText, "Render: plain text", "Ctrl+Alt+0";
    Raw, "Markdown: raw", "Ctrl+Alt+1"; Preview, "Markdown: preview", "Ctrl+Alt+2"; Hybrid, "Markdown: hybrid", "Ctrl+Alt+3";
    Utf8, "Encoding: UTF-8", "Ctrl+Alt+U"; Ascii, "Encoding: ASCII", "Ctrl+Alt+A"; Hex, "View: hexadecimal", "Ctrl+Alt+H"; Binary, "View: binary", "Ctrl+Alt+B";
    ZoomIn, "Zoom in", "Ctrl+="; ZoomOut, "Zoom out", "Ctrl+-"; ResetZoom, "Reset zoom", "Ctrl+0";
    LineNumbers, "Toggle line numbers", "Ctrl+Alt+L"; Overwrite, "Toggle insert / overwrite", "Insert";
    Spaces, "Toggle tabs / spaces", "Ctrl+Alt+T"; LineEnding, "Convert LF / CRLF", "Ctrl+Alt+E";
    MatchCase, "Search: match case", "Alt+C"; WholeWord, "Search: whole word", "Alt+W"; Regex, "Search: regular expression", "Alt+R";
    ReplaceOne, "Replace current match", "Alt+Enter"; ReplaceAll, "Replace all matches", "Ctrl+Alt+Enter";
    Menu, "Application menu", "F10";
    Palette, "Command palette", "Ctrl+Shift+P"; Help, "Keyboard shortcuts", "F1";
}
pub fn shortcut(key: &Key, modifiers: Modifiers, editing: bool) -> Option<Command> {
    ENTRIES
        .iter()
        .find(|entry| {
            if !editing
                && matches!(
                    entry.command,
                    Command::Indent | Command::Outdent | Command::Overwrite
                )
            {
                return false;
            }
            binding_matches(entry.binding, key, modifiers)
        })
        .map(|entry| entry.command)
        .or_else(|| match key {
            Key::Character(c)
                if modifiers.ctrl && !modifiers.alt && c.eq_ignore_ascii_case("y") =>
            {
                Some(Command::Redo)
            }
            Key::Character(c) if modifiers.ctrl && c == "+" => Some(Command::ZoomIn),
            _ => None,
        })
}
fn binding_matches(binding: &str, key: &Key, modifiers: Modifiers) -> bool {
    if modifiers.ctrl != binding.contains("Ctrl+")
        || modifiers.alt != binding.contains("Alt+")
        || modifiers.shift != binding.contains("Shift+")
        || modifiers.logo
    {
        return false;
    }
    let name = binding.rsplit('+').next().unwrap_or(binding);
    match (name, key) {
        ("Tab", Key::Tab)
        | ("Up", Key::Up)
        | ("Down", Key::Down)
        | ("Enter", Key::Enter)
        | ("Insert", Key::Insert)
        | ("F1", Key::F1)
        | ("F10", Key::F10)
        | ("F3", Key::F3) => true,
        (_, Key::Character(text)) => name.eq_ignore_ascii_case(text),
        _ => false,
    }
}

impl Command {
    pub fn category(self) -> &'static str {
        use Command::*;
        match self {
            New | Open | Save | SaveAs | Close | Quit => "File",
            Undo | Redo | Cut | Copy | Paste | SelectAll | Indent | Outdent | DuplicateLine
            | DeleteLine | MoveLineUp | MoveLineDown | Bold | Italic | InlineCode => "Edit",
            Find | Replace | NextMatch | PreviousMatch | GoTo | MatchCase | WholeWord | Regex
            | ReplaceOne | ReplaceAll => "Search",
            NextTab | PreviousTab => "Tabs",
            Menu | Palette | Help => "Application",
            _ => "View",
        }
    }
}
