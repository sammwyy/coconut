use creamui_image::ImageData;
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 80_000_000;
pub const MAX_DIRECTORY_IMAGES: usize = 10_000;
pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico",
];

pub struct Asset {
    pub path: PathBuf,
    pub data: ImageData,
    pub metadata: Metadata,
}
pub struct Metadata {
    pub format: String,
    pub color: String,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    pub original_dimensions: (u32, u32),
    pub orientation: String,
    pub has_alpha: bool,
    pub icc_bytes: usize,
    pub exif: Vec<(String, String)>,
    pub warnings: Vec<String>,
}
impl Asset {
    pub fn size(&self) -> creamui_core::Size {
        creamui_core::Size {
            width: self.data.width() as f32,
            height: self.data.height() as f32,
        }
    }
}

pub fn load(path: &Path) -> Result<Asset, String> {
    let mut file =
        File::open(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let stat = file
        .metadata()
        .map_err(|error| format!("Cannot read file information: {error}"))?;
    if !stat.is_file() {
        return Err("Choose an image file".into());
    }
    if stat.len() > MAX_FILE_BYTES {
        return Err("Image exceeds the 128 MiB file limit".into());
    }
    let mut bytes = Vec::with_capacity(stat.len() as usize);
    file.by_ref()
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read image: {error}"))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Image exceeds the 128 MiB file limit".into());
    }
    decode(path, bytes, stat.modified().ok())
}
fn decode(path: &Path, bytes: Vec<u8>, modified: Option<SystemTime>) -> Result<Asset, String> {
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| format!("Cannot identify image: {error}"))?;
    let format = reader.format().ok_or("Unrecognized image format")?;
    if !matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Jpeg
            | ImageFormat::WebP
            | ImageFormat::Gif
            | ImageFormat::Bmp
            | ImageFormat::Tiff
            | ImageFormat::Ico
    ) {
        return Err(format!("Unsupported format: {format:?}"));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("Cannot decode image: {error}"))?;
    let dimensions = decoder.dimensions();
    if dimensions.0 as u64 * dimensions.1 as u64 > MAX_PIXELS {
        return Err("Image exceeds the 80 megapixel limit".into());
    }
    let color = decoder.original_color_type();
    let has_alpha = decoder.color_type().has_alpha();
    let mut warnings = vec![];
    let (orientation, orientation_name) = match decoder.orientation() {
        Ok(orientation) => {
            use image::metadata::Orientation;
            let name = match orientation {
                Orientation::NoTransforms => "Normal",
                Orientation::Rotate90 => "90° clockwise (applied)",
                Orientation::Rotate180 => "180° (applied)",
                Orientation::Rotate270 => "90° counterclockwise (applied)",
                Orientation::FlipHorizontal => "Horizontal mirror (applied)",
                Orientation::FlipVertical => "Vertical mirror (applied)",
                Orientation::Rotate90FlipH => "90° clockwise and mirrored (applied)",
                Orientation::Rotate270FlipH => "90° counterclockwise and mirrored (applied)",
            };
            (orientation, name)
        }
        Err(error) => {
            warnings.push(format!("Orientation: {error}"));
            (image::metadata::Orientation::NoTransforms, "Unavailable")
        }
    };
    let icc_bytes = match decoder.icc_profile() {
        Ok(profile) => profile.map_or(0, |value| value.len()),
        Err(error) => {
            warnings.push(format!("ICC metadata: {error}"));
            0
        }
    };
    let exif = match decoder.exif_metadata() {
        Ok(Some(bytes)) => exif_fields(bytes, &mut warnings),
        Ok(None) => vec![],
        Err(error) => {
            warnings.push(format!("EXIF metadata: {error}"));
            vec![]
        }
    };
    let mut decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Cannot decode image pixels: {error}"))?;
    decoded.apply_orientation(orientation);
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();
    let data = ImageData::from_rgba(width, height, rgba.into_raw())
        .map_err(|error| format!("Cannot prepare image: {error}"))?;
    for warning in &warnings {
        eprintln!("cviewer: {}: {warning}", path.display());
    }
    Ok(Asset {
        path: path.into(),
        data,
        metadata: Metadata {
            format: format!("{format:?}").to_uppercase(),
            color: format!("{color:?}").to_uppercase(),
            bytes: bytes.len() as u64,
            modified,
            original_dimensions: dimensions,
            orientation: orientation_name.into(),
            has_alpha,
            icc_bytes,
            exif,
            warnings,
        },
    })
}
fn exif_fields(bytes: Vec<u8>, warnings: &mut Vec<String>) -> Vec<(String, String)> {
    if bytes.len() > 4 * 1024 * 1024 {
        warnings.push("EXIF metadata exceeds 4 MiB; fields were not parsed".into());
        return vec![];
    }
    let bytes = bytes
        .strip_prefix(b"Exif\0\0")
        .map_or_else(|| bytes.clone(), |value| value.to_vec());
    match exif::Reader::new().read_raw(bytes) {
        Ok(exif) => exif
            .fields()
            .filter(|field| field.ifd_num == exif::In::PRIMARY)
            .take(128)
            .map(|field| {
                let value = if let exif::Value::Undefined(bytes, _) = &field.value {
                    format!("Binary metadata ({} bytes)", bytes.len())
                } else {
                    field.display_value().with_unit(&exif).to_string()
                };
                (field.tag.to_string(), value.chars().take(512).collect())
            })
            .collect(),
        Err(error) => {
            warnings.push(format!("EXIF fields could not be read: {error}"));
            vec![]
        }
    }
}
pub fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            EXTENSIONS
                .iter()
                .any(|extension| value.eq_ignore_ascii_case(extension))
        })
}
pub fn neighbors(path: &Path) -> Result<Vec<PathBuf>, String> {
    let parent = path.parent().ok_or("Image has no parent directory")?;
    let mut paths = vec![];
    for entry in std::fs::read_dir(parent)
        .map_err(|error| format!("Cannot list neighboring images: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Cannot read directory entry: {error}"))?;
        let file = entry.path();
        if supported(&file) && file.is_file() {
            if paths.len() == MAX_DIRECTORY_IMAGES {
                return Err(
                    "Directory contains more than 10000 images; open a selection instead".into(),
                );
            }
            paths.push(file);
        }
    }
    if !paths.iter().any(|value| value == path) {
        if paths.len() == MAX_DIRECTORY_IMAGES {
            return Err(
                "Directory contains more than 10000 images; open a selection instead".into(),
            );
        }
        paths.push(path.into());
    }
    paths.sort_by_cached_key(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    Ok(paths)
}
pub fn absolute(path: PathBuf) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = path
        .parent()
        .ok_or("Image has no parent directory")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    Ok(parent.join(path.file_name().ok_or("Choose an image file")?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_pixels_information_and_neighbors_without_modifying_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo ñ.png");
        let image = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 128, 0, 255, 0, 255]).unwrap();
        image.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let asset = load(&path).unwrap();
        assert_eq!((asset.data.width(), asset.data.height()), (2, 1));
        assert_eq!(&asset.data.pixels()[..4], &[128, 0, 0, 128]);
        assert!(asset.metadata.has_alpha);
        assert_eq!(asset.metadata.format, "PNG");
        assert!(asset.metadata.exif.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(directory.path().join("z.jpg"), b"broken").unwrap();
        std::fs::write(directory.path().join("ignore.txt"), b"text").unwrap();
        assert_eq!(neighbors(&path).unwrap().len(), 2);
        assert!(load(&directory.path().join("z.jpg")).is_err());
        assert!(load(directory.path()).is_err());
    }
    #[test]
    fn malformed_exif_is_reported_and_large_files_are_rejected() {
        let mut warnings = vec![];
        assert!(exif_fields(vec![0; 32], &mut warnings).is_empty());
        assert_eq!(warnings.len(), 1);
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(load(file.path()).err().unwrap().contains("128 MiB"));
    }
    #[test]
    fn jpeg_exif_information_and_orientation_are_applied_before_display() {
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode(
                &[255, 0, 0, 0, 255, 0],
                2,
                1,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let mut tiff = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
        tiff.extend_from_slice(&2_u16.to_le_bytes());
        tiff.extend_from_slice(&0x010f_u16.to_le_bytes());
        tiff.extend_from_slice(&2_u16.to_le_bytes());
        tiff.extend_from_slice(&8_u32.to_le_bytes());
        tiff.extend_from_slice(&38_u32.to_le_bytes());
        tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
        tiff.extend_from_slice(&3_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&6_u32.to_le_bytes());
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        tiff.extend_from_slice(b"Coconut\0");
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(tiff);
        let mut bytes = jpeg[..2].to_vec();
        bytes.extend_from_slice(&[0xff, 0xe1]);
        bytes.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        bytes.extend(payload);
        bytes.extend_from_slice(&jpeg[2..]);
        let asset = decode(Path::new("camera.jpg"), bytes, None).unwrap();
        assert_eq!((asset.data.width(), asset.data.height()), (1, 2));
        assert_eq!(asset.metadata.original_dimensions, (2, 1));
        assert!(asset
            .metadata
            .exif
            .iter()
            .any(|(name, value)| name == "Make" && value.contains("Coconut")));
    }
}
