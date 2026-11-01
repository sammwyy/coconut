use super::control_editor::{self, Editor, GAP, ROW_HEIGHT};
use super::control_tiles::{awake_tile, battery_tile, device_tile, slider_tile};
use crate::config::TrayConfig;
use crate::control_layout::COLUMNS;
use crate::panels::{bluetooth, brightness, energy, network, volume};
use crate::shared::{
    BatteryRevision, BluetoothPowered, BluetoothRevision, BrightnessLevel, KeepAwakeState,
    NetworkRevision, PowerProfileRevision, ToggleKeepAwake, VolumeLevel, WifiEnabled,
};
use crate::{Control, ControlLayout};
use coconut_api::battery::{BatteryIntegration, Fallback as BatteryFallback};
use coconut_api::bluetooth::{BluetoothIntegration, Fallback as BluetoothFallback};
use coconut_api::brightness::{BrightnessIntegration, Fallback as BrightnessFallback};
use coconut_api::network::{Fallback as NetworkFallback, NetworkIntegration};
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::{
    shell_border, shell_card, shell_muted, shell_text, wifi_strength_icon, ISLAND_RADIUS,
};
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::layout::Style as LayoutStyle;
use creamui_core::{Border, BoxedWidget, Size, StyleProp, Styled};
use creamui_reactive::Signal;
use creamui_widgets::{
    layout::{fixed, Align, Flex, Grid, StyleExt, Track},
    ContextMenu, RawScrollView, RawText, RawView,
};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

#[cfg(test)]
#[path = "control_center_tests.rs"]
mod tests;

/// Which content the control center popup currently shows, stored per-open
/// via `ctx.scratch.get_or_insert_with(|| Signal::new(PanelView::Main))` so
/// drilling into a device and back is a pure signal flip — the popup window
/// itself is never closed and reopened. Ported from
/// `desktop/shell/src/panels/control_center/mod.rs::PanelView`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelView {
    Main,
    Network,
    Bluetooth,
    Energy,
    Brightness,
    Volume,
}

/// The `"control_center"` panel: opened either directly (grouped tray mode's
/// single fused button) or from any device panel's own entry point. Its
/// device tiles navigate *within this same popup* by flipping the
/// `PanelView` scratch signal (mirroring the old hand-created
/// `Signal::new(control_center::PanelView::Main)` in
/// `desktop/shell/src/lib.rs`) rather than opening a separate popup — unlike
/// the individual-mode tray icons (and this panel's own back button), which
/// reach the exact same 5 device panels as freestanding popups via
/// `PanelHost`'s normal `open_panel("network"|...)` dispatch. Both paths
/// call the same `build_content` function in each device panel's module
/// (`network::build_content`, `bluetooth::build_content`, etc.), so there is
/// exactly one implementation of each device's UI; only whether an
/// `on_back` closure is threaded through differs.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::network::NetworkIntegration>`
/// - `Rc<dyn coconut_api::bluetooth::BluetoothIntegration>`
/// - `Rc<dyn coconut_api::battery::BatteryIntegration>`
/// - `Rc<dyn coconut_api::volume::VolumeIntegration>`
/// - `Rc<dyn coconut_api::brightness::BrightnessIntegration>`
/// - `Rc<dyn coconut_api::power_profile::PowerProfileIntegration>`
/// - [`crate::WifiEnabled`] (`Signal<bool>`)
/// - [`crate::BluetoothPowered`] (`Signal<bool>`)
/// - [`crate::BrightnessLevel`] (`Signal<f32>`)
/// - [`crate::VolumeLevel`] (`Signal<f32>`)
/// - [`crate::KeepAwakeState`] (`Signal<bool>`)
/// - [`crate::ToggleKeepAwake`] (`Rc<dyn Fn()>`)
/// - [`crate::NetworkRevision`], [`crate::BluetoothRevision`],
///   [`crate::BatteryRevision`], [`crate::PowerProfileRevision`]
///   (`Signal<()>` each) — read unconditionally on every rebuild, regardless
///   of which `PanelView` is showing (matching `desktop/shell/src/lib.rs`'s old
///   `open_control_center` popup builder, which subscribed to all four
///   before ever dispatching on view), purely for their reactive
///   subscription.
///
/// Drilling into `Network`/`Bluetooth` additionally relies on everything
/// documented on [`network::NetworkPanel`]/[`bluetooth::BluetoothPanel`]
/// being pre-populated too, since `build_content` is shared verbatim
/// (including their own `scratch`-only state).
pub struct ControlCenterPanel {
    tray: TrayConfig,
    layout: Signal<ControlLayout>,
    app: Option<creamui_render::AppHandle>,
}

