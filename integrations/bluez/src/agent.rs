use super::{State, DEVICE_INTERFACE, SERVICE};
use coconut_api::bluetooth::{PairingKind, PairingPrompt, PairingResponse};
use dbus::blocking::Connection;
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::MatchRule;
use dbus::{Message, MessageType, Path};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AGENT_PATH: &str = "/org/coconut/PairingAgent";
const AGENT_INTERFACE: &str = "org.bluez.Agent1";
const MANAGER_INTERFACE: &str = "org.bluez.AgentManager1";

pub(super) fn pair(
    connection: &Connection,
    device: &Path<'static>,
    address: &str,
    state: Arc<Mutex<State>>,
) -> Result<(), String> {
    pair_with_timeout(connection, device, address, state, Duration::from_secs(90))
}

fn pair_with_timeout(
    connection: &Connection,
    device: &Path<'static>,
    address: &str,
    state: Arc<Mutex<State>>,
    timeout: Duration,
) -> Result<(), String> {
    let (owner,): (String,) = connection
        .with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            Duration::from_secs(2),
        )
        .method_call("org.freedesktop.DBus", "GetNameOwner", (SERVICE,))
        .map_err(|error| error.to_string())?;
    let manager = connection.with_proxy(SERVICE, "/org/bluez", Duration::from_secs(3));
    let _: () = manager
        .method_call(
            MANAGER_INTERFACE,
            "RegisterAgent",
            (Path::from(AGENT_PATH), "KeyboardDisplay"),
        )
        .map_err(|error| error.to_string())?;
    let pending = Arc::new(Mutex::new(None::<(Message, PairingKind)>));
    let requests = pending.clone();
    let cache = state.clone();
    let device_path = device.clone();
    let address = address.to_owned();
    let mut rule = MatchRule::new_method_call()
        .with_path(AGENT_PATH)
        .with_interface(AGENT_INTERFACE)
        .with_sender(owner.clone());
    rule.strict_sender = true;
    let agent = connection.start_receive(
        rule,
        Box::new(move |message, connection| {
            let member = message
                .member()
                .map(|member| member.to_string())
                .unwrap_or_default();
            if matches!(member.as_str(), "Cancel" | "Release") {
                requests.lock().ok().and_then(|mut pending| pending.take());
                if let Ok(mut state) = cache.lock() {
                    state.prompt = None;
                    state.answer = None;
                    state.operation_revision += 1;
                }
                let _ = connection.send(message.method_return());
                return true;
            }
            let request = decode_request(&member, &message);
            let Ok((path, kind, code)) = request else {
                let _ = connection.send(rejected(&message));
                return true;
            };
            if path != device_path {
                let _ = connection.send(rejected(&message));
                return true;
            }
            if let Ok(mut state) = cache.lock() {
                state.operation_revision += 1;
                state.prompt = Some(PairingPrompt {
                    id: state.operation_revision,
                    address: address.clone(),
                    kind,
                    code,
                });
                state.answer = None;
            }
            if kind == PairingKind::Display {
                let _ = connection.send(message.method_return());
            } else if let Ok(mut pending) = requests.lock() {
                *pending = Some((message, kind));
            }
            true
        }),
    );
    let result = Arc::new(Mutex::new(None));
    let pair_message = Message::new_method_call(SERVICE, device.clone(), DEVICE_INTERFACE, "Pair")?;
    let serial = connection
        .send(pair_message)
        .map_err(|_| "Could not start Bluetooth pairing")?;
    let replies: Vec<_> = [MessageType::MethodReturn, MessageType::Error]
        .into_iter()
        .map(|msg_type| {
            let result = result.clone();
            let mut rule = MatchRule::new().with_sender(owner.clone());
            rule.msg_type = Some(msg_type);
            connection.start_receive(
                rule,
                Box::new(move |mut message, _| {
                    if message.get_reply_serial() == Some(serial) {
                        if let Ok(mut result) = result.lock() {
                            *result = Some(
                                message
                                    .as_result()
                                    .map(|_| ())
                                    .map_err(|error| error.to_string()),
                            );
                        }
                    }
                    true
                }),
            )
        })
        .collect();
    let outcome = (|| {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(result) = result.lock().ok().and_then(|mut result| result.take()) {
                return result;
            }
            if Instant::now() >= deadline {
                cancel_pairing(connection, device);
                return Err("Bluetooth pairing timed out.".into());
            }
            connection
                .process(Duration::from_millis(100))
                .map_err(|error| error.to_string())?;
            let answer = state.lock().ok().and_then(|mut state| state.answer.take());
            if let Some(answer) = answer {
                let request = pending.lock().ok().and_then(|mut pending| pending.take());
                if let Some((message, kind)) = request {
                    match reply(&message, kind, &answer) {
                        Ok(response) => {
                            connection
                                .send(response)
                                .map_err(|_| "Could not answer the Bluetooth pairing request")?;
                            if let Ok(mut state) = state.lock() {
                                state.prompt = None;
                                state.operation_revision += 1;
                            }
                        }
                        Err(error) => {
                            if let Ok(mut pending) = pending.lock() {
                                *pending = Some((message, kind));
                            }
                            if let Ok(mut state) = state.lock() {
                                state.error = Some(error);
                                state.operation_revision += 1;
                            }
                        }
                    }
                } else if matches!(answer, PairingResponse::Reject) {
                    cancel_pairing(connection, device);
                }
            }
        }
    })();
    connection.stop_receive(agent);
    for token in replies {
        connection.stop_receive(token);
    }
    let _: Result<(), _> = manager.method_call(
        MANAGER_INTERFACE,
        "UnregisterAgent",
        (Path::from(AGENT_PATH),),
    );
    if let Ok(mut state) = state.lock() {
        state.prompt = None;
        state.answer = None;
        state.operation_revision += 1;
    }
    outcome
}

