use image_rs::{ImageFormat, ImageReader};
use ring::{
    aead, hmac,
    rand::{SecureRandom, SystemRandom},
};
use std::{
    env, fs,
    io::{self, Cursor, Read, Write},
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions, StandardVisualKey, Visual},
    probe::Hint,
};

const MAX_SIDE: u32 = 512;
const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024;

pub struct Thumbnailer {
    cache: PathBuf,
    key: [u8; 32],
}

impl Thumbnailer {
    pub fn new() -> io::Result<Self> {
        let cache_root = xdg_dir("XDG_CACHE_HOME", ".cache")?;
        let state_root = xdg_dir("XDG_STATE_HOME", ".local/state")?;
        Self::at(
            cache_root.join("coconut/thumbnails"),
            state_root.join("coconut/thumbnails.key"),
        )
    }

    pub fn at(cache: PathBuf, key_path: PathBuf) -> io::Result<Self> {
        private_dir(&cache)?;
        private_dir(
            key_path
                .parent()
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?,
        )?;
        let key = match fs::read(&key_path) {
            Ok(bytes) => bytes
                .try_into()
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut bytes = [0; 32];
                SystemRandom::new().fill(&mut bytes).map_err(crypto_error)?;
                let parent = key_path.parent().expect("key parent");
                let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
                temporary
                    .as_file()
                    .set_permissions(fs::Permissions::from_mode(0o600))?;
                temporary.write_all(&bytes)?;
                match temporary.persist_noclobber(&key_path) {
                    Ok(_) => bytes,
                    Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                        fs::read(&key_path)?
                            .try_into()
                            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?
                    }
                    Err(error) => return Err(error.error),
                }
            }
            Err(error) => return Err(error),
        };
        Ok(Self { cache, key })
    }

    pub fn supports(path: &Path) -> bool {
        matches!(
            extension(path).as_deref(),
            Some(
                "png"
                    | "jpg"
                    | "jpeg"
                    | "webp"
                    | "gif"
                    | "bmp"
                    | "ico"
                    | "svg"
                    | "tif"
                    | "tiff"
                    | "mp4"
                    | "mkv"
                    | "webm"
                    | "mov"
                    | "avi"
                    | "mp3"
                    | "m4a"
                    | "flac"
                    | "ogg"
                    | "opus"
                    | "wma"
                    | "wav"
                    | "aac"
                    | "exe"
            )
        )
    }

    pub fn get(&self, path: &Path, side: u32) -> io::Result<Option<Vec<u8>>> {
        if !Self::supports(path) || side == 0 {
            return Ok(None);
        }
        let absolute = fs::canonicalize(path)?;
        let metadata = fs::metadata(&absolute)?;
        if !metadata.is_file() {
            return Ok(None);
        }
        let side = side.min(MAX_SIDE);
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let mut identity = Vec::from(absolute.as_os_str().as_bytes());
        identity.extend_from_slice(&metadata.len().to_le_bytes());
        identity.extend_from_slice(&modified.as_nanos().to_le_bytes());
        identity.extend_from_slice(&side.to_le_bytes());
        let digest = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &self.key), &identity);
        let name: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        let destination = self.cache.join(name);
        if let Ok(file) = fs::File::open(&destination) {
            if file.metadata()?.len() <= MAX_CACHE_BYTES + 28 {
                let mut encrypted = Vec::new();
                file.take(MAX_CACHE_BYTES + 28)
                    .read_to_end(&mut encrypted)?;
                if let Ok(plain) = self.decrypt(&encrypted, digest.as_ref()) {
                    return Ok(Some(plain));
                }
            }
        }
        let Some(png) = generate(&absolute, side)? else {
            return Ok(None);
        };
        if png.len() as u64 <= MAX_CACHE_BYTES {
            let encrypted = self.encrypt(&png, digest.as_ref())?;
            let mut temporary = tempfile::NamedTempFile::new_in(&self.cache)?;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
            temporary.write_all(&encrypted)?;
            temporary.persist(&destination).map_err(|e| e.error)?;
        }
        Ok(Some(png))
    }

    fn encrypt(&self, plain: &[u8], identity: &[u8]) -> io::Result<Vec<u8>> {
        let mut nonce = [0; 12];
        SystemRandom::new().fill(&mut nonce).map_err(crypto_error)?;
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.key).map_err(crypto_error)?;
        let key = aead::LessSafeKey::new(key);
        let mut ciphertext = plain.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(identity),
            &mut ciphertext,
        )
        .map_err(crypto_error)?;
        let mut output = nonce.to_vec();
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    fn decrypt(&self, encrypted: &[u8], identity: &[u8]) -> io::Result<Vec<u8>> {
        let (nonce, ciphertext) = encrypted
            .split_at_checked(12)
            .ok_or_else(|| crypto_error("short cache entry"))?;
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.key).map_err(crypto_error)?;
        let mut output = ciphertext.to_vec();
        let plain = aead::LessSafeKey::new(key)
            .open_in_place(
                aead::Nonce::try_assume_unique_for_key(nonce).map_err(crypto_error)?,
                aead::Aad::from(identity),
                &mut output,
            )
            .map_err(crypto_error)?;
        let len = plain.len();
        output.truncate(len);
        Ok(output)
    }
}