impl ControlCenterPanel {
    pub fn new(tray: TrayConfig) -> Self {
        let mut layout: ControlLayout = coconut_core::modules::load_module("control-center");
        layout.normalize();
        Self {
            tray,
            layout: Signal::new(layout),
            app: None,
        }
    }

    pub fn with_app(mut self, app: creamui_render::AppHandle) -> Self {
        self.app = Some(app);
        self
    }

    fn available(&self) -> Vec<Control> {
        Control::ALL
            .into_iter()
            .filter(|control| match control {
                Control::Wifi => self.tray.wifi.shows_in_panel(),
                Control::Bluetooth => self.tray.bluetooth.shows_in_panel(),
                Control::Battery => self.tray.battery.shows_in_panel(),
                Control::Brightness => self.tray.brightness.shows_in_panel(),
                Control::Volume => self.tray.volume.shows_in_panel(),
                Control::KeepAwake => true,
            })
            .collect()
    }
}

impl Panel for ControlCenterPanel {
    fn id(&self) -> &'static str {
        "control_center"
    }

    fn title(&self) -> &'static str {
        "Coconut Control Center"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn preferred_size(&self, scratch: &coconut_plugin_kit::SharedState) -> Size {
        let view = scratch.get_or_insert_with(|| Signal::new(PanelView::Main));
        if view.get() != PanelView::Main {
            return self.size();
        }
        let editor = scratch.get_or_insert_with(|| Editor::new(self.layout.peek()));
        let mut visible = editor.layout.get();
        visible
            .tiles
            .retain(|tile| self.available().contains(&tile.control));
        let rows = visible.rows().max(
            editor
                .preview
                .get()
                .map_or(0, |tile| tile.row + u16::from(tile.height)),
        );
        let grid_height = (f32::from(rows) * (ROW_HEIGHT + GAP) - GAP).max(ROW_HEIGHT);
        Size {
            width: WIDTH as f32,
            height: grid_height
                + if editor.editing.get() { 144.0 } else { 78.0 }
                + if editor.error.get().is_some() {
                    32.0
                } else {
                    0.0
                },
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        // Read unconditionally, regardless of which `PanelView` is showing —
        // see the doc comment above.
        if let Some(revision) = ctx.shared.get::<NetworkRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BluetoothRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BatteryRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<PowerProfileRevision>() {
            revision.0.get();
        }

        let view = ctx
            .scratch
            .get_or_insert_with(|| Signal::new(PanelView::Main));

        match view.get() {
            PanelView::Network => {
                return network::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    Some(back_to_main(view.clone())),
                    ctx.size,
                );
            }
            PanelView::Bluetooth => {
                return bluetooth::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    Some(back_to_main(view.clone())),
                    ctx.size,
                );
            }
            PanelView::Energy => {
                return energy::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    ctx.size,
                    Some(back_to_main(view.clone())),
                );
            }
            PanelView::Brightness => {
                return brightness::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    ctx.size,
                    Some(back_to_main(view.clone())),
                );
            }
            PanelView::Volume => {
                return volume::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    ctx.size,
                    Some(back_to_main(view.clone())),
                );
            }
            PanelView::Main => {}
        }

        let network = ctx
            .shared
            .get::<Rc<dyn NetworkIntegration>>()
            .unwrap_or_else(|| Rc::new(NetworkFallback));
        let bluetooth = ctx
            .shared
            .get::<Rc<dyn BluetoothIntegration>>()
            .unwrap_or_else(|| Rc::new(BluetoothFallback));
        let battery = ctx
            .shared
            .get::<Rc<dyn BatteryIntegration>>()
            .unwrap_or_else(|| Rc::new(BatteryFallback));
        let volume = ctx
            .shared
            .get::<Rc<dyn VolumeIntegration>>()
            .unwrap_or_else(|| Rc::new(VolumeFallback));
        let brightness = ctx
            .shared
            .get::<Rc<dyn BrightnessIntegration>>()
            .unwrap_or_else(|| Rc::new(BrightnessFallback));
        let wifi_enabled = ctx
            .shared
            .get::<WifiEnabled>()
            .unwrap_or_else(|| WifiEnabled(Signal::new(network.enabled())))
            .0;
        let bluetooth_powered = ctx
            .shared
            .get::<BluetoothPowered>()
            .unwrap_or_else(|| BluetoothPowered(Signal::new(bluetooth.powered())))
            .0;
        let brightness_level = ctx
            .shared
            .get::<BrightnessLevel>()
            .unwrap_or_else(|| BrightnessLevel(Signal::new(brightness.level())))
            .0;
        let volume_level = ctx
            .shared
            .get::<VolumeLevel>()
            .unwrap_or_else(|| VolumeLevel(Signal::new(volume.level())))
            .0;
        let awake = ctx
            .shared
            .get::<KeepAwakeState>()
            .unwrap_or_else(|| KeepAwakeState(Signal::new(false)))
            .0
            .get();
        let toggle_awake = ctx
            .shared
            .get::<ToggleKeepAwake>()
            .unwrap_or_else(|| ToggleKeepAwake(Rc::new(|| {})))
            .0;

        let wifi_on = wifi_enabled.get();
        let connected = wifi_on && network.connected();
        let network_name = if wifi_on {
            network
                .network_name()
                .unwrap_or_else(|| "No network".into())
        } else {
            "Off".into()
        };
        let wifi_icon = if !wifi_on {
            "wifi-slash"
        } else {
            wifi_strength_icon(network.strength().unwrap_or(100))
        };
        let wifi_caption = if connected {
            "Connected"
        } else if wifi_on {
            "Disconnected"
        } else {
            "Radio off"
        };

        let bluetooth_on = bluetooth_powered.get();
        let bluetooth_connected = bluetooth_on && bluetooth.connected();
        let bluetooth_icon = if bluetooth_connected {
            "bluetooth-connected"
        } else if bluetooth_on {
            "bluetooth-on"
        } else {
            "bluetooth-off"
        };
        let bluetooth_name = if bluetooth_connected {
            bluetooth
                .device_name()
                .unwrap_or_else(|| "Connected".into())
        } else if bluetooth_on {
            "On".into()
        } else {
            "Off".into()
        };
        let bluetooth_caption = if bluetooth_connected {
            "Connected"
        } else if bluetooth_on {
            "Available"
        } else {
            "Powered off"
        };

        let battery_percent = battery.percentage();
        let battery_icon = if battery.charging() {
            "battery-bolt"
        } else {
            match battery_percent.unwrap_or(0) {
                80.. => "battery-full",
                50..=79 => "battery-mid",
                20..=49 => "battery-low",
                _ => "battery-empty",
            }
        };
        let battery_caption = if battery_percent.is_none() {
            "(Unavailable)"
        } else if battery.charging() {
            "(Charging)"
        } else {
            ""
        };

        let brightness_value = brightness_level.get();
        let volume_value = volume_level.get();
        let brightness_text = format!("{}%", (brightness_value * 100.0).round());
        let volume_text = format!(
            "{}%{}",
            (volume_value * 100.0).round(),
            if volume.muted() { "  M" } else { "" }
        );
        let volume_icon = if volume.muted() || volume_value <= 0.01 {
            "volume-mute"
        } else if volume_value < 0.34 {
            "volume-low"
        } else if volume_value < 0.67 {
            "volume-mid"
        } else {
            "volume-high"
        };

        let brightness_backend = brightness.clone();
        let volume_backend = volume.clone();
        let set_brightness = brightness_level.clone();
        let set_volume = volume_level.clone();
        let toggle_wifi = {
            let network = network.clone();
            let wifi_enabled = wifi_enabled.clone();
            Rc::new(move || {
                let next = !wifi_enabled.get();
                wifi_enabled.set(next);
                network.set_enabled(next);
            })
        };
        let toggle_bluetooth = {
            let bluetooth = bluetooth.clone();
            let bluetooth_powered = bluetooth_powered.clone();
            Rc::new(move || {
                let next = !bluetooth_powered.get();
                bluetooth_powered.set(next);
                bluetooth.set_powered(next);
            })
        };

        let open_network_view = {
            let view = view.clone();
            Rc::new(move || view.set(PanelView::Network))
        };
        let open_bluetooth_view = {
            let view = view.clone();
            Rc::new(move || view.set(PanelView::Bluetooth))
        };
        let open_energy_view = {
            let view = view.clone();
            Rc::new(move || view.set(PanelView::Energy))
        };
        let open_brightness_view = {
            let view = view.clone();
            Rc::new(move || view.set(PanelView::Brightness))
        };
        let open_volume_view = {
            let view = view.clone();
            Rc::new(move || view.set(PanelView::Volume))
        };

        let editor = ctx
            .scratch
            .get_or_insert_with(|| Editor::new(self.layout.peek()));
        let editing = editor.editing.get();
        let layout = editor.layout.get();
        let available = self.available();
        let width = (ctx.size.width - 34.0).max(0.0);
        let grid_width = (width - 8.0).max(0.0);
        let column_width = (grid_width - GAP * f32::from(COLUMNS - 1)) / f32::from(COLUMNS);
        let mut visible = layout.clone();
        visible
            .tiles
            .retain(|tile| available.contains(&tile.control));
        visible.tiles.sort_by_key(|tile| tile.control as u8);
        let preview = editor.preview.get();
        let rows = visible
            .rows()
            .max(preview.map_or(0, |tile| tile.row + u16::from(tile.height)));
        let grid_height = (f32::from(rows) * (ROW_HEIGHT + GAP) - GAP).max(ROW_HEIGHT);
        let mut grid = Grid::new()
            .columns(usize::from(COLUMNS))
            .gap(GAP)
            .template_rows((0..rows.max(1)).map(|_| Track::px(ROW_HEIGHT)))
            .size(grid_width, grid_height);
        for tile in visible.tiles {
            let size = Size {
                width: f32::from(tile.width) * (column_width + GAP) - GAP,
                height: f32::from(tile.height) * (ROW_HEIGHT + GAP) - GAP,
            };
            let content = if editing {
                editor.tile(tile, size, column_width)
            } else {
                let content = match tile.control {
                    Control::Wifi => device_tile(
                        size,
                        wifi_icon,
                        "Wi-Fi",
                        network_name.clone(),
                        wifi_caption,
                        wifi_on,
                        toggle_wifi.clone(),
                        open_network_view.clone(),
                    ),
                    Control::Bluetooth => device_tile(
                        size,
                        bluetooth_icon,
                        "Bluetooth",
                        bluetooth_name.clone(),
                        bluetooth_caption,
                        bluetooth_on,
                        toggle_bluetooth.clone(),
                        open_bluetooth_view.clone(),
                    ),
                    Control::Battery => battery_tile(
                        size,
                        battery_icon,
                        battery_percent,
                        battery_caption,
                        open_energy_view.clone(),
                    ),
                    Control::KeepAwake => awake_tile(size, awake, toggle_awake.clone()),
                    Control::Brightness => slider_tile(
                        size,
                        "brightness",
                        "Brightness",
                        brightness_value,
                        brightness_text.clone(),
                        open_brightness_view.clone(),
                        {
                            let set_brightness = set_brightness.clone();
                            let brightness_backend = brightness_backend.clone();
                            move |level| {
                                set_brightness.set(level);
                                brightness_backend.set_level(level);
                            }
                        },
                    ),
                    Control::Volume => slider_tile(
                        size,
                        volume_icon,
                        "Volume",
                        volume_value,
                        volume_text.clone(),
                        open_volume_view.clone(),
                        {
                            let set_volume = set_volume.clone();
                            let volume_backend = volume_backend.clone();
                            move |level| {
                                set_volume.set(level);
                                volume_backend.set_level(level);
                            }
                        },
                    ),
                };
                let edit = editor.clone();
                Box::new(
                    RawView::new(LayoutStyle::default())
                        .with_context_menu(move |point| edit.open_tile(point, tile))
                        .child(content),
                ) as BoxedWidget
            };
            grid = grid.child(control_editor::positioned(
                content,
                tile,
                size,
                tile.control.label(),
            ));
        }
        if let Some(tile) = preview {
            let mut tile = tile;
            tile.width = tile.width.clamp(1, COLUMNS);
            tile.column = tile.column.min(COLUMNS - tile.width);
            let size = Size {
                width: f32::from(tile.width) * (column_width + GAP) - GAP,
                height: f32::from(tile.height) * (ROW_HEIGHT + GAP) - GAP,
            };
            let ghost = Flex::column()
                .size(size.width, size.height)
                .padding(8.0)
                .property(StyleProp::Border(Border::new(
                    coconut_plugin_kit::chrome::shell_accent(),
                    2.0,
                )))
                .property(StyleProp::CornerRadius(ISLAND_RADIUS));
            grid = grid.child(control_editor::positioned(
                Box::new(ghost),
                tile,
                size,
                "drop-preview",
            ));
        }
        if rows == 0 {
            grid = grid.child(Box::new(
                RawView::new(LayoutStyle::default().grid_column_span(u16::from(COLUMNS))).child(
                    Box::new(RawText::new(
                        "No controls. Choose Edit, then Add.",
                        shell_muted(),
                        13.0,
                    )),
                ),
            ));
        }
        let error = editor.error.get();
        let mut header = Flex::row().align(Align::Center).gap(8.0).child(
            coconut_plugin_kit::chrome::settings_panel_header(
                "system",
                "Control center",
                "/connectivity",
                None,
            ),
        );
        if !editing {
            header = header
                .child(Box::new(Flex::row().grow(1.0)))
                .child(editor.toolbar(self.layout.clone(), self.app.clone(), available.clone()));
        }
        let mut root = Flex::column()
            .size(ctx.size.width, ctx.size.height)
            .padding(16.0)
            .gap(12.0)
            .property(StyleProp::Background(shell_card().into()))
            .property(StyleProp::Border(Border::new(shell_border(), 1.0)))
            .child(Box::new(header));
        if editing {
            root = root
                .child(editor.toolbar(self.layout.clone(), self.app.clone(), available.clone()))
                .child(Box::new(RawText::new(
                    "Drag to move · Drag the corner to resize",
                    shell_muted(),
                    11.0,
                )));
        }
        if let Some(error) = error.as_ref() {
            root = root.child(Box::new(RawText::new(error.clone(), shell_text(), 12.0)));
        }
        let viewport_height = (ctx.size.height
            - if editing { 144.0 } else { 78.0 }
            - if error.is_some() { 32.0 } else { 0.0 })
        .max(0.0);
        editor.scroll.set(
            editor
                .scroll
                .peek()
                .min((grid_height - viewport_height).max(0.0)),
        );
        root = root.child(Box::new(
            RawScrollView::controlled(
                control_editor::scroll_style(width, viewport_height),
                editor.scroll.clone(),
            )
            .child(Box::new(grid)),
        ));
        let menu = editor.clone();
        let mut root = RawView::new(LayoutStyle {
            size: fixed(ctx.size.width, ctx.size.height),
            ..Default::default()
        })
        .with_context_menu(move |point| menu.open_add(point, &available))
        .child(Box::new(root));
        if editor.menu.is_open() {
            let menu = creamui_reactive::with_context_scope(|| {
                let mut theme = creamui_theme::use_theme();
                theme.colors.surface_elevated.a = 255;
                creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
                ContextMenu::new(&editor.menu, ctx.size)
            });
            root = root.child(Box::new(menu));
        }
        Box::new(root)
    }
}

fn back_to_main(view: Signal<PanelView>) -> Rc<dyn Fn()> {
    Rc::new(move || view.set(PanelView::Main))
}
