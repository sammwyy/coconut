use super::*;
use crate::*;
use coconut_api::battery::{BatteryDetails, BatteryIntegration};
use coconut_api::bluetooth::{
    BluetoothDevice, BluetoothIntegration, PairingKind, PairingPrompt, PairingResponse,
};
use coconut_api::brightness::BrightnessIntegration;
use coconut_api::network::{NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork};
use coconut_api::power_profile::{PowerProfile, PowerProfileIntegration};
use coconut_api::volume::{AudioDevice, AudioDirection, AudioStream, VolumeIntegration};
use coconut_plugin_kit::{design, SharedState};
use creamui_core::{
    BoxedWidget, Key, KeyInput, Painter, Point, Rect, Renderer, Scene, Size, TextAlign,
};
use creamui_reactive::Signal;
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
struct Labels(Vec<(String, Rect)>);
impl Painter for Labels {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, rect: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
        self.0.push((text.to_owned(), rect));
    }
}

struct View {
    scene: Scene,
    labels: Vec<(String, Rect)>,
    size: Size,
}
impl View {
    fn point(&self, label: &str) -> Point {
        self.labels
            .iter()
            .filter(|(text, _)| text == label)
            .find_map(|(_, rect)| {
                let point = Point {
                    x: rect.x + rect.width / 2.0,
                    y: rect.y + rect.height / 2.0,
                };
                (point.x < self.size.width
                    && point.y < self.size.height
                    && self.scene.hit_test(point).is_some())
                .then_some(point)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{label:?} needs a visible hit target; labels: {:?}",
                    self.labels
                )
            })
    }
    fn click(&self, label: &str) {
        let point = self.point(label);
        if let Some(click) = self.scene.hit_test_at(point) {
            click(point);
        } else {
            self.scene.hit_test(point).unwrap()();
        }
    }
    fn drag_first(&self, x: f32, mut rows: std::ops::Range<i32>, fraction: f32) {
        let (point, index) = rows
            .find_map(|y| {
                let point = Point { x, y: y as f32 };
                self.scene.drag_hit_test(point).map(|index| (point, index))
            })
            .expect("a visible mixer slider");
        let (rect, start) = self.scene.drag_start_at(point).unwrap();
        start(
            Point {
                x: point.x - rect.x,
                y: point.y - rect.y,
            },
            rect,
        );
        self.scene.draggable_at(index).unwrap().1(
            Point {
                x: rect.width * fraction,
                y: 10.0,
            },
            rect,
        );
        self.scene.drag_end_at(index).unwrap()();
    }
}

fn render(mut build: impl FnMut() -> BoxedWidget, size: Size, theme: Theme, name: &str) -> View {
    let mut labels = Labels::default();
    let scene = Renderer::new().render(build(), size, &mut labels);
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        Color::rgba(0, 0, 0, 0),
        theme.colors,
    );
    Renderer::new().render(build(), size, &mut recorder);
    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
    raster.render(&recorder.finish(), &Damage::Full);
    assert!(raster.pixmap().pixel(0, 0).unwrap().alpha() > 0);
    if let Ok(prefix) = std::env::var("COCONUT_TRAY_SNAPSHOT") {
        raster
            .pixmap()
            .save_png(format!("{prefix}-{name}.png"))
            .unwrap();
    }
    View {
        scene,
        labels: labels.0,
        size,
    }
}

