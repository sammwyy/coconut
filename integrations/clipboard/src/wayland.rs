use crate::history::History;
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use wayland_client::{
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(3);
const TEXT_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain;charset=UTF-8",
    "text/plain",
    "UTF8_STRING",
];

fn text_offer(mimes: &[String]) -> Option<(&str, bool)> {
    if mimes.iter().any(|mime| mime == "x-kde-passwordManagerHint") {
        return None;
    }
    let mime = TEXT_MIMES
        .iter()
        .find(|mime| mimes.iter().any(|value| value == **mime))?;
    let retain = mimes.len() < 256 && mimes.iter().all(|mime| TEXT_MIMES.contains(&mime.as_str()));
    Some((mime, retain))
}

struct Command {
    text: Arc<str>,
    deadline: Instant,
    reply: oneshot::Sender<Result<(), String>>,
}

#[derive(Clone)]
pub(crate) struct Backend {
    commands: mpsc::SyncSender<Command>,
    wake: Arc<Mutex<UnixStream>>,
    stopped: tokio::sync::watch::Receiver<Option<String>>,
}

impl Backend {
    pub fn start(history: Arc<Mutex<History>>) -> io::Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        writer.set_nonblocking(true)?;
        let (commands, receiver) = mpsc::sync_channel(32);
        let (stopped, stop_receiver) = tokio::sync::watch::channel(None);
        std::thread::Builder::new()
            .name("clipboard-wayland".into())
            .spawn(move || {
                if let Err(error) = run(history.clone(), &reader, &receiver) {
                    eprintln!("clipboard: Wayland backend unavailable: {error}");
                    let mut history = history.lock().unwrap();
                    history.available = false;
                    history.error = error.to_string();
                    history.selection_changed();
                    let _ = stopped.send(Some(error.to_string()));
                }
                while let Ok(command) = receiver.try_recv() {
                    let _ = command
                        .reply
                        .send(Err("Clipboard backend unavailable".into()));
                }
            })?;
        Ok(Self {
            commands,
            wake: Arc::new(Mutex::new(writer)),
            stopped: stop_receiver,
        })
    }

    pub async fn wait_stopped(&self) -> String {
        let mut stopped = self.stopped.clone();
        if stopped.borrow().is_none() {
            let _ = stopped.changed().await;
        }
        let error = stopped
            .borrow()
            .clone()
            .unwrap_or_else(|| "Clipboard backend stopped".into());
        error
    }

    pub async fn set_text(&self, text: Arc<str>) -> Result<(), String> {
        let (reply, receiver) = oneshot::channel();
        self.commands
            .try_send(Command {
                text,
                deadline: Instant::now() + TRANSFER_TIMEOUT,
                reply,
            })
            .map_err(|_| "Clipboard command queue unavailable or full".to_string())?;
        match self.wake.lock().unwrap().write(&[1]) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.to_string()),
        }
        tokio::time::timeout(Duration::from_secs(5), receiver)
            .await
            .map_err(|_| "Clipboard command timed out".to_string())?
            .map_err(|_| "Clipboard backend disconnected".to_string())?
    }
}

struct Incoming {
    stream: UnixStream,
    bytes: Vec<u8>,
    generation: u64,
    deadline: Instant,
    retain_selection: bool,
}

struct Outgoing {
    file: std::fs::File,
    text: Arc<str>,
    offset: usize,
    deadline: Instant,
}

struct State {
    history: Arc<Mutex<History>>,
    manager: Option<ZwlrDataControlManagerV1>,
    seats: Vec<(u32, wl_seat::WlSeat)>,
    device: Option<ZwlrDataControlDeviceV1>,
    selection: Option<ZwlrDataControlOfferV1>,
    incoming: Option<Incoming>,
    outgoing: Vec<Outgoing>,
    marker: String,
    finished: bool,
    selected_seat: u32,
}

impl State {
    fn own(&mut self, text: Arc<str>, qh: &QueueHandle<Self>) {
        let source = self.manager.as_ref().unwrap().create_data_source(qh, text);
        for mime in TEXT_MIMES {
            source.offer((*mime).into());
        }
        source.offer(self.marker.clone());
        self.device.as_ref().unwrap().set_selection(Some(&source));
    }

