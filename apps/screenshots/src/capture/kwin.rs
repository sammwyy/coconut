//! KDE Plasma capture: `org.kde.KWin.ScreenShot2` writes a PNG into a file
//! descriptor the caller supplies. Used when a Wayland compositor does not
//! implement `wlr-screencopy` (KDE Plasma exports this interface instead).

use crate::image::Shot;
use dbus::arg::{OwnedFd, RefArg, Variant};
use dbus::blocking::Connection;
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, FromRawFd, OwnedFd as StdOwnedFd},
    time::Duration,
};

type Options = HashMap<String, Variant<Box<dyn RefArg>>>;

fn memfd() -> Result<StdOwnedFd, String> {
    let name = c"coconut-screenshots";
    let fd = unsafe { libc::memfd_create(name.as_ptr().cast(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(format!(
            "memfd_create failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(unsafe { StdOwnedFd::from_raw_fd(fd) })
}

pub fn screen() -> Result<Shot, String> {
    let connection = Connection::new_session()
        .map_err(|error| format!("session bus connect failed: {error}"))?;
    let proxy = connection.with_proxy(
        "org.kde.KWin.ScreenShot2",
        "/org/kde/KWin/ScreenShot2",
        Duration::from_secs(15),
    );
    let file = memfd()?;
    // The call consumes the fd it receives: hand over a duplicate so `file`
    // survives to read the image back.
    let duplicate = unsafe {
        let fd = libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0);
        if fd < 0 {
            return Err(format!(
                "fcntl(F_DUPFD_CLOEXEC) failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        OwnedFd::new(fd)
    };
    let mut options = Options::new();
    options.insert("include-cursor".into(), Variant(Box::new(false)));
    options.insert("native-resolution".into(), Variant(Box::new(true)));
    options.insert("include-decoration".into(), Variant(Box::new(true)));
    let (_reply,): (Options,) = proxy
        .method_call(
            "org.kde.KWin.ScreenShot2",
            "CaptureScreen",
            ("coconut-screenshots".to_owned(), options, duplicate),
        )
        .map_err(|error| format!("KWin CaptureScreen failed: {error}"))?;
    let mut contents = Vec::new();
    let mut file = std::fs::File::from(file);
    std::io::Read::read_to_end(&mut file, &mut contents)
        .map_err(|error| format!("could not read the KWin screenshot: {error}"))?;
    let decoded = image::load_from_memory(&contents)
        .map_err(|error| format!("KWin returned an undecodable capture: {error}"))?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    Shot::new(width, height, decoded.into_raw())
}