struct Sound {
    level: Cell<f32>,
    muted: Cell<bool>,
    devices: RefCell<Vec<AudioDevice>>,
    streams: RefCell<Vec<AudioStream>>,
    actions: RefCell<Vec<String>>,
    error: RefCell<Option<String>>,
}
impl Sound {
    fn new() -> Self {
        Self {
            level: Cell::new(0.42),
            muted: Cell::new(false),
            error: RefCell::new(None),
            actions: RefCell::new(vec![]),
            devices: RefCell::new(vec![
                AudioDevice {
                    id: "speakers".into(),
                    name: "Built-in speakers".into(),
                    description: "Analog stereo".into(),
                    direction: AudioDirection::Output,
                    level: 0.42,
                    muted: false,
                    active: true,
                },
                AudioDevice {
                    id: "headphones".into(),
                    name: "Studio headphones".into(),
                    description: "USB audio · Stereo".into(),
                    direction: AudioDirection::Output,
                    level: 0.6,
                    muted: false,
                    active: false,
                },
                AudioDevice {
                    id: "mic".into(),
                    name: "USB microphone".into(),
                    description: "Mono input".into(),
                    direction: AudioDirection::Input,
                    level: 0.7,
                    muted: false,
                    active: true,
                },
                AudioDevice {
                    id: "built-in-mic".into(),
                    name: "Built-in microphone".into(),
                    description: "Internal input".into(),
                    direction: AudioDirection::Input,
                    level: 0.5,
                    muted: false,
                    active: false,
                },
            ]),
            streams: RefCell::new(vec![
                AudioStream {
                    id: "music".into(),
                    name: "Music".into(),
                    description: "Current track".into(),
                    direction: AudioDirection::Output,
                    level: 0.8,
                    muted: false,
                },
                AudioStream {
                    id: "browser".into(),
                    name: "Browser".into(),
                    description: "Video playback".into(),
                    direction: AudioDirection::Output,
                    level: 0.5,
                    muted: false,
                },
            ]),
        }
    }
}
impl VolumeIntegration for Sound {
    fn level(&self) -> f32 {
        self.level.get()
    }
    fn muted(&self) -> bool {
        self.muted.get()
    }
    fn set_level(&self, level: f32) {
        self.level.set(level);
        self.actions.borrow_mut().push("master-level".into());
    }
    fn supports_mute(&self) -> bool {
        true
    }
    fn set_muted(&self, value: bool) {
        self.muted.set(value);
        for device in self
            .devices
            .borrow_mut()
            .iter_mut()
            .filter(|device| device.active && device.direction == AudioDirection::Output)
        {
            device.muted = value;
        }
    }
    fn devices(&self) -> Vec<AudioDevice> {
        self.devices.borrow().clone()
    }
    fn streams(&self) -> Vec<AudioStream> {
        self.streams.borrow().clone()
    }
    fn set_default_device(&self, id: &str) {
        let direction = self
            .devices
            .borrow()
            .iter()
            .find(|device| device.id == id)
            .unwrap()
            .direction;
        for device in self
            .devices
            .borrow_mut()
            .iter_mut()
            .filter(|device| device.direction == direction)
        {
            device.active = device.id == id;
        }
        self.actions.borrow_mut().push(format!("default:{id}"));
    }
    fn set_device_level(&self, id: &str, level: f32) {
        self.devices
            .borrow_mut()
            .iter_mut()
            .find(|device| device.id == id)
            .unwrap()
            .level = level;
        self.actions.borrow_mut().push(format!("device:{id}"));
    }
    fn set_device_muted(&self, id: &str, value: bool) {
        self.devices
            .borrow_mut()
            .iter_mut()
            .find(|device| device.id == id)
            .unwrap()
            .muted = value;
    }
    fn set_stream_level(&self, id: &str, level: f32) {
        self.streams
            .borrow_mut()
            .iter_mut()
            .find(|stream| stream.id == id)
            .unwrap()
            .level = level;
        self.actions.borrow_mut().push(format!("stream:{id}"));
    }
    fn set_stream_muted(&self, id: &str, value: bool) {
        self.streams
            .borrow_mut()
            .iter_mut()
            .find(|stream| stream.id == id)
            .unwrap()
            .muted = value;
    }
    fn last_error(&self) -> Option<String> {
        self.error.borrow().clone()
    }
}