fn generate(path: &Path, side: u32) -> io::Result<Option<Vec<u8>>> {
    let ext = extension(path);
    if ext.as_deref() == Some("svg") {
        return svg_preview(path, side).map(Some);
    }
    let image = if matches!(
        ext.as_deref(),
        Some(
            "mp4"
                | "mkv"
                | "webm"
                | "mov"
                | "avi"
                | "mp3"
                | "m4a"
                | "flac"
                | "ogg"
                | "opus"
                | "wma"
                | "wav"
                | "aac"
        )
    ) {
        let audio = matches!(
            ext.as_deref(),
            Some("mp3" | "m4a" | "flac" | "ogg" | "opus" | "wma" | "wav" | "aac")
        );
        let Some(frame) = (if audio {
            match audio_cover(path)? {
                Some(cover) => Some(cover),
                None => ffmpeg_frame(path, None, "ffmpeg")?,
            }
        } else {
            video_frame(path)?
        }) else {
            return Ok(None);
        };
        image_rs::load_from_memory(&frame).map_err(io::Error::other)?
    } else if ext.as_deref() == Some("exe") {
        let Some(icon) = pe_icon(path)? else {
            return Ok(None);
        };
        image_rs::load_from_memory(&icon).map_err(io::Error::other)?
    } else {
        let mut reader = ImageReader::open(path)
            .map_err(io::Error::other)?
            .with_guessed_format()
            .map_err(io::Error::other)?;
        let mut limits = image_rs::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(16_000);
        limits.max_image_height = Some(16_000);
        reader.limits(limits);
        reader.decode().map_err(io::Error::other)?
    };
    let thumb = image.thumbnail(side, side);
    let mut png = Cursor::new(Vec::new());
    thumb
        .write_to(&mut png, ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(Some(png.into_inner()))
}

fn audio_cover(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let source = fs::File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    let options = MetadataOptions {
        limit_metadata_bytes: Limit::Maximum(4 * 1024 * 1024),
        limit_visual_bytes: Limit::Maximum(MAX_CACHE_BYTES as usize),
    };
    let Ok(mut probed) =
        symphonia::default::get_probe().format(&hint, stream, &FormatOptions::default(), &options)
    else {
        return Ok(None);
    };
    let from_probe = probed.metadata.get().and_then(|mut metadata| {
        metadata
            .skip_to_latest()
            .and_then(|revision| choose_cover(revision.visuals()))
    });
    if from_probe.is_some() {
        return Ok(from_probe);
    }
    let mut metadata = probed.format.metadata();
    Ok(metadata
        .skip_to_latest()
        .and_then(|revision| choose_cover(revision.visuals())))
}

fn choose_cover(visuals: &[Visual]) -> Option<Vec<u8>> {
    visuals
        .iter()
        .filter(|visual| visual.data.len() as u64 <= MAX_CACHE_BYTES)
        .filter(|visual| visual.media_type.starts_with("image/"))
        .max_by_key(|visual| (visual.usage == Some(StandardVisualKey::FrontCover)) as u8)
        .map(|visual| visual.data.to_vec())
}

fn svg_preview(path: &Path, side: u32) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_CACHE_BYTES {
        return Err(io::Error::other("SVG exceeds thumbnail input limit"));
    }
    let mut data = Vec::new();
    file.take(MAX_CACHE_BYTES).read_to_end(&mut data)?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())
        .map_err(io::Error::other)?;
    let size = tree.size();
    let scale = side as f32 / size.width().max(size.height());
    let width = ((size.width() * scale).round() as u32).max(1);
    let height = ((size.height() * scale).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| io::Error::other("invalid SVG dimensions"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            width as f32 / size.width(),
            height as f32 / size.height(),
        ),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(io::Error::other)
}