fn cancel_pairing(connection: &Connection, device: &Path<'static>) {
    if let Ok(message) =
        Message::new_method_call(SERVICE, device.clone(), DEVICE_INTERFACE, "CancelPairing")
    {
        let _ = connection.send(message);
    }
}

fn decode_request(
    member: &str,
    message: &Message,
) -> Result<(Path<'static>, PairingKind, Option<String>), String> {
    let path: Path<'static> = message.read1().map_err(|error| error.to_string())?;
    let (kind, code) = match member {
        "RequestConfirmation" => {
            let (_, code): (Path<'static>, u32) =
                message.read2().map_err(|error| error.to_string())?;
            (PairingKind::Confirmation, Some(format!("{code:06}")))
        }
        "RequestPinCode" => (PairingKind::Pin, None),
        "RequestPasskey" => (PairingKind::Passkey, None),
        "RequestAuthorization" | "AuthorizeService" => (PairingKind::Authorization, None),
        "DisplayPinCode" => {
            let (_, code): (Path<'static>, String) =
                message.read2().map_err(|error| error.to_string())?;
            (PairingKind::Display, Some(code))
        }
        "DisplayPasskey" => {
            let (_, code, _): (Path<'static>, u32, u16) =
                message.read3().map_err(|error| error.to_string())?;
            (PairingKind::Display, Some(format!("{code:06}")))
        }
        _ => return Err("Unsupported pairing request".into()),
    };
    Ok((path, kind, code))
}

fn rejected(message: &Message) -> Message {
    message.error(
        &dbus::strings::ErrorName::from("org.bluez.Error.Rejected"),
        c"Pairing was rejected",
    )
}

fn reply(
    message: &Message,
    kind: PairingKind,
    response: &PairingResponse,
) -> Result<Message, String> {
    if matches!(response, PairingResponse::Reject) {
        return Ok(rejected(message));
    }
    match (kind, response) {
        (PairingKind::Pin, PairingResponse::Code(code)) if !code.is_empty() && code.len() <= 16 => {
            Ok(message.method_return().append1(code.clone()))
        }
        (PairingKind::Passkey, PairingResponse::Code(code))
            if !code.is_empty()
                && code.len() <= 6
                && code.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            Ok(message
                .method_return()
                .append1(code.parse::<u32>().map_err(|_| "Enter a numeric passkey")?))
        }
        (PairingKind::Confirmation | PairingKind::Authorization, PairingResponse::Accept) => {
            Ok(message.method_return())
        }
        _ => Err("Enter a valid PIN (1–16 characters) or numeric passkey (up to 6 digits).".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Message {
        let mut message =
            Message::new_method_call(SERVICE, AGENT_PATH, AGENT_INTERFACE, "RequestPasskey")
                .unwrap();
        message.set_serial(1);
        message
    }
    #[test]
    fn pairing_responses_validate_codes_and_rejections() {
        let message = request();
        let response = reply(
            &message,
            PairingKind::Passkey,
            &PairingResponse::Code("000123".into()),
        )
        .unwrap();
        assert_eq!(response.read1::<u32>().unwrap(), 123);
        assert!(reply(
            &message,
            PairingKind::Passkey,
            &PairingResponse::Code("abc".into())
        )
        .is_err());
        assert!(reply(
            &message,
            PairingKind::Pin,
            &PairingResponse::Code(String::new())
        )
        .is_err());
        assert_eq!(
            reply(
                &message,
                PairingKind::Confirmation,
                &PairingResponse::Reject
            )
            .unwrap()
            .msg_type(),
            MessageType::Error
        );
    }
    #[test]
    fn confirmation_codes_keep_leading_zeroes() {
        let message = request().append2(Path::from("/org/bluez/device"), 42u32);
        let (_, kind, code) = decode_request("RequestConfirmation", &message).unwrap();
        assert_eq!(kind, PairingKind::Confirmation);
        assert_eq!(code.as_deref(), Some("000042"));
    }

    #[test]
    #[ignore = "requires dbus-daemon; starts a private fixture bus"]
    fn private_bus_pairing_handles_confirmation_rejection_and_passkeys() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::thread;
        struct Bus(std::process::Child);
        impl Drop for Bus {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        for (method, response, succeeds) in [
            ("RequestConfirmation", PairingResponse::Accept, true),
            ("RequestConfirmation", PairingResponse::Reject, false),
            (
                "RequestPasskey",
                PairingResponse::Code("000123".into()),
                true,
            ),
        ] {
            let mut bus = Bus(Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap());
            let mut address = String::new();
            BufReader::new(bus.0.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            let address = address.trim().to_owned();
            let server_address = address.clone();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let server = thread::spawn(move || {
                let connection = Connection::new_address(&server_address).unwrap();
                connection
                    .request_name(SERVICE, false, true, false)
                    .unwrap();
                let pending = Arc::new(Mutex::new(None::<Message>));
                let request_serial = Arc::new(std::sync::atomic::AtomicU32::new(0));
                let stopped = Arc::new(AtomicBool::new(false));
                let callback_pending = pending.clone();
                let callback_serial = request_serial.clone();
                let done = stopped.clone();
                connection.start_receive(
                    MatchRule::new_method_call(),
                    Box::new(move |message, connection| {
                        let member = message.member().unwrap().to_string();
                        if member == "Pair" {
                            let destination = message.sender().unwrap().to_string();
                            let request = Message::new_method_call(
                                destination,
                                AGENT_PATH,
                                AGENT_INTERFACE,
                                method,
                            )
                            .unwrap();
                            let request = if method == "RequestConfirmation" {
                                request.append2(Path::from("/org/bluez/device"), 42u32)
                            } else {
                                request.append1(Path::from("/org/bluez/device"))
                            };
                            callback_serial
                                .store(connection.send(request).unwrap(), Ordering::Release);
                            *callback_pending.lock().unwrap() = Some(message);
                        } else {
                            assert!(matches!(
                                member.as_str(),
                                "RegisterAgent" | "UnregisterAgent"
                            ));
                            connection.send(message.method_return()).unwrap();
                            if member == "UnregisterAgent" {
                                done.store(true, Ordering::Release);
                            }
                        }
                        true
                    }),
                );
                for msg_type in [MessageType::MethodReturn, MessageType::Error] {
                    let pending = pending.clone();
                    let serial = request_serial.clone();
                    let mut rule = MatchRule::new();
                    rule.msg_type = Some(msg_type);
                    connection.start_receive(
                        rule,
                        Box::new(move |message, connection| {
                            if message.get_reply_serial() == Some(serial.load(Ordering::Acquire)) {
                                if method == "RequestPasskey" {
                                    assert_eq!(message.read1::<u32>().unwrap(), 123);
                                }
                                let pair = pending.lock().unwrap().take().unwrap();
                                connection
                                    .send(if message.msg_type() == MessageType::Error {
                                        rejected(&pair)
                                    } else {
                                        pair.method_return()
                                    })
                                    .unwrap();
                            }
                            true
                        }),
                    );
                }
                ready_tx.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(6);
                while !stopped.load(Ordering::Acquire) && Instant::now() < deadline {
                    connection.process(Duration::from_millis(30)).unwrap();
                }
                assert!(
                    stopped.load(Ordering::Acquire),
                    "the agent must unregister when pairing finishes"
                );
            });
            ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let state = Arc::new(Mutex::new(State::default()));
            let cache = state.clone();
            let answer = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    if let Ok(mut state) = cache.lock() {
                        if let Some(prompt) = &state.prompt {
                            if method == "RequestConfirmation" {
                                assert_eq!(prompt.code.as_deref(), Some("000042"));
                            }
                            state.answer = Some(response);
                            break;
                        }
                    }
                    assert!(
                        Instant::now() < deadline,
                        "the pairing request must reach the UI state"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            });
            let client = Connection::new_address(&address).unwrap();
            let result = pair_with_timeout(
                &client,
                &Path::from("/org/bluez/device"),
                "AA:00:00:00:00:01",
                state.clone(),
                Duration::from_secs(4),
            );
            answer.join().unwrap();
            server.join().unwrap();
            assert_eq!(result.is_ok(), succeeds, "{result:?}");
            assert!(state.lock().unwrap().prompt.is_none());
        }
    }
}