#[derive(Default)]
struct Network {
    joined: RefCell<Vec<(String, String)>>,
    disconnected: RefCell<Vec<String>>,
    revealed: Cell<bool>,
}
impl NetworkIntegration for Network {
    fn connected(&self) -> bool {
        true
    }
    fn network_name(&self) -> Option<String> {
        Some("Coconut office".into())
    }
    fn strength(&self) -> Option<u8> {
        Some(85)
    }
    fn enabled(&self) -> bool {
        true
    }
    fn set_enabled(&self, _: bool) {}
    fn has_wifi_adapter(&self) -> bool {
        true
    }
    fn supports_password_connection(&self) -> bool {
        true
    }
    fn connect_wifi_with_password(&self, ssid: &str, password: &str) {
        self.joined
            .borrow_mut()
            .push((ssid.into(), password.into()));
    }
    fn can_disconnect(&self) -> bool {
        true
    }
    fn disconnect(&self, interface: &str) {
        self.disconnected.borrow_mut().push(interface.into());
    }
    fn request_wifi_password(&self, _: &str) {
        self.revealed.set(true);
    }
    fn cached_wifi_password(&self, _: &str) -> Option<String> {
        self.revealed.get().then(|| "test password".into())
    }
    fn clear_wifi_password(&self) {
        self.revealed.set(false);
    }
    fn saved_wifi_networks(&self) -> Vec<String> {
        vec!["Coconut office".into(), "Home network".into()]
    }
    fn wifi_networks(&self) -> Vec<WifiNetwork> {
        vec![
            WifiNetwork {
                ssid: "Coconut office".into(),
                strength: 85,
                secured: true,
                active: true,
                band: Some("5 GHz"),
                security: "WPA2 Personal".into(),
            },
            WifiNetwork {
                ssid: "Guest network".into(),
                strength: 70,
                secured: true,
                active: false,
                band: Some("2.4 GHz"),
                security: "WPA3 Personal".into(),
            },
        ]
    }
    fn devices(&self) -> Vec<NetworkDevice> {
        vec![NetworkDevice {
            interface: "wlan0".into(),
            kind: NetworkDeviceKind::Wifi,
            connection_name: Some("Coconut office".into()),
            active: true,
            ip_address: Some("192.168.1.42".into()),
            subnet_mask: Some("255.255.255.0".into()),
            gateway: Some("192.168.1.1".into()),
            dns: vec!["192.168.1.1".into()],
            dhcp: true,
        }]
    }
}

struct Bluetooth {
    devices: RefCell<Vec<BluetoothDevice>>,
    prompt: RefCell<Option<PairingPrompt>>,
    responses: RefCell<Vec<PairingResponse>>,
}
impl Bluetooth {
    fn new() -> Self {
        Self {
            devices: RefCell::new(vec![
                BluetoothDevice {
                    address: "AA:00:00:00:00:01".into(),
                    name: "Studio headphones".into(),
                    paired: true,
                    connected: true,
                    trusted: true,
                    battery_percent: Some(84),
                    icon_hint: Some("audio-headphones".into()),
                    class: None,
                },
                BluetoothDevice {
                    address: "AA:00:00:00:00:02".into(),
                    name: "Travel keyboard".into(),
                    paired: false,
                    connected: false,
                    trusted: false,
                    battery_percent: None,
                    icon_hint: Some("input-keyboard".into()),
                    class: None,
                },
            ]),
            prompt: RefCell::new(None),
            responses: RefCell::new(vec![]),
        }
    }
}
impl BluetoothIntegration for Bluetooth {
    fn powered(&self) -> bool {
        true
    }
    fn connected(&self) -> bool {
        true
    }
    fn device_name(&self) -> Option<String> {
        Some("Studio headphones".into())
    }
    fn set_powered(&self, _: bool) {}
    fn devices(&self) -> Vec<BluetoothDevice> {
        self.devices.borrow().clone()
    }
    fn supports_trust(&self) -> bool {
        true
    }
    fn set_trusted(&self, address: &str, value: bool) {
        self.devices
            .borrow_mut()
            .iter_mut()
            .find(|device| device.address == address)
            .unwrap()
            .trusted = value;
    }
    fn connect(&self, address: &str) {
        *self.prompt.borrow_mut() = Some(PairingPrompt {
            id: 1,
            address: address.into(),
            kind: PairingKind::Confirmation,
            code: Some("002473".into()),
        });
    }
    fn pairing_prompt(&self) -> Option<PairingPrompt> {
        self.prompt.borrow().clone()
    }
    fn respond_to_pairing(&self, _: u64, response: PairingResponse) {
        self.responses.borrow_mut().push(response);
        self.prompt.borrow_mut().take();
    }
}

