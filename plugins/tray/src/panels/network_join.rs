use coconut_api::network::{NetworkIntegration, WifiNetwork};
use creamui_core::{BoxedWidget, Key, Size};
use creamui_reactive::{batch, Signal};
use creamui_widgets::{
    layout::Flex, ButtonVariant, Dialog, DialogContent, DialogHeader, TextController, TextInput,
};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct WifiJoin {
    pub network: Signal<Option<WifiNetwork>>,
    pub password: TextController,
    error: Signal<Option<String>>,
}

impl WifiJoin {
    pub fn new() -> Self {
        Self {
            network: Signal::new(None),
            password: TextController::new(""),
            error: Signal::new(None),
        }
    }
    pub fn open(&self, network: WifiNetwork) {
        batch(|| {
            self.password.set_value("");
            self.error.set(None);
            self.network.set(Some(network));
        });
    }
    fn close(&self) {
        batch(|| {
            self.network.set(None);
            self.password.set_value("");
            self.error.set(None);
        });
    }
    pub fn dialog(&self, backend: Rc<dyn NetworkIntegration>, size: Size) -> Option<BoxedWidget> {
        let network = self.network.get()?;
        let cancel = self.clone();
        let supported = backend.supports_password_connection()
            && matches!(
                network.security.as_str(),
                "WPA Personal" | "WPA2 Personal" | "WPA3 Personal"
            );
        if !supported {
            let close = self.clone();
            return Some(Box::new(
                Dialog::new(
                    network.ssid,
                    "Configure this network's security in Network settings.",
                    move || cancel.close(),
                )
                .width(size.width - 32.0)
                .dismiss_action("Cancel")
                .action("Network settings", ButtonVariant::Primary, move || {
                    close.close();
                    coconut_plugin_kit::open_settings("/connectivity/wifi/nearby");
                }),
            ));
        }
        let state = self.clone();
        let ssid = network.ssid.clone();
        let wpa3 = network.security == "WPA3 Personal";
        let submit: Rc<dyn Fn()> = Rc::new(move || {
            let password = state.password.value();
            if password.len() < if wpa3 { 1 } else { 8 } {
                state.error.set(Some(
                    if wpa3 {
                        "Enter a password."
                    } else {
                        "Enter at least 8 characters."
                    }
                    .into(),
                ));
                return;
            }
            if backend.connecting_network().is_some() {
                state
                    .error
                    .set(Some("Another network connection is in progress.".into()));
                return;
            }
            backend.connect_wifi_with_password(&ssid, &password);
            state.close();
        });
        let enter = submit.clone();
        let escape = self.clone();
        let mut input_style = TextInput::default_style();
        input_style.size.width = creamui_core::layout::Dimension::Percent(1.0);
        let input = TextInput::controlled_with_style(input_style, &self.password)
            .password()
            .placeholder("Network password")
            .on_submit(move || enter())
            .on_key_press(move |input| {
                if input.key == Key::Escape {
                    escape.close();
                }
            });
        let mut content = Flex::column()
            .gap(10.0)
            .child(super::detail::text(
                format!("{} · {}", network.ssid, network.security),
                false,
                12.0,
            ))
            .child(Box::new(input));
        if let Some(error) = self.error.get() {
            content = content.child(super::detail::text(error, true, 12.0));
        }
        Some(Box::new(
            Dialog::custom(move || cancel.close())
                .width(size.width - 32.0)
                .header(Box::new(DialogHeader::new("Connect to Wi-Fi").compact()))
                .content(Box::new(
                    DialogContent::new().compact().child(Box::new(content)),
                ))
                .dismiss_action("Cancel")
                .action("Connect", ButtonVariant::Primary, move || submit()),
        ))
    }
}
