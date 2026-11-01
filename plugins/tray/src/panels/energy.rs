use crate::shared::{BatteryRevision, KeepAwakeState, PowerProfileRevision, ToggleKeepAwake};
use coconut_api::battery::{BatteryIntegration, Fallback as BatteryFallback};
use coconut_api::power_profile::{
    Fallback as PowerProfileFallback, PowerProfile, PowerProfileIntegration,
};
use coconut_plugin_kit::chrome::{
    compact_hero_card, list_row, section_label, settings_panel_header, shell_text,
};
use coconut_plugin_kit::{pixel_icon, Panel, PanelRenderContext, SharedState};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::layout::Flex;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

/// The device panel wrapper opened directly by the `"tray.battery"` island
/// (individual tray mode) or by `PanelHost` for any other caller. Delegates
/// to [`build_content`] with `on_back: None`.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::battery::BatteryIntegration>`
/// - `Rc<dyn coconut_api::power_profile::PowerProfileIntegration>`
/// - [`crate::KeepAwakeState`] (`Signal<bool>`)
/// - [`crate::ToggleKeepAwake`] (`Rc<dyn Fn()>`)
/// - [`crate::BatteryRevision`], [`crate::PowerProfileRevision`]
///   (`Signal<()>` each) — read for their reactive subscription only.
pub struct EnergyPanel;

impl Panel for EnergyPanel {
    fn id(&self) -> &'static str {
        "energy"
    }

    fn title(&self) -> &'static str {
        "Coconut Energy"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        build_content(&ctx.shared, &ctx.scratch, ctx.size, None)
    }
}