struct Brightness(Cell<f32>);
impl BrightnessIntegration for Brightness {
    fn level(&self) -> f32 {
        self.0.get()
    }
    fn set_level(&self, value: f32) {
        self.0.set(value);
    }
}
struct Battery;
impl BatteryIntegration for Battery {
    fn percentage(&self) -> Option<u8> {
        Some(78)
    }
    fn charging(&self) -> bool {
        false
    }
    fn details(&self) -> BatteryDetails {
        BatteryDetails {
            time_remaining_seconds: Some(7200),
            health_percent: Some(96),
            power_watts: Some(8.4),
            cycles: Some(131),
        }
    }
}
struct Power(RefCell<Vec<PowerProfile>>);
impl PowerProfileIntegration for Power {
    fn profiles(&self) -> Vec<PowerProfile> {
        self.0.borrow().clone()
    }
    fn set_profile(&self, id: &str) {
        for profile in self.0.borrow_mut().iter_mut() {
            profile.active = profile.id == id;
        }
    }
}

struct Fixture {
    shared: SharedState,
    scratch: SharedState,
    sound: Rc<Sound>,
    network: Rc<Network>,
    bluetooth: Rc<Bluetooth>,
    brightness: Rc<Brightness>,
    power: Rc<Power>,
}
impl Fixture {
    fn new() -> Self {
        let shared = SharedState::default();
        let sound = Rc::new(Sound::new());
        let network = Rc::new(Network::default());
        let bluetooth = Rc::new(Bluetooth::new());
        let brightness = Rc::new(Brightness(Cell::new(0.65)));
        let power = Rc::new(Power(RefCell::new(vec![
            PowerProfile {
                id: "power-saver".into(),
                active: false,
            },
            PowerProfile {
                id: "balanced".into(),
                active: true,
            },
            PowerProfile {
                id: "performance".into(),
                active: false,
            },
        ])));
        shared.insert(sound.clone() as Rc<dyn VolumeIntegration>);
        shared.insert(network.clone() as Rc<dyn NetworkIntegration>);
        shared.insert(bluetooth.clone() as Rc<dyn BluetoothIntegration>);
        shared.insert(brightness.clone() as Rc<dyn BrightnessIntegration>);
        shared.insert(Rc::new(Battery) as Rc<dyn BatteryIntegration>);
        shared.insert(power.clone() as Rc<dyn PowerProfileIntegration>);
        shared.insert(VolumeLevel(Signal::new(0.42)));
        shared.insert(BrightnessLevel(Signal::new(0.65)));
        shared.insert(WifiEnabled(Signal::new(true)));
        shared.insert(BluetoothPowered(Signal::new(true)));
        let awake = Signal::new(false);
        shared.insert(KeepAwakeState(awake.clone()));
        shared.insert(ToggleKeepAwake(Rc::new(move || awake.set(!awake.peek()))));
        Self {
            shared,
            scratch: SharedState::default(),
            sound,
            network,
            bluetooth,
            brightness,
            power,
        }
    }
}