    fn read_selection(&mut self, qh: &QueueHandle<Self>) -> io::Result<()> {
        let Some(mut incoming) = self.incoming.take() else {
            return Ok(());
        };
        let maximum = self.history.lock().unwrap().config.max_item_bytes;
        if Instant::now() >= incoming.deadline {
            eprintln!("clipboard: clipboard read timed out");
            return Ok(());
        }
        let mut buffer = [0; 8192];
        loop {
            match incoming.stream.read(&mut buffer) {
                Ok(0) => {
                    match String::from_utf8(incoming.bytes) {
                        Ok(text) if !text.is_empty() => {
                            let text: Arc<str> = text.into();
                            let current = {
                                let mut history = self.history.lock().unwrap();
                                if incoming.generation != history.generation
                                    || text.len() > history.config.max_item_bytes
                                {
                                    false
                                } else {
                                    history.record(text.clone());
                                    true
                                }
                            };
                            if current && incoming.retain_selection {
                                self.own(text, qh);
                            }
                        }
                        Ok(_) => {}
                        Err(_) => eprintln!("clipboard: selection is not UTF-8 text"),
                    }
                    return Ok(());
                }
                Ok(length) => {
                    if incoming.bytes.len() + length > maximum {
                        eprintln!("clipboard: selection exceeds configured item limit");
                        return Ok(());
                    }
                    incoming.bytes.extend_from_slice(&buffer[..length]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    eprintln!("clipboard: cannot read selection: {error}");
                    return Ok(());
                }
            }
        }
        self.incoming = Some(incoming);
        Ok(())
    }

    fn write_selections(&mut self) {
        self.outgoing.retain_mut(|outgoing| {
            if Instant::now() >= outgoing.deadline {
                eprintln!("clipboard: clipboard write timed out");
                return false;
            }
            match outgoing
                .file
                .write(&outgoing.text.as_bytes()[outgoing.offset..])
            {
                Ok(0) => false,
                Ok(length) => {
                    outgoing.offset += length;
                    outgoing.offset < outgoing.text.len()
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    true
                }
                Err(error) => {
                    if error.kind() != io::ErrorKind::BrokenPipe {
                        eprintln!("clipboard: cannot serve selection: {error}");
                    }
                    false
                }
            }
        });
    }
}

fn run(
    history: Arc<Mutex<History>>,
    wake: &UnixStream,
    commands: &mpsc::Receiver<Command>,
) -> io::Result<()> {
    let connection = Connection::connect_to_env().map_err(io::Error::other)?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut state = State {
        history,
        manager: None,
        seats: Vec::new(),
        device: None,
        selection: None,
        incoming: None,
        outgoing: Vec::new(),
        marker: format!("application/x-coconut-clipboard-{}", std::process::id()),
        finished: false,
        selected_seat: 0,
    };
    connection.display().get_registry(&qh, ());
    queue.roundtrip(&mut state).map_err(io::Error::other)?;
    queue.roundtrip(&mut state).map_err(io::Error::other)?;
    let seat_name = state.history.lock().unwrap().config.seat.clone();
    let (id, seat) = state
        .seats
        .iter()
        .find(|(_, seat)| {
            seat_name.as_ref().is_none_or(|name| {
                seat.data::<Mutex<String>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .as_str()
                    == name
            })
        })
        .ok_or_else(|| io::Error::other("Configured Wayland seat is unavailable"))?;
    state.selected_seat = *id;
    let manager = state
        .manager
        .as_ref()
        .ok_or_else(|| io::Error::other("Compositor does not expose wlr-data-control"))?;
    state.device = Some(manager.get_data_device(seat, &qh, ()));
    {
        let mut history = state.history.lock().unwrap();
        history.available = true;
        history.error.clear();
        history.notify();
    }
    loop {
        queue
            .dispatch_pending(&mut state)
            .map_err(io::Error::other)?;
        if state.finished {
            return Err(io::Error::other("Wayland clipboard seat disconnected"));
        }
        state.read_selection(&qh)?;
        state.write_selections();
        while let Ok(command) = commands.try_recv() {
            if command.reply.is_closed() || Instant::now() >= command.deadline {
                continue;
            }
            if command.text.len() > state.history.lock().unwrap().config.max_item_bytes {
                let _ = command
                    .reply
                    .send(Err("Text exceeds the current clipboard limit".into()));
                continue;
            }
            state.incoming = None;
            state.history.lock().unwrap().selection_changed();
            state.own(command.text.clone(), &qh);
            let result = connection.flush().map_err(|error| error.to_string());
            if result.is_ok() {
                state.history.lock().unwrap().record(command.text);
            }
            let _ = command.reply.send(result);
        }
        connection.flush().map_err(io::Error::other)?;
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let mut fds = vec![
            pollfd(connection.as_fd().as_raw_fd(), libc::POLLIN),
            pollfd(wake.as_raw_fd(), libc::POLLIN),
        ];
        if let Some(incoming) = &state.incoming {
            fds.push(pollfd(incoming.stream.as_raw_fd(), libc::POLLIN));
        }
        for outgoing in &state.outgoing {
            fds.push(pollfd(outgoing.file.as_raw_fd(), libc::POLLOUT));
        }
        let timeout = if state.incoming.is_some() || !state.outgoing.is_empty() {
            100
        } else {
            1000
        };
        let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
        if fds[0].revents != 0 {
            guard.read().map_err(io::Error::other)?;
        } else {
            drop(guard);
        }
        if fds[1].revents != 0 {
            let mut wake = wake;
            let mut buffer = [0; 128];
            while wake.read(&mut buffer).is_ok_and(|length| length > 0) {}
        }
        state.history.lock().unwrap().prune();
    }
}

fn pollfd(fd: i32, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "zwlr_data_control_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(1), qh, ()));
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "wl_seat" => {
                state.seats.push((
                    name,
                    registry.bind(name, version.min(2), qh, Mutex::new(String::new())),
                ));
            }
            wl_registry::Event::GlobalRemove { name } if name == state.selected_seat => {
                state.finished = true
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, Mutex<String>> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        event: wl_seat::Event,
        name: &Mutex<String>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Name { name: value } = event {
            *name.lock().unwrap() = value;
        }
    }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlManagerV1,
        _: wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlOfferV1, Mutex<Vec<String>>> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        mimes: &Mutex<Vec<String>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            let mut mimes = mimes.lock().unwrap();
            if mimes.len() < 256 && mime_type.len() < 1024 {
                mimes.push(mime_type);
            }
        }
    }
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrDataControlDeviceV1,
        event: zwlr_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_device_v1::Event::Selection { id } => {
                if let Some(previous) = state.selection.take() {
                    previous.destroy();
                }
                state.selection = id.clone();
                state.incoming = None;
                if let Some(offer) = id {
                    let mimes = offer.data::<Mutex<Vec<String>>>().unwrap().lock().unwrap();
                    if mimes.contains(&state.marker) {
                        return;
                    }
                    let generation = state.history.lock().unwrap().selection_changed();
                    if let Some((mime, retain_selection)) = text_offer(&mimes) {
                        match UnixStream::pair() {
                            Ok((stream, writer)) => {
                                if let Err(error) = stream.set_nonblocking(true) {
                                    eprintln!("clipboard: cannot prepare transfer: {error}");
                                    return;
                                }
                                offer.receive(mime.into(), writer.as_fd());
                                state.incoming = Some(Incoming {
                                    stream,
                                    bytes: Vec::new(),
                                    generation,
                                    deadline: Instant::now() + TRANSFER_TIMEOUT,
                                    retain_selection,
                                });
                            }
                            Err(error) => eprintln!("clipboard: cannot receive selection: {error}"),
                        }
                    }
                } else {
                    state.history.lock().unwrap().selection_changed();
                }
            }
            zwlr_data_control_device_v1::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    wayland_client::event_created_child!(State, ZwlrDataControlDeviceV1, [0 => (ZwlrDataControlOfferV1, Mutex::new(Vec::<String>::new()))]);
}

