//! xterm keyboard encoding, including modifiers and application cursor mode.
use creamui_render::platform::{Key, Modifiers};
pub fn encode(key: &Key, m: Modifiers, application_cursor: bool) -> Vec<u8> {
    let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.ctrl);
    let sequence = match key {
        Key::Character(text) => {
            let mut bytes = text.as_bytes().to_vec();
            if m.ctrl && text.len() == 1 {
                let c = text.as_bytes()[0];
                bytes = match c {
                    b' ' | b'@' | b'2' => vec![0],
                    b'a'..=b'z' | b'A'..=b'Z' | b'['..=b'_' => vec![c & 0x1f],
                    b'3' => vec![27],
                    b'4' => vec![28],
                    b'5' => vec![29],
                    b'6' => vec![30],
                    b'7' | b'/' => vec![31],
                    b'8' | b'?' => vec![127],
                    _ => bytes,
                };
            }
            if m.alt {
                bytes.insert(0, 27);
            }
            return bytes;
        }
        Key::Space => {
            let mut bytes = vec![if m.ctrl { 0 } else { b' ' }];
            if m.alt {
                bytes.insert(0, 27);
            }
            return bytes;
        }
        Key::Enter => "\r".into(),
        Key::Backspace => if m.ctrl { "\x08" } else { "\x7f" }.into(),
        Key::Tab => if m.shift { "\x1b[Z" } else { "\t" }.into(),
        Key::Escape => "\x1b".into(),
        Key::Up | Key::Down | Key::Right | Key::Left | Key::Home | Key::End => {
            let end = match key {
                Key::Up => 'A',
                Key::Down => 'B',
                Key::Right => 'C',
                Key::Left => 'D',
                Key::Home => 'H',
                _ => 'F',
            };
            if modifier > 1 {
                format!("\x1b[1;{modifier}{end}")
            } else {
                format!("\x1b{}{end}", if application_cursor { 'O' } else { '[' })
            }
        }
        Key::F1 | Key::F2 | Key::F3 | Key::F4 => {
            let end = match key {
                Key::F1 => 'P',
                Key::F2 => 'Q',
                Key::F3 => 'R',
                _ => 'S',
            };
            if modifier > 1 {
                format!("\x1b[1;{modifier}{end}")
            } else {
                format!("\x1bO{end}")
            }
        }
        Key::Insert
        | Key::Delete
        | Key::PageUp
        | Key::PageDown
        | Key::F5
        | Key::F6
        | Key::F7
        | Key::F8
        | Key::F9
        | Key::F10
        | Key::F11
        | Key::F12 => {
            let n = match key {
                Key::Insert => 2,
                Key::Delete => 3,
                Key::PageUp => 5,
                Key::PageDown => 6,
                Key::F5 => 15,
                Key::F6 => 17,
                Key::F7 => 18,
                Key::F8 => 19,
                Key::F9 => 20,
                Key::F10 => 21,
                Key::F11 => 23,
                _ => 24,
            };
            if modifier > 1 {
                format!("\x1b[{n};{modifier}~")
            } else {
                format!("\x1b[{n}~")
            }
        }
        _ => return Vec::new(),
    };
    let mut bytes = sequence.into_bytes();
    if m.alt && matches!(key, Key::Enter | Key::Backspace | Key::Tab) {
        bytes.insert(0, 27);
    }
    bytes
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\x1b', "");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_control_and_application_keys() {
        let m = Modifiers::default();
        assert_eq!(
            encode(
                &Key::Character("c".into()),
                Modifiers { ctrl: true, ..m },
                false
            ),
            b"\x03"
        );
        assert_eq!(encode(&Key::Tab, m, false), b"\t");
        assert_eq!(encode(&Key::Up, m, true), b"\x1bOA");
        assert_eq!(
            encode(&Key::Right, Modifiers { ctrl: true, ..m }, false),
            b"\x1b[1;5C"
        );
        assert_eq!(encode(&Key::F12, m, false), b"\x1b[24~");
        assert_eq!(
            encode(&Key::Character("á".into()), m, false),
            "á".as_bytes()
        );
        assert_eq!(
            encode(
                &Key::Character("b".into()),
                Modifiers { alt: true, ..m },
                false
            ),
            b"\x1bb"
        );
    }
    #[test]
    fn paste_preserves_bracketed_newlines_and_removes_embedded_escape_sequences() {
        assert_eq!(paste("a\r\nb", true), b"\x1b[200~a\nb\x1b[201~");
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("\x1b[201~", true), b"\x1b[200~[201~\x1b[201~");
    }
}