#[test]
fn sound_output_input_and_application_controls_dispatch_to_the_right_target() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let f = Fixture::new();
            let size = Size {
                width: 380.0,
                height: 620.0,
            };
            let build = || volume::build_content(&f.shared, &f.scratch, size, None);
            let view = render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-output"),
            );
            view.click("Use");
            assert!(
                f.sound
                    .devices()
                    .iter()
                    .find(|device| device.id == "headphones")
                    .unwrap()
                    .active
            );
            assert_eq!(
                f.sound.actions.borrow().last().unwrap(),
                "default:headphones"
            );
            view.click("Input");
            let view = render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-input"),
            );
            view.drag_first(100.0, 180..420, 0.8);
            assert_eq!(f.sound.actions.borrow().last().unwrap(), "device:mic");
            view.click("Use");
            assert!(
                f.sound
                    .devices()
                    .iter()
                    .find(|device| device.id == "built-in-mic")
                    .unwrap()
                    .active
            );
            assert!(
                f.sound
                    .devices()
                    .iter()
                    .find(|device| device.id == "headphones")
                    .unwrap()
                    .active
            );

            view.click("Applications");
            let view = render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-applications"),
            );
            view.drag_first(100.0, 180..420, 0.6);
            assert_eq!(f.sound.actions.borrow().last().unwrap(), "stream:music");
            let rect = view
                .labels
                .iter()
                .find(|(text, _)| text == "Music")
                .unwrap()
                .1;
            view.scene
                .hit_test(Point {
                    x: size.width - 51.0,
                    y: rect.y + 18.0,
                })
                .expect("the application's mute action is visible")();
            assert!(f.sound.streams.borrow()[0].muted);
            render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-muted"),
            );

            f.sound.streams.borrow_mut().clear();
            render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-empty"),
            );
            *f.sound.error.borrow_mut() =
                Some("The audio device was disconnected. Choose another output.".into());
            render(
                build,
                size,
                theme,
                &format!("{variant}-details-audio-error"),
            );
        });
    }
}

#[test]
fn wifi_password_submit_cancel_saved_networks_and_disconnect_are_usable() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let f = Fixture::new();
        let size = Size {
            width: 380.0,
            height: 620.0,
        };
        let build = || network::build_content(&f.shared, &f.scratch, None, size);
        let view = render(build, size, theme, "light-details-network");
        view.click("Guest network");
        let view = render(build, size, theme, "light-details-network-password");
        let focus = view
            .scene
            .modal_focus()
            .expect("the password dialog must receive focus");
        assert!(view.scene.focus_accepts_text_input(focus));
        let join = f.scratch.get::<network_join::WifiJoin>().unwrap();
        join.password.set_value("fixture password");
        let view = render(build, size, theme, "light-details-network-password-masked");
        assert!(view
            .labels
            .iter()
            .all(|(text, _)| !text.contains("fixture password")));
        view.scene
            .on_key_at(view.scene.modal_focus().unwrap())
            .unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(
            &f.network.joined.borrow()[0],
            &("Guest network".into(), "fixture password".into())
        );
        assert!(join.password.value().is_empty());
        let view = render(build, size, theme, "light-details-network-joined");
        view.click("Guest network");
        let view = render(build, size, theme, "light-details-network-cancel");
        view.scene
            .on_key_at(view.scene.modal_focus().unwrap())
            .unwrap()(KeyInput {
            key: Key::Escape,
            modifiers: Default::default(),
        });
        assert!(join.network.peek().is_none());
        let view = render(build, size, theme, "light-details-network-list");
        view.click("Saved");
        let view = render(build, size, theme, "light-details-network-saved");
        assert!(view.labels.iter().any(|(text, _)| text == "Home network"));
        view.click("Coconut office");
        let view = render(build, size, theme, "light-details-network-interface");
        view.click("Disconnect");
        assert_eq!(&f.network.disconnected.borrow()[0], "wlan0");
    });
}