fn pe_icon(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > 128 * 1024 * 1024 {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(128 * 1024 * 1024).read_to_end(&mut bytes)?;
    let Some(pe) = number(&bytes, 0x3c, 4).map(|value| value as usize) else {
        return Ok(None);
    };
    if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Ok(None);
    }
    let sections = number(&bytes, pe + 6, 2).unwrap_or(0) as usize;
    let optional = pe + 24;
    let optional_size = number(&bytes, pe + 20, 2).unwrap_or(0) as usize;
    let directory_offset = match number(&bytes, optional, 2) {
        Some(0x10b) => 96,
        Some(0x20b) => 112,
        _ => return Ok(None),
    };
    let resource_rva = number(&bytes, optional + directory_offset + 16, 4).unwrap_or(0) as usize;
    if resource_rva == 0 {
        return Ok(None);
    }
    let section_start = optional + optional_size;
    let rva_offset = |rva: usize| -> Option<usize> {
        (0..sections.min(96)).find_map(|index| {
            let offset = section_start.checked_add(index * 40)?;
            let virtual_size = number(&bytes, offset + 8, 4)? as usize;
            let virtual_address = number(&bytes, offset + 12, 4)? as usize;
            let raw_size = number(&bytes, offset + 16, 4)? as usize;
            let raw_offset = number(&bytes, offset + 20, 4)? as usize;
            let relative = rva.checked_sub(virtual_address)?;
            (relative < virtual_size.max(raw_size) && relative < raw_size)
                .then(|| raw_offset.checked_add(relative))
                .flatten()
                .filter(|&position| position < bytes.len())
        })
    };
    let Some(base) = rva_offset(resource_rva) else {
        return Ok(None);
    };
    let mut entries = vec![0usize];
    for level in 0..3 {
        let mut next = Vec::new();
        for directory in entries {
            let Some(start) = base.checked_add(directory) else {
                continue;
            };
            let count = (number(&bytes, start + 12, 2).unwrap_or(0)
                + number(&bytes, start + 14, 2).unwrap_or(0))
            .min(1024);
            for index in 0..count as usize {
                let entry = start + 16 + index * 8;
                let Some(name) = number(&bytes, entry, 4) else {
                    continue;
                };
                let Some(target) = number(&bytes, entry + 4, 4) else {
                    continue;
                };
                if level == 0 && name != 3 {
                    continue;
                }
                if level < 2 && target & 0x8000_0000 != 0 {
                    next.push((target & 0x7fff_ffff) as usize);
                } else if level == 2 && target & 0x8000_0000 == 0 {
                    next.push(target as usize);
                }
            }
        }
        entries = next;
    }
    let icon = entries
        .into_iter()
        .filter_map(|offset| {
            let header = base.checked_add(offset)?;
            let rva = number(&bytes, header, 4)? as usize;
            let length = number(&bytes, header + 4, 4)? as usize;
            if !(24..=4 * 1024 * 1024).contains(&length) {
                return None;
            }
            let start = rva_offset(rva)?;
            let data = bytes.get(start..start.checked_add(length)?)?;
            if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                let width = u32::from_be_bytes(data.get(16..20)?.try_into().ok()?);
                let height = u32::from_be_bytes(data.get(20..24)?.try_into().ok()?);
                return (width > 0 && height > 0 && width <= 256 && height <= 256)
                    .then_some((width * height, data));
            }
            if length < 40 {
                return None;
            }
            let width = number(data, 4, 4)?;
            let height = number(data, 8, 4)? / 2;
            (width > 0 && height > 0 && width <= 256 && height <= 256)
                .then_some((width * height, data))
        })
        .max_by_key(|(area, _)| *area);
    let Some((_, data)) = icon else {
        return Ok(None);
    };
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(Some(data.to_vec()));
    }
    let width = number(data, 4, 4).unwrap_or(0);
    let height = number(data, 8, 4).unwrap_or(0) / 2;
    let mut ico = Vec::with_capacity(22 + data.len());
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    ico.push(if width == 256 { 0 } else { width as u8 });
    ico.push(if height == 256 { 0 } else { height as u8 });
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(number(data, 14, 2).unwrap_or(32) as u16).to_le_bytes());
    ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(data);
    Ok(Some(ico))
}

