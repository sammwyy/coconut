use crate::popups::{PopupHost, ShellNotification};
use coconut_api::battery::BatteryIntegration;
use coconut_core::BatteryAlertsConfig;
use creamui_reactive::{create_effect, Effect, Signal};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BatteryState {
    percent: u8,
    charging: bool,
}

impl BatteryState {
    fn read(battery: &dyn BatteryIntegration) -> Option<Self> {
        battery.percentage().map(|percent| Self {
            percent,
            charging: battery.charging(),
        })
    }

    fn icon(self) -> &'static str {
        if self.charging {
            "battery-bolt"
        } else if self.percent <= 5 {
            "battery-empty"
        } else if self.percent <= 20 {
            "battery-low"
        } else if self.percent < 80 {
            "battery-mid"
        } else {
            "battery-full"
        }
    }
}

#[derive(Default)]
struct Observer {
    previous: Option<BatteryState>,
}

impl Observer {
    fn update(
        &mut self,
        next: Option<BatteryState>,
        alerts: &BatteryAlertsConfig,
    ) -> Vec<ShellNotification> {
        let previous = self.previous;
        self.previous = next;
        let Some(previous) = previous else {
            return Vec::new();
        };
        let Some(next) = next else {
            return Vec::new();
        };

        let mut notifications = Vec::new();
        if previous.charging != next.charging {
            notifications.push(ShellNotification {
                icon: next.icon(),
                summary: if next.charging {
                    "Power connected"
                } else {
                    "Running on battery"
                }
                .into(),
                body: if next.charging {
                    format!("Charging battery ({}%).", next.percent)
                } else {
                    format!("Battery at {}%.", next.percent)
                },
            });
        }

        if !next.charging {
            if crossed(previous.percent, next.percent, alerts.critical_percent) {
                notifications.push(ShellNotification {
                    icon: "battery-empty",
                    summary: "Battery critically low".into(),
                    body: format!("{}% remaining. Connect power now.", next.percent),
                });
            } else if crossed(previous.percent, next.percent, alerts.low_percent) {
                notifications.push(ShellNotification {
                    icon: "battery-low",
                    summary: "Battery low".into(),
                    body: format!("{}% remaining.", next.percent),
                });
            }
        }
        notifications
    }
}

fn crossed(previous: u8, next: u8, threshold: u8) -> bool {
    previous > threshold && next <= threshold
}

pub(crate) fn watch(
    host: &Rc<PopupHost>,
    battery: Rc<dyn BatteryIntegration>,
    revision: Signal<()>,
) -> Effect {
    let host = Rc::downgrade(host);
    let mut observer = Observer::default();
    create_effect(move || {
        revision.get();
        let alerts = host
            .upgrade()
            .map(|host| host.battery_alerts())
            .unwrap_or_default();
        let notifications = observer.update(BatteryState::read(battery.as_ref()), &alerts);
        if let Some(host) = host.upgrade() {
            for notification in notifications {
                host.notify(notification);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALERTS: BatteryAlertsConfig = BatteryAlertsConfig {
        low_percent: 10,
        critical_percent: 5,
    };

    fn state(percent: u8, charging: bool) -> BatteryState {
        BatteryState { percent, charging }
    }

    #[test]
    fn only_alerts_when_crossing_thresholds_while_unplugged() {
        let mut observer = Observer::default();
        assert!(observer.update(Some(state(12, false)), &ALERTS).is_empty());
        let low = observer.update(Some(state(10, false)), &ALERTS);
        assert_eq!(low[0].summary, "Battery low");
        assert!(observer.update(Some(state(9, false)), &ALERTS).is_empty());
        let critical = observer.update(Some(state(5, false)), &ALERTS);
        assert_eq!(critical[0].summary, "Battery critically low");
    }

    #[test]
    fn reports_power_source_changes_for_devices_with_a_battery() {
        let mut observer = Observer::default();
        assert!(observer.update(Some(state(60, false)), &ALERTS).is_empty());
        let connected = observer.update(Some(state(60, true)), &ALERTS);
        assert_eq!(connected[0].summary, "Power connected");
        let unplugged = observer.update(Some(state(60, false)), &ALERTS);
        assert_eq!(unplugged[0].summary, "Running on battery");
    }
}