#[test]
fn bluetooth_pairing_confirmation_and_trust_actions_work() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::dark());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let f = Fixture::new();
        let size = Size {
            width: 380.0,
            height: 620.0,
        };
        let build = || bluetooth::build_content(&f.shared, &f.scratch, None, size);
        let view = render(build, size, theme, "dark-details-bluetooth");
        view.click("Travel keyboard");
        let view = render(build, size, theme, "dark-details-bluetooth-device");
        view.click("Pair & connect");
        let view = render(build, size, theme, "dark-details-bluetooth-pairing");
        view.click("Codes match");
        assert!(matches!(
            f.bluetooth.responses.borrow().last(),
            Some(PairingResponse::Accept)
        ));
        *f.bluetooth.prompt.borrow_mut() = Some(PairingPrompt {
            id: 2,
            address: "AA:00:00:00:00:02".into(),
            kind: PairingKind::Pin,
            code: None,
        });
        let mut view = render(build, size, theme, "dark-details-bluetooth-pin");
        for character in "0123".chars() {
            let focus = view.scene.modal_focus().unwrap();
            assert!(view.scene.focus_accepts_text_input(focus));
            view.scene.on_key_at(focus).unwrap()(KeyInput {
                key: Key::Char(character),
                modifiers: Default::default(),
            });
            view = render(build, size, theme, "dark-details-bluetooth-pin");
        }
        view.scene
            .on_key_at(view.scene.modal_focus().unwrap())
            .unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Default::default(),
        });
        assert!(
            matches!(f.bluetooth.responses.borrow().last(), Some(PairingResponse::Code(code)) if code == "0123")
        );
        *f.bluetooth.prompt.borrow_mut() = Some(PairingPrompt {
            id: 3,
            address: "AA:00:00:00:00:02".into(),
            kind: PairingKind::Confirmation,
            code: Some("123456".into()),
        });
        let view = render(build, size, theme, "dark-details-bluetooth-reject");
        view.scene
            .on_key_at(view.scene.modal_focus().unwrap())
            .unwrap()(KeyInput {
            key: Key::Escape,
            modifiers: Default::default(),
        });
        assert!(matches!(
            f.bluetooth.responses.borrow().last(),
            Some(PairingResponse::Reject)
        ));
        f.scratch.get::<BluetoothDetailView>().unwrap().0.set(None);
        let view = render(build, size, theme, "dark-details-bluetooth-list");
        view.click("Studio headphones");
        let view = render(build, size, theme, "dark-details-bluetooth-trust");
        let rect = view
            .labels
            .iter()
            .find(|(text, _)| text == "Allow automatic connections")
            .unwrap()
            .1;
        view.scene
            .hit_test(Point {
                x: 320.0,
                y: rect.y + rect.height / 2.0,
            })
            .unwrap()();
        assert!(!f.bluetooth.devices.borrow()[0].trusted);
    });
}

#[test]
fn brightness_presets_power_profiles_and_compact_detail_views_render() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (width, height) in [(380.0, 620.0), (320.0, 480.0)] {
            creamui_reactive::with_context_scope(|| {
                design::load_fonts();
                let theme = design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let f = Fixture::new();
                let size = Size { width, height };
                let view = render(
                    || brightness::build_content(&f.shared, &f.scratch, size, None),
                    size,
                    theme,
                    &format!("{variant}-{width}-details-brightness"),
                );
                view.click("50%");
                assert_eq!(f.brightness.level(), 0.5);
                let view = render(
                    || energy::build_content(&f.shared, &f.scratch, size, None),
                    size,
                    theme,
                    &format!("{variant}-{width}-details-power"),
                );
                view.click("Power Saver");
                assert!(
                    f.power
                        .profiles()
                        .iter()
                        .find(|profile| profile.id == "power-saver")
                        .unwrap()
                        .active
                );
                render(
                    || network::build_content(&f.shared, &f.scratch, None, size),
                    size,
                    theme,
                    &format!("{variant}-{width}-details-network"),
                );
                render(
                    || bluetooth::build_content(&f.shared, &f.scratch, None, size),
                    size,
                    theme,
                    &format!("{variant}-{width}-details-bluetooth"),
                );
                render(
                    || volume::build_content(&f.shared, &f.scratch, size, None),
                    size,
                    theme,
                    &format!("{variant}-{width}-details-output"),
                );
            });
        }
    }
}