fn number(data: &[u8], offset: usize, len: usize) -> Option<u32> {
    let slice = data.get(offset..offset.checked_add(len)?)?;
    Some(
        slice
            .iter()
            .enumerate()
            .fold(0, |value, (i, byte)| value | (*byte as u32) << (8 * i)),
    )
}

fn video_frame(path: &Path) -> io::Result<Option<Vec<u8>>> {
    video_frame_with(path, "ffmpeg")
}

fn video_frame_with(path: &Path, ffmpeg: &str) -> io::Result<Option<Vec<u8>>> {
    for seek in ["1", "0"] {
        match ffmpeg_frame(path, Some(seek), ffmpeg) {
            Ok(Some(frame)) => return Ok(Some(frame)),
            Ok(None) => {}
            Err(error) => {
                eprintln!(
                    "coconut-thumbnails: ffmpeg could not decode {}: {error}",
                    path.display()
                );
                break;
            }
        }
    }
    gstreamer_frame(path)
}

fn gstreamer_frame(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let uri = url::Url::from_file_path(path)
        .map_err(|()| io::Error::new(io::ErrorKind::InvalidInput, "invalid video path"))?;
    let child = match Command::new("gst-launch-1.0")
        .args(["-q", "uridecodebin"])
        .arg(format!("uri={uri}"))
        .args([
            "!",
            "videoconvert",
            "!",
            "videoscale",
            "add-borders=true",
            "!",
            "video/x-raw,width=[1,512],height=[1,512],pixel-aspect-ratio=1/1",
            "!",
            "pngenc",
            "snapshot=true",
            "!",
            "fdsink",
            "fd=1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    read_frame(child)
}

fn ffmpeg_frame(path: &Path, seek: Option<&str>, binary: &str) -> io::Result<Option<Vec<u8>>> {
    let mut command = Command::new(binary);
    command.args([
        "-v",
        "error",
        "-nostdin",
        "-protocol_whitelist",
        "file,pipe",
    ]);
    if let Some(seek) = seek {
        command.args(["-ss", seek]);
    }
    command.arg("-i").arg(path);
    if seek.is_none() {
        command.args(["-map", "0:v:0?", "-an"]);
    }
    let child = match command
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=512:512:force_original_aspect_ratio=decrease",
            "-f",
            "image2",
            "-vcodec",
            "png",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    read_frame(child)
}

fn read_frame(mut child: Child) -> io::Result<Option<Vec<u8>>> {
    let stdout = child.stdout.take().expect("piped stdout");
    let reader = thread::spawn(move || {
        let mut frame = Vec::new();
        stdout
            .take(MAX_CACHE_BYTES + 1)
            .read_to_end(&mut frame)
            .map(|_| frame)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break (child.wait()?, true);
        }
        thread::sleep(Duration::from_millis(20));
    };
    let frame = reader
        .join()
        .map_err(|_| io::Error::other("frame reader panicked"))??;
    Ok((!timed_out
        && status.success()
        && !frame.is_empty()
        && frame.len() as u64 <= MAX_CACHE_BYTES)
        .then_some(frame))
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

fn xdg_dir(variable: &str, fallback: &str) -> io::Result<PathBuf> {
    if let Some(value) = env::var_os(variable).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    Ok(
        PathBuf::from(env::var_os("HOME").ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?)
            .join(fallback),
    )
}

fn private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn crypto_error(_: impl std::fmt::Debug) -> io::Error {
    io::Error::other("thumbnail encryption failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_private_and_invalidates_on_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("picture.png");
        image_rs::RgbaImage::new(10, 10).save(&path).unwrap();
        let cache = dir.path().join("cache");
        let key = dir.path().join("state/key");
        let generator = Thumbnailer::at(cache.clone(), key.clone()).unwrap();
        let first = generator.get(&path, 32).unwrap().unwrap();
        assert_eq!(generator.get(&path, 32).unwrap(), Some(first.clone()));
        let encrypted = fs::read(
            fs::read_dir(&cache)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(!encrypted.windows(first.len()).any(|window| window == first));
        assert_eq!(
            fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&key).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            Thumbnailer::at(cache.clone(), key)
                .unwrap()
                .get(&path, 32)
                .unwrap(),
            Some(first.clone())
        );
        image_rs::RgbaImage::from_pixel(20, 20, image_rs::Rgba([255, 30, 15, 255]))
            .save(&path)
            .unwrap();
        assert_ne!(generator.get(&path, 32).unwrap(), Some(first));
    }

    #[test]
    fn tampered_cache_is_regenerated() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("test.png");
        image_rs::RgbaImage::new(2, 2).save(&source).unwrap();
        let cache = dir.path().join("cache");
        let generator = Thumbnailer::at(cache.clone(), dir.path().join("key")).unwrap();
        let original = generator.get(&source, 64).unwrap().unwrap();
        let cached = fs::read_dir(cache).unwrap().next().unwrap().unwrap().path();
        fs::write(&cached, b"invalid").unwrap();
        assert_eq!(generator.get(&source, 64).unwrap(), Some(original));
        assert!(generator
            .get(Path::new("not-supported.txt"), 64)
            .unwrap()
            .is_none());
    }

    #[test]
    fn cache_entries_cannot_be_swapped_between_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let generator = Thumbnailer::at(cache.clone(), dir.path().join("key")).unwrap();
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.png");
        image_rs::RgbaImage::from_pixel(2, 2, image_rs::Rgba([255, 0, 0, 255]))
            .save(&first)
            .unwrap();
        image_rs::RgbaImage::from_pixel(2, 2, image_rs::Rgba([0, 0, 255, 255]))
            .save(&second)
            .unwrap();
        let expected = generator.get(&second, 32).unwrap().unwrap();
        let first_bytes = generator.get(&first, 32).unwrap().unwrap();
        let cached: Vec<_> = fs::read_dir(cache)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let left = fs::read(&cached[0]).unwrap();
        let right = fs::read(&cached[1]).unwrap();
        fs::write(&cached[0], right).unwrap();
        fs::write(&cached[1], left).unwrap();
        assert_eq!(generator.get(&first, 32).unwrap(), Some(first_bytes));
        assert_eq!(generator.get(&second, 32).unwrap(), Some(expected));
    }

    #[test]
    fn extracts_pe_icon_resource() {
        let dir = tempfile::tempdir().unwrap();
        let icon_path = dir.path().join("icon.png");
        image_rs::RgbaImage::new(16, 16).save(&icon_path).unwrap();
        let png = fs::read(icon_path).unwrap();
        let mut pe = vec![0u8; 0x600];
        pe[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        pe[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
        pe[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        pe[0x98 + 112..0x98 + 116].copy_from_slice(&0x1000u32.to_le_bytes());
        let section = 0x98 + 0xe0;
        pe[section + 8..section + 12].copy_from_slice(&0x400u32.to_le_bytes());
        pe[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        pe[section + 16..section + 20].copy_from_slice(&0x400u32.to_le_bytes());
        pe[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        for (base, name, target) in [
            (0x200, 3u32, 0x8000_0040u32),
            (0x240, 1, 0x8000_0080),
            (0x280, 0x409, 0xc0),
        ] {
            pe[base + 14..base + 16].copy_from_slice(&1u16.to_le_bytes());
            pe[base + 16..base + 20].copy_from_slice(&name.to_le_bytes());
            pe[base + 20..base + 24].copy_from_slice(&target.to_le_bytes());
        }
        pe[0x2c0..0x2c4].copy_from_slice(&0x1100u32.to_le_bytes());
        pe[0x2c4..0x2c8].copy_from_slice(&(png.len() as u32).to_le_bytes());
        pe[0x300..0x300 + png.len()].copy_from_slice(&png);
        let source = dir.path().join("program.exe");
        fs::write(&source, pe).unwrap();
        let generator = Thumbnailer::at(dir.path().join("cache"), dir.path().join("key")).unwrap();
        let result = generator.get(&source, 32).unwrap().unwrap();
        assert!(result.starts_with(b"\x89PNG"));
    }

    #[test]
    fn decodes_classic_pe_dib_icon() {
        let mut dib = vec![0u8; 48];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1u32.to_le_bytes());
        dib[8..12].copy_from_slice(&2u32.to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[40..44].copy_from_slice(&[10, 20, 255, 255]);
        let mut ico = vec![0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 32, 0];
        ico.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes());
        ico.extend_from_slice(&dib);
        let decoded = image_rs::load_from_memory(&ico).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1, 1));
    }

    #[test]
    fn scales_svg_into_png() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("vector.svg");
        fs::write(
            &source,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="red"/></svg>"#,
        ).unwrap();
        let generator = Thumbnailer::at(dir.path().join("cache"), dir.path().join("key")).unwrap();
        let png = generator.get(&source, 40).unwrap().unwrap();
        let decoded = image_rs::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (40, 20));
    }

    #[test]
    fn extracts_short_video_frame_when_ffmpeg_is_available() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("short !% clip.mkv");
        let result = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=96x48:r=2",
                "-frames:v",
                "1",
                "-c:v",
                "ffv1",
                "-y",
            ])
            .arg(&source)
            .status();
        if matches!(result, Err(ref error) if error.kind() == io::ErrorKind::NotFound) {
            return;
        }
        assert!(result.unwrap().success());
        let generator = Thumbnailer::at(dir.path().join("cache"), dir.path().join("key")).unwrap();
        assert!(generator
            .get(&source, 32)
            .unwrap()
            .unwrap()
            .starts_with(b"\x89PNG"));
        if Command::new("gst-inspect-1.0")
            .arg("avdec_ffv1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            let frame = gstreamer_frame(&source).unwrap().unwrap();
            let image = image_rs::load_from_memory(&frame).unwrap();
            assert_eq!(image.width(), image.height() * 2);
            assert!(image.width() <= 512);
            assert!(video_frame_with(&source, "/no-such-ffmpeg")
                .unwrap()
                .unwrap()
                .starts_with(b"\x89PNG"));
        }
    }

    #[test]
    fn embedded_audio_artwork_and_missing_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("song.mp3");
        let status = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=d=1",
                "-c:a",
                "libmp3lame",
                "-y",
            ])
            .arg(&audio)
            .status();
        if matches!(status, Err(ref error) if error.kind() == io::ErrorKind::NotFound) {
            return;
        }
        assert!(status.unwrap().success());
        let generator = Thumbnailer::at(dir.path().join("cache"), dir.path().join("key")).unwrap();
        assert!(generator.get(&audio, 40).unwrap().is_none());
        let cover = dir.path().join("cover.png");
        image_rs::RgbaImage::from_pixel(32, 32, image_rs::Rgba([255, 70, 10, 255]))
            .save(&cover)
            .unwrap();
        let with_art = dir.path().join("with-art.mp3");
        assert!(Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&audio)
            .arg("-i")
            .arg(&cover)
            .args([
                "-map",
                "0:a",
                "-map",
                "1:v",
                "-c:a",
                "copy",
                "-c:v",
                "png",
                "-disposition:v",
                "attached_pic",
                "-y"
            ])
            .arg(&with_art)
            .status()
            .unwrap()
            .success());
        assert!(audio_cover(&with_art).unwrap().is_some());
        assert!(generator
            .get(&with_art, 40)
            .unwrap()
            .unwrap()
            .starts_with(b"\x89PNG"));
    }
}
