//! `coconut-plugin-weather`: the dock's weather island and panel, plus the
//! relocated `WeatherState`/HTTP-refresh domain logic (formerly
//! `apps/shell/src/weather.rs`). Phase 4a of the dock/island/panel refactor
//! (`~/.claude/plans/luminous-prancing-wombat.md`).

mod island;
mod panel;
mod state;

pub use island::WeatherIsland;
pub use panel::WeatherPanel;
pub use state::{
    condition_label, format_temperature, icon_for, refresh, HourlyForecast, Weather, WeatherState,
};

use coconut_plugin_kit::{Island, Panel, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn id(&self) -> &'static str {
        "weather"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(WeatherIsland)]
    }

    fn panels(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        vec![Rc::new(WeatherPanel)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_island_and_panel_ids_match_shell_toml() {
        assert_eq!(WeatherPlugin.id(), "weather");
        assert_eq!(WeatherIsland.id(), "weather");
        assert_eq!(WeatherPanel.id(), "weather");
    }
}
