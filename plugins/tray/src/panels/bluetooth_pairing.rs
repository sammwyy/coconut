use coconut_api::bluetooth::{BluetoothIntegration, PairingKind, PairingResponse};
use coconut_plugin_kit::SharedState;
use creamui_core::{BoxedWidget, Key, Size};
use creamui_widgets::{
    layout::Flex, ButtonVariant, Dialog, DialogContent, DialogHeader, TextController, TextInput,
};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
struct PairingEntry {
    id: Rc<Cell<u64>>,
    input: TextController,
}

pub(super) fn overlay(
    content: BoxedWidget,
    backend: Rc<dyn BluetoothIntegration>,
    scratch: &SharedState,
    size: Size,
) -> BoxedWidget {
    let mut root = creamui_widgets::RawView::new(creamui_core::layout::Style {
        size: creamui_widgets::layout::fixed(size.width, size.height),
        ..Default::default()
    })
    .child(content);
    let Some(prompt) = backend.pairing_prompt() else {
        return Box::new(root);
    };
    let entry = scratch.get_or_insert_with(|| PairingEntry {
        id: Rc::new(Cell::new(0)),
        input: TextController::new(""),
    });
    if entry.id.get() != prompt.id {
        entry.id.set(prompt.id);
        entry.input.set_value("");
    }
    let id = prompt.id;
    let cancel_backend = backend.clone();
    let cancel: Rc<dyn Fn()> =
        Rc::new(move || cancel_backend.respond_to_pairing(id, PairingResponse::Reject));
    let submit_backend = backend.clone();
    let code = entry.input.clone();
    let kind = prompt.kind;
    let submit: Rc<dyn Fn()> = Rc::new(move || {
        submit_backend.respond_to_pairing(
            id,
            if matches!(kind, PairingKind::Pin | PairingKind::Passkey) {
                PairingResponse::Code(code.value())
            } else {
                PairingResponse::Accept
            },
        );
    });
    let name = backend
        .devices()
        .iter()
        .find(|device| device.address == prompt.address)
        .map(|device| device.name.clone())
        .unwrap_or_else(|| prompt.address.clone());
    let mut body = Flex::column()
        .gap(12.0)
        .child(super::detail::text(name, true, 14.0))
        .child(super::detail::text(
            match kind {
                PairingKind::Confirmation => {
                    "Check that this code matches the code on your device."
                }
                PairingKind::Authorization => "Allow this device to pair and connect?",
                PairingKind::Pin => "Enter the PIN shown on your Bluetooth device.",
                PairingKind::Passkey => "Enter the numeric passkey shown on your device.",
                PairingKind::Display => {
                    "Type this code on your Bluetooth device, then press Enter there."
                }
            },
            false,
            12.0,
        ));
    if let Some(code) = prompt.code {
        body = body.child(super::detail::text(code, true, 34.0));
    }
    if matches!(kind, PairingKind::Pin | PairingKind::Passkey) {
        let enter = submit.clone();
        let escape = cancel.clone();
        let mut input_style = TextInput::default_style();
        input_style.size.width = creamui_core::layout::Dimension::Percent(1.0);
        body = body.child(Box::new(
            TextInput::controlled_with_style(input_style, &entry.input)
                .placeholder("Pairing code")
                .on_submit(move || enter())
                .on_key_press(move |input| {
                    if input.key == Key::Escape {
                        escape();
                    }
                }),
        ));
    }
    if let Some(error) = backend.last_error() {
        body = body.child(super::detail::text(error, true, 12.0));
    }
    let dismiss = cancel.clone();
    let mut dialog = Dialog::custom(move || dismiss())
        .width(size.width - 32.0)
        .header(Box::new(DialogHeader::new("Bluetooth pairing").compact()))
        .content(Box::new(
            DialogContent::new().compact().child(Box::new(body)),
        ))
        .action("Cancel", ButtonVariant::Secondary, move || cancel());
    if kind != PairingKind::Display {
        dialog = dialog.action(
            if kind == PairingKind::Confirmation {
                "Codes match"
            } else if kind == PairingKind::Authorization {
                "Allow"
            } else {
                "Pair"
            },
            ButtonVariant::Primary,
            move || submit(),
        );
    }
    root = root.child(Box::new(dialog));
    Box::new(root)
}
