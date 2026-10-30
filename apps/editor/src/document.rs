use ropey::Rope;
use std::{
    fs,
    io::{BufWriter, Write},
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Utf8,
    Ascii,
    Hex,
    Binary,
}
impl ViewMode {
    pub const ALL: [Self; 4] = [Self::Utf8, Self::Ascii, Self::Hex, Self::Binary];
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF",
            Self::Ascii => "ASCII",
            Self::Hex => "HEX",
            Self::Binary => "Binary",
        }
    }
    pub fn bytes(self) -> bool {
        matches!(self, Self::Hex | Self::Binary)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    PlainText,
    Markdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownMode {
    Raw,
    Preview,
    Hybrid,
}
impl MarkdownMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Raw => "Raw",
            Self::Preview => "Preview",
            Self::Hybrid => "Hybrid",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Ascii,
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Bytes,
}
impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ascii => "ASCII",
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Bytes => "Bytes",
        }
    }
}
#[derive(Clone)]
pub struct Snapshot {
    pub newline: &'static str,
    pub text: Rope,
    pub encoding: Encoding,
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DiskStamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}
impl DiskStamp {
    pub fn read(path: &Path) -> Option<Self> {
        fs::metadata(path).ok().map(|m| Self {
            modified: m.modified().ok(),
            len: m.len(),
        })
    }
}
#[derive(Clone)]
pub struct Document {
    pub text: Rope,
    pub path: Option<PathBuf>,
    pub encoding: Encoding,
    pub view: ViewMode,
    pub markdown: MarkdownMode,
    pub language: Language,
    pub revision: u64,
    pub saved_revision: u64,
    pub stamp: Option<DiskStamp>,
    pub newline: &'static str,
    pub markdown_index: Arc<crate::markdown::BlockIndex>,
    pub markdown_revision: Option<u64>,
    pub(crate) next_revision: u64,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            text: Rope::new(),
            path: None,
            encoding: Encoding::Utf8,
            view: ViewMode::Utf8,
            markdown: MarkdownMode::Raw,
            language: Language::PlainText,
            revision: 0,
            saved_revision: 0,
            stamp: None,
            newline: "\n",
            markdown_index: Arc::new(crate::markdown::BlockIndex::default()),
            markdown_revision: Some(0),
            next_revision: 1,
        }
    }
}
impl Document {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let data = fs::read(&path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        let (text, encoding) = decode(&data)?;
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let stamp = DiskStamp::read(&path);
        let text = Rope::from_str(&text);
        let language = if path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md")
                || extension.eq_ignore_ascii_case("markdown")
                || extension.eq_ignore_ascii_case("mdown")
        }) {
            Language::Markdown
        } else {
            Language::PlainText
        };
        Ok(Self {
            language,
            markdown_index: Arc::new(crate::markdown::BlockIndex::build(&text)),
            text,
            view: if encoding == Encoding::Bytes {
                ViewMode::Hex
            } else {
                ViewMode::Utf8
            },
            encoding,
            path: Some(path),
            stamp,
            newline,
            ..Self::default()
        })
    }
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        format!("{name}{} - CEdit", if self.dirty() { " *" } else { "" })
    }
    pub fn markdown_mode(&self) -> MarkdownMode {
        if self.language == Language::Markdown && !self.view.bytes() {
            self.markdown
        } else {
            MarkdownMode::Raw
        }
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            newline: self.newline,
            text: self.text.clone(),
            encoding: self.encoding,
            revision: self.revision,
        }
    }
    pub fn restore(&mut self, snapshot: Snapshot) {
        self.markdown_revision = None;
        self.text = snapshot.text;
        self.newline = snapshot.newline;
        self.encoding = snapshot.encoding;
        self.revision = snapshot.revision;
        match self.encoding {
            Encoding::Bytes if !self.view.bytes() => self.view = ViewMode::Hex,
            Encoding::Ascii => self.view = ViewMode::Ascii,
            _ if self.view.bytes() || self.view == ViewMode::Ascii => self.view = ViewMode::Utf8,
            _ => {}
        }
    }
    pub fn touch(&mut self) {
        self.revision = self.next_revision;
        self.next_revision += 1;
        self.markdown_revision = None;
    }
    pub fn replace(&mut self, range: Range<usize>, value: &str) -> Result<(), String> {
        if (self.view == ViewMode::Ascii || self.encoding == Encoding::Ascii) && !value.is_ascii() {
            return Err("ASCII mode accepts only ASCII characters.".into());
        }
        if self.encoding == Encoding::Bytes && value.chars().any(|c| c as u32 > 255) {
            return Err("Byte mode accepts values from 00 to FF.".into());
        }
        self.text.remove(range.clone());
        self.text.insert(range.start, value);
        self.touch();
        Ok(())
    }
    pub fn set_view(&mut self, view: ViewMode) -> Result<(), String> {
        if view == ViewMode::Ascii
            && self.encoding != Encoding::Bytes
            && self.text.chars().any(|c| !c.is_ascii())
        {
            return Err("The document contains non-ASCII characters. Use UTF text or HEX.".into());
        }
        if view.bytes() && self.encoding != Encoding::Bytes {
            let mut data = Vec::new();
            write_snapshot(&self.snapshot(), &mut data).map_err(|e| e.to_string())?;
            self.text = Rope::from_str(&data.iter().map(|&b| b as char).collect::<String>());
            self.encoding = Encoding::Bytes;
        } else if !view.bytes() && self.encoding == Encoding::Bytes {
            let data: Vec<u8> = self.text.chars().map(|c| c as u8).collect();
            let (text, encoding) = decode(&data)?;
            if encoding == Encoding::Bytes {
                return Err(
                    "These bytes are not valid UTF text. Use HEX or Binary to edit them.".into(),
                );
            }
            if view == ViewMode::Ascii && !text.is_ascii() {
                return Err(
                    "The document contains non-ASCII characters. Use UTF text or HEX.".into(),
                );
            }
            self.text = Rope::from_str(&text);
            self.encoding = encoding;
        }
        if view == ViewMode::Ascii && self.encoding != Encoding::Ascii {
            self.encoding = Encoding::Ascii;
            self.touch();
        }
        self.markdown_revision = None;
        self.view = view;
        Ok(())
    }
    pub fn line_end(&self, line: usize) -> usize {
        let start = self.text.line_to_char(line);
        let mut end = start + self.text.line(line).len_chars();
        while end > start && matches!(self.text.char(end - 1), '\n' | '\r') {
            end -= 1;
        }
        end
    }
    pub fn byte_len(&self) -> usize {
        match self.encoding {
            Encoding::Bytes => self.text.len_chars(),
            Encoding::Utf8 | Encoding::Ascii => self.text.len_bytes(),
            Encoding::Utf8Bom => self.text.len_bytes() + 3,
            _ => self.text.len_utf16_cu() * 2 + 2,
        }
    }
}
fn decode(data: &[u8]) -> Result<(String, Encoding), String> {
    if (data.starts_with(&[0xff, 0xfe]) || data.starts_with(&[0xfe, 0xff]))
        && data.len().is_multiple_of(2)
    {
        let le = data[0] == 0xff;
        let units: Vec<u16> = data[2..]
            .chunks_exact(2)
            .map(|b| {
                if le {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        if let Ok(text) = String::from_utf16(&units) {
            return Ok((
                text,
                if le {
                    Encoding::Utf16Le
                } else {
                    Encoding::Utf16Be
                },
            ));
        }
    }
    let bom = data.starts_with(&[0xef, 0xbb, 0xbf]);
    let body = if bom { &data[3..] } else { data };
    Ok(match std::str::from_utf8(body) {
        Ok(text) if !text.contains('\0') => (
            text.into(),
            if bom {
                Encoding::Utf8Bom
            } else {
                Encoding::Utf8
            },
        ),
        _ => (data.iter().map(|&b| b as char).collect(), Encoding::Bytes),
    })
}
pub fn write_snapshot(snapshot: &Snapshot, writer: &mut impl Write) -> std::io::Result<()> {
    match snapshot.encoding {
        Encoding::Utf8 | Encoding::Utf8Bom | Encoding::Ascii => {
            if snapshot.encoding == Encoding::Utf8Bom {
                writer.write_all(&[0xef, 0xbb, 0xbf])?;
            }
            for chunk in snapshot.text.chunks() {
                if snapshot.encoding == Encoding::Ascii && !chunk.is_ascii() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Non-ASCII character in ASCII document",
                    ));
                }
                writer.write_all(chunk.as_bytes())?;
            }
        }
        Encoding::Bytes => {
            for c in snapshot.text.chars() {
                writer.write_all(&[c as u8])?;
            }
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let le = snapshot.encoding == Encoding::Utf16Le;
            writer.write_all(if le { &[0xff, 0xfe] } else { &[0xfe, 0xff] })?;
            for c in snapshot.text.chars() {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    writer.write_all(&if le {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    })?;
                }
            }
        }
    }
    Ok(())
}
pub fn save(
    snapshot: &Snapshot,
    path: &Path,
    expected: Option<&DiskStamp>,
) -> Result<DiskStamp, String> {
    let actual_path = if path.is_symlink() {
        fs::canonicalize(path).map_err(|e| e.to_string())?
    } else {
        path.to_path_buf()
    };
    let parent = actual_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    if let Ok(metadata) = fs::metadata(&actual_path) {
        temp.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|e| e.to_string())?;
    }
    {
        let mut writer = BufWriter::new(temp.as_file_mut());
        write_snapshot(snapshot, &mut writer)
            .and_then(|()| writer.flush())
            .map_err(|e| e.to_string())?;
    }
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    if let Some(stamp) = expected {
        if DiskStamp::read(path).as_ref() != Some(stamp) {
            return Err("The file changed on disk. Save As to keep both versions.".into());
        }
    }
    temp.persist(&actual_path)
        .map_err(|e| format!("Cannot save {}: {e}", path.display()))?;
    DiskStamp::read(path).ok_or_else(|| "Saved file metadata is unavailable.".into())
}
pub fn parse_bytes(text: &str, radix: u32) -> Result<String, String> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let width = if radix == 16 { 2 } else { 8 };
    if !digits.is_ascii() || !digits.len().is_multiple_of(width) {
        return Err(format!("Use complete groups of {width} digits per byte."));
    }
    digits
        .as_bytes()
        .chunks(width)
        .map(|chunk| {
            u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), radix)
                .map(|byte| byte as char)
                .map_err(|_| "Invalid byte value.".into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodings_and_all_bytes_round_trip() {
        for data in [
            b"hello\r\nworld".to_vec(),
            vec![0xef, 0xbb, 0xbf, b'a'],
            vec![0xff, 0xfe, 0x3d, 0xd8, 0, 0xde],
            vec![0xfe, 0xff, 0, 65],
            (0..=255).collect(),
        ] {
            let (text, encoding) = decode(&data).unwrap();
            let snapshot = Snapshot {
                newline: "\n",
                text: Rope::from_str(&text),
                encoding,
                revision: 0,
            };
            let mut output = Vec::new();
            write_snapshot(&snapshot, &mut output).unwrap();
            assert_eq!(data, output);
        }
    }
    #[test]
    fn view_changes_preserve_bytes_and_dirty_state() {
        let mut doc = Document::default();
        doc.replace(0..0, "é\r\n").unwrap();
        let revision = doc.revision;
        doc.set_view(ViewMode::Hex).unwrap();
        assert_eq!(doc.text.len_chars(), 4);
        doc.set_view(ViewMode::Utf8).unwrap();
        assert_eq!(doc.text.to_string(), "é\r\n");
        assert_eq!(doc.revision, revision);
    }
    #[test]
    fn atomic_save_detects_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text");
        fs::write(&path, "original").unwrap();
        let doc = Document::open(path.clone()).unwrap();
        fs::write(&path, "external edit").unwrap();
        assert!(save(&doc.snapshot(), &path, doc.stamp.as_ref()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "external edit");
    }
    #[test]
    fn ascii_encoding_is_independent_from_markdown() {
        let mut doc = Document::default();
        doc.replace(0..0, "# Heading").unwrap();
        doc.language = Language::Markdown;
        doc.markdown = MarkdownMode::Hybrid;
        doc.set_view(ViewMode::Ascii).unwrap();
        assert_eq!(doc.encoding, Encoding::Ascii);
        assert_eq!(doc.markdown_mode(), MarkdownMode::Hybrid);
        assert!(doc.replace(0..0, "é").is_err());
        let mut bytes = Vec::new();
        write_snapshot(&doc.snapshot(), &mut bytes).unwrap();
        assert_eq!(bytes, b"# Heading");
        doc.set_view(ViewMode::Hex).unwrap();
        assert_eq!(doc.encoding, Encoding::Bytes);
        assert_eq!(doc.markdown_mode(), MarkdownMode::Raw);
    }
    #[test]
    fn malformed_unicode_signatures_stay_editable_as_bytes() {
        for data in [vec![0xff, 0xfe, 0], vec![0xff, 0xfe, 0, 0xd8]] {
            let (text, encoding) = decode(&data).unwrap();
            assert_eq!(encoding, Encoding::Bytes);
            let mut output = Vec::new();
            let doc = Document {
                text: Rope::from_str(&text),
                encoding,
                ..Document::default()
            };
            write_snapshot(&doc.snapshot(), &mut output).unwrap();
            assert_eq!(output, data);
        }
    }
    #[test]
    fn byte_patterns_validate_complete_values() {
        assert_eq!(
            parse_bytes("00 FF 41", 16)
                .unwrap()
                .chars()
                .map(|c| c as u8)
                .collect::<Vec<_>>(),
            vec![0, 255, 65]
        );
        assert_eq!(parse_bytes("01000001 01000010", 2).unwrap(), "AB");
        assert!(parse_bytes("F", 16).is_err());
        assert!(parse_bytes("é", 16).is_err());
    }
}
