use crate::image::Shot;
use dbus::{
    arg::{OwnedFd, RefArg, Variant},
    blocking::Connection,
};
use std::{
    collections::HashMap,
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd},
    time::{Duration, Instant},
};

type Options = HashMap<String, Variant<Box<dyn RefArg>>>;
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CAPTURE_BYTES: usize = 512 * 1024 * 1024;

fn pipe() -> Result<(std::fs::File, OwnedFd), String> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        return Err(format!(
            "Could not create screenshot pipe: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(unsafe { (std::fs::File::from_raw_fd(fds[0]), OwnedFd::new(fds[1])) })
}

fn number(metadata: &Options, key: &str) -> Result<usize, String> {
    metadata
        .get(key)
        .and_then(|value| value.0.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("KWin returned an invalid {key}"))
}

fn dimensions(metadata: &Options) -> Result<(usize, usize, usize, usize), String> {
    let (width, height, stride, format) = (
        number(metadata, "width")?,
        number(metadata, "height")?,
        number(metadata, "stride")?,
        number(metadata, "format")?,
    );
    let channels = match format {
        4..=6 | 16..=18 => 4,
        13 => 3,
        _ => return Err(format!("Unsupported KWin image format {format}")),
    };
    let size = stride
        .checked_mul(height)
        .ok_or("KWin screenshot is too large")?;
    if width == 0
        || height == 0
        || width > u32::MAX as usize
        || height > u32::MAX as usize
        || width.checked_mul(channels).is_none_or(|row| stride < row)
        || size > MAX_CAPTURE_BYTES
        || width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .is_none_or(|size| size > MAX_CAPTURE_BYTES)
    {
        return Err("KWin returned invalid screenshot dimensions".into());
    }
    Ok((width, height, stride, format))
}

fn read_pixels(mut pipe: std::fs::File, size: usize, timeout: Duration) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + timeout;
    let mut pixels = vec![0; size];
    let mut offset = 0;
    while offset < size {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Timed out reading the KWin screenshot".into());
        }
        let mut poll = libc::pollfd {
            fd: pipe.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe {
            libc::poll(
                &mut poll,
                1,
                remaining.as_millis().max(1).min(i32::MAX as u128) as i32,
            )
        };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("Could not wait for the KWin screenshot: {error}"));
        }
        if ready == 0 {
            return Err("Timed out reading the KWin screenshot".into());
        }
        match pipe.read(&mut pixels[offset..]) {
            Ok(0) => {
                return Err(format!(
                    "KWin closed the screenshot pipe after {offset} of {size} bytes"
                ))
            }
            Ok(read) => offset += read,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) =>
            {
                continue
            }
            Err(error) => return Err(format!("Could not read the KWin screenshot: {error}")),
        }
    }
    Ok(pixels)
}

fn decode(metadata: &Options, contents: &[u8]) -> Result<Shot, String> {
    if metadata.get("type").and_then(|value| value.0.as_str()) != Some("raw") {
        return Err("KWin returned an unsupported screenshot type".into());
    }
    let (width, height, stride, format) = dimensions(metadata)?;
    if contents.len() != stride * height {
        return Err("KWin returned a truncated screenshot".into());
    }
    let mut pixels = vec![0; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let channels = if format == 13 { 3 } else { 4 };
            let from = y * stride + x * channels;
            let (r, g, b, a) = match format {
                4..=6 => {
                    let pixel = u32::from_ne_bytes(contents[from..from + 4].try_into().unwrap());
                    (
                        (pixel >> 16) as u8,
                        (pixel >> 8) as u8,
                        pixel as u8,
                        if format == 4 {
                            255
                        } else {
                            (pixel >> 24) as u8
                        },
                    )
                }
                13 => (contents[from], contents[from + 1], contents[from + 2], 255),
                16..=18 => (
                    contents[from],
                    contents[from + 1],
                    contents[from + 2],
                    if format == 16 {
                        255
                    } else {
                        contents[from + 3]
                    },
                ),
                _ => unreachable!(),
            };
            let straight = matches!(format, 5 | 17);
            let premultiply = |channel: u8| {
                if straight {
                    ((channel as u16 * a as u16 + 127) / 255) as u8
                } else {
                    channel
                }
            };
            let to = (y * width + x) * 4;
            pixels[to..to + 4].copy_from_slice(&[
                premultiply(r),
                premultiply(g),
                premultiply(b),
                a,
            ]);
        }
    }
    Shot::new(width as u32, height as u32, pixels)
}

pub fn screen() -> Result<Shot, String> {
    let connection = Connection::new_session()
        .map_err(|error| format!("Session bus connect failed: {error}"))?;
    capture(&connection)
}