fn nonblocking(fd: &OwnedFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl Dispatch<ZwlrDataControlSourceV1, Arc<str>> for State {
    fn event(
        state: &mut Self,
        source: &ZwlrDataControlSourceV1,
        event: zwlr_data_control_source_v1::Event,
        text: &Arc<str>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_source_v1::Event::Send { fd, mime_type }
                if TEXT_MIMES.contains(&mime_type.as_str()) =>
            {
                if state.outgoing.len() >= 16 {
                    eprintln!("clipboard: too many concurrent paste transfers");
                    return;
                }
                if let Err(error) = nonblocking(&fd) {
                    eprintln!("clipboard: cannot prepare paste: {error}");
                    return;
                }
                state.outgoing.push(Outgoing {
                    file: fd.into(),
                    text: text.clone(),
                    offset: 0,
                    deadline: Instant::now() + TRANSFER_TIMEOUT,
                });
            }
            zwlr_data_control_source_v1::Event::Cancelled => source.destroy(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_capture_preserves_rich_formats_and_skips_sensitive_selections() {
        assert_eq!(
            text_offer(&["text/plain".into()]),
            Some(("text/plain", true))
        );
        assert_eq!(
            text_offer(&["text/plain".into(), "text/html".into()]),
            Some(("text/plain", false))
        );
        assert_eq!(
            text_offer(&["text/plain".into(), "x-kde-passwordManagerHint".into()]),
            None
        );
        assert_eq!(text_offer(&["image/png".into()]), None);
    }
}