/// The energy panel's actual content, shared between [`EnergyPanel`] and
/// [`crate::panels::control_center::ControlCenterPanel`]'s embedded Energy
/// view. Ported from `desktop/shell/src/panels/energy/mod.rs::build`.
pub fn build_content(
    shared: &SharedState,
    scratch: &SharedState,
    size: Size,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let battery = shared
        .get::<Rc<dyn BatteryIntegration>>()
        .unwrap_or_else(|| Rc::new(BatteryFallback));
    let power_profile = shared
        .get::<Rc<dyn PowerProfileIntegration>>()
        .unwrap_or_else(|| Rc::new(PowerProfileFallback));
    let awake = shared
        .get::<KeepAwakeState>()
        .unwrap_or_else(|| KeepAwakeState(Signal::new(false)))
        .0
        .get();
    let toggle_awake = shared
        .get::<ToggleKeepAwake>()
        .unwrap_or_else(|| ToggleKeepAwake(Rc::new(|| {})))
        .0;
    if let Some(revision) = shared.get::<BatteryRevision>() {
        revision.0.get();
    }
    if let Some(revision) = shared.get::<PowerProfileRevision>() {
        revision.0.get();
    }

    let battery_percent = battery.percentage();
    let has_battery = battery_percent.is_some();
    let profiles = power_profile.profiles();
    let active_profile = profiles.iter().find(|profile| profile.active);

    let (hero_icon, hero_title, hero_caption) = if has_battery {
        let icon = if battery.charging() {
            "battery-bolt"
        } else {
            match battery_percent.unwrap_or(0) {
                80.. => "battery-full",
                50..=79 => "battery-mid",
                20..=49 => "battery-low",
                _ => "battery-empty",
            }
        };
        let value = battery_percent
            .map(|percent| format!("{percent}%"))
            .unwrap_or_else(|| "--".into());
        let caption = if battery.charging() && battery_percent == Some(100) {
            "Fully charged"
        } else if battery.charging() {
            "Charging"
        } else {
            "On battery"
        };
        (icon, value, caption.to_owned())
    } else {
        // No battery (a desktop, or a laptop UPower can't see one on):
        // lead with the active power plan instead of a battery reading.
        let icon = active_profile
            .map(|profile| profile_icon(&profile.id))
            .unwrap_or("power-plan");
        let title = active_profile
            .map(|profile| profile_label(&profile.id).to_owned())
            .unwrap_or_else(|| "Plugged in".to_owned());
        (icon, title, "Power plan".to_owned())
    };

    let mut profiles_section = Flex::column().gap(8.0);
    if !profiles.is_empty() {
        profiles_section = profiles_section.child(section_label("POWER PLAN"));
        for profile in &profiles {
            profiles_section = profiles_section.child(profile_row(profile, power_profile.clone()));
        }
    }

    let scroll = scratch
        .get_or_insert_with(|| PowerScroll(creamui_widgets::ScrollController::new(0.0)))
        .0;
    let mut content = Flex::column()
        .gap(12.0)
        .child(compact_hero_card(hero_icon, hero_title, hero_caption))
        .child(super::detail::card(Box::new(
            Flex::column()
                .gap(8.0)
                .child(super::detail::toggle("Keep awake", awake, toggle_awake))
                .child(super::detail::text(
                    if awake {
                        "Automatic sleep is inhibited."
                    } else {
                        "Automatic sleep is allowed."
                    },
                    false,
                    12.0,
                )),
        )))
        .child(Box::new(profiles_section));
    if profiles.is_empty() {
        content = content.child(super::detail::text(
            "No power profile service is available.",
            false,
            12.0,
        ));
    }
    if let Some(error) = power_profile.last_error() {
        content = content.child(super::detail::error(error));
    }
    if has_battery {
        let details = battery.details();
        let mut readings = Flex::column().gap(8.0);
        if let Some(seconds) = details.time_remaining_seconds {
            let minutes = seconds / 60;
            readings = readings.child(coconut_plugin_kit::chrome::detail_row(
                if battery.charging() {
                    "Until charged"
                } else {
                    "Time remaining"
                },
                format!("{} h {} min", minutes / 60, minutes % 60),
            ));
        }
        if let Some(health) = details.health_percent {
            readings = readings.child(coconut_plugin_kit::chrome::detail_row(
                "Battery health",
                format!("{health}%"),
            ));
        }
        if let Some(watts) = details.power_watts {
            readings = readings.child(coconut_plugin_kit::chrome::detail_row(
                if battery.charging() {
                    "Charge rate"
                } else {
                    "Discharge rate"
                },
                format!("{watts:.1} W"),
            ));
        }
        if let Some(cycles) = details.cycles {
            readings = readings.child(coconut_plugin_kit::chrome::detail_row(
                "Charge cycles",
                cycles.to_string(),
            ));
        }
        if details.time_remaining_seconds.is_some()
            || details.health_percent.is_some()
            || details.power_watts.is_some()
            || details.cycles.is_some()
        {
            content = content.child(super::detail::card(Box::new(readings)));
        }
    }
    super::detail::panel(
        size,
        settings_panel_header("battery-full", "Power", "/hardware", on_back),
        scroll,
        Box::new(content),
    )
}

fn profile_row(
    profile: &PowerProfile,
    power_profile: Rc<dyn PowerProfileIntegration>,
) -> BoxedWidget {
    let id = profile.id.clone();
    let on_select = Rc::new(move || power_profile.set_profile(&id));
    list_row(
        profile_icon(&profile.id),
        profile_label(&profile.id).to_owned(),
        profile_description(&profile.id).to_owned(),
        profile
            .active
            .then(|| pixel_icon("check", 14.0, shell_text())),
        profile.active,
        Some(on_select),
    )
}

fn profile_icon(id: &str) -> &'static str {
    match id {
        "power-saver" => "power-saver",
        "performance" => "power-performance",
        _ => "power-balanced",
    }
}

fn profile_label(id: &str) -> &str {
    match id {
        "power-saver" => "Power Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

fn profile_description(id: &str) -> &str {
    match id {
        "power-saver" => "Reduced power use",
        "balanced" => "Default performance",
        "performance" => "Maximum performance",
        _ => "",
    }
}

#[derive(Clone)]
struct PowerScroll(creamui_widgets::ScrollController);