fn capture(connection: &Connection) -> Result<Shot, String> {
    let proxy = connection.with_proxy(
        "org.kde.KWin.ScreenShot2",
        "/org/kde/KWin/ScreenShot2",
        CAPTURE_TIMEOUT,
    );
    let (reader, writer) = pipe()?;
    let options: Options = [
        (
            "include-cursor".into(),
            Variant(Box::new(false) as Box<dyn RefArg>),
        ),
        (
            "native-resolution".into(),
            Variant(Box::new(true) as Box<dyn RefArg>),
        ),
        (
            "include-decoration".into(),
            Variant(Box::new(true) as Box<dyn RefArg>),
        ),
    ]
    .into_iter()
    .collect();
    let (metadata,): (Options,) = proxy
        .method_call(
            "org.kde.KWin.ScreenShot2",
            "CaptureWorkspace",
            (options, writer),
        )
        .map_err(|error| format!("KWin CaptureWorkspace failed: {error}"))?;
    let (_, height, stride, _) = dimensions(&metadata)?;
    let contents = read_pixels(reader, height * stride, CAPTURE_TIMEOUT)?;
    decode(&metadata, &contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::fd::IntoRawFd};

    fn metadata(width: u32, height: u32, stride: u32, format: u32) -> Options {
        let mut metadata: Options = [
            ("width", width),
            ("height", height),
            ("stride", stride),
            ("format", format),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), Variant(Box::new(value) as Box<dyn RefArg>)))
        .collect();
        metadata.insert("type".into(), Variant(Box::new("raw".to_owned())));
        metadata
    }

    #[test]
    fn decodes_native_argb_and_padded_rgb_rows() {
        let shot = decode(&metadata(1, 1, 4, 6), &0x80402010u32.to_ne_bytes()).unwrap();
        assert_eq!(shot.pixels, [64, 32, 16, 128]);
        let shot = decode(&metadata(1, 2, 4, 13), &[1, 2, 3, 0, 4, 5, 6, 0]).unwrap();
        assert_eq!(shot.pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
        let shot = decode(&metadata(1, 1, 4, 17), &[128, 64, 32, 128]).unwrap();
        assert_eq!(shot.pixels, [64, 32, 16, 128]);
    }

    #[test]
    fn rejects_invalid_metadata_and_truncated_pixels() {
        for metadata in [
            metadata(0, 1, 4, 6),
            metadata(2, 1, 4, 6),
            metadata(1, u32::MAX, u32::MAX, 6),
            metadata(1, 1, 4, 999),
        ] {
            assert!(decode(&metadata, &[]).is_err());
        }
        assert!(decode(&metadata(1, 1, 4, 6), &[0; 3]).is_err());
    }

    #[test]
    fn pipe_waits_for_delayed_chunks_and_detects_eof_and_timeout() {
        let (reader, writer) = pipe().unwrap();
        let writer = std::thread::spawn(move || {
            let mut writer = unsafe { std::fs::File::from_raw_fd(writer.into_raw_fd()) };
            std::thread::sleep(Duration::from_millis(10));
            writer.write_all(&[1, 2]).unwrap();
            std::thread::sleep(Duration::from_millis(10));
            writer.write_all(&[3, 4]).unwrap();
        });
        assert_eq!(
            read_pixels(reader, 4, Duration::from_secs(1)).unwrap(),
            [1, 2, 3, 4]
        );
        writer.join().unwrap();
        let (reader, writer) = pipe().unwrap();
        drop(writer);
        assert!(read_pixels(reader, 4, Duration::from_secs(1))
            .unwrap_err()
            .contains("closed"));
        let (reader, _writer) = pipe().unwrap();
        assert!(read_pixels(reader, 4, Duration::from_millis(10))
            .unwrap_err()
            .contains("Timed out"));
    }

    #[test]
    fn capture_workspace_reads_raw_pixels_written_after_the_dbus_reply() {
        use dbus::{
            channel::{MatchingReceiver, Sender},
            message::MatchRule,
        };
        use std::{
            io::{BufRead, BufReader},
            process::{Child, Command, Stdio},
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
        };

        struct Bus(Child);
        impl Drop for Bus {
            fn drop(&mut self) {
                self.0.kill().unwrap();
                self.0.wait().unwrap();
            }
        }

        let mut bus = Bus(Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1", "--nopidfile"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap());
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();
        let server_address = address.clone();
        let (ready, receiver) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let connection = Connection::new_address(&server_address).unwrap();
            connection
                .request_name("org.kde.KWin.ScreenShot2", false, false, true)
                .unwrap();
            let served = Arc::new(AtomicBool::new(false));
            let done = served.clone();
            connection.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |message, connection| {
                    assert_eq!(message.path().as_deref(), Some("/org/kde/KWin/ScreenShot2"));
                    assert_eq!(
                        message.interface().as_deref(),
                        Some("org.kde.KWin.ScreenShot2")
                    );
                    assert_eq!(message.member().as_deref(), Some("CaptureWorkspace"));
                    let (options, writer): (Options, OwnedFd) = message.read2().unwrap();
                    assert_eq!(options["native-resolution"].0.as_i64(), Some(1));
                    connection
                        .send(message.method_return().append1(metadata(1, 2, 8, 17)))
                        .unwrap();
                    let mut writer = unsafe { std::fs::File::from_raw_fd(writer.into_raw_fd()) };
                    std::thread::sleep(Duration::from_millis(10));
                    writer.write_all(&[128, 64, 32, 128, 0, 0, 0, 0]).unwrap();
                    std::thread::sleep(Duration::from_millis(10));
                    writer.write_all(&[10, 20, 30, 255, 0, 0, 0, 0]).unwrap();
                    done.store(true, Ordering::Relaxed);
                    false
                }),
            );
            ready.send(()).unwrap();
            while !served.load(Ordering::Relaxed) {
                connection.process(Duration::from_secs(2)).unwrap();
            }
        });
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let connection = Connection::new_address(&address).unwrap();
        let shot = capture(&connection).unwrap();
        assert_eq!(shot.size(), (1, 2));
        assert_eq!(shot.pixels, [64, 32, 16, 128, 10, 20, 30, 255]);
        server.join().unwrap();
    }
}
