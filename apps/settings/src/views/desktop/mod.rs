use crate::{components::update_config, routes::destination::Section};
use coconut_core::{DockPosition, ShellConfig};
use creamui_core::{BoxedWidget, Styled};
use creamui_reactive::Signal;
use creamui_widgets::{Select, Slider, Switch};

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub mod bars;
pub mod icons;
pub mod island_settings;
pub mod islands;
pub mod notifications;
pub mod tray;

pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let config = &context.config;
    let view = &context.view;
    let native = &context.native;
    build(native, config, view)
}

fn build(state: &State, config: &Signal<ShellConfig>, nav: &creamui_router::Router) -> BoxedWidget {
    let c = config.get();
    let write = config.clone();
    let panel: BoxedWidget = Box::new(Switch::new(c.statusbar.enabled, move || {
        update_config(&write, |c| c.statusbar.enabled = !c.statusbar.enabled)
    }));
    let positions = [
        DockPosition::Bottom,
        DockPosition::Left,
        DockPosition::Right,
        DockPosition::Top,
    ];
    let selected = positions
        .iter()
        .position(|p| *p == c.dockbar.position)
        .unwrap_or(0);
    let write = config.clone();
    let position: BoxedWidget = Box::new(
        Select::controlled(
            &["Bottom", "Left", "Right", "Top"],
            state.select("dock-position", selected),
        )
        .width(136.0)
        .height(34.0)
        .on_select(move |i| update_config(&write, |c| c.dockbar.position = positions[i])),
    );
    let write = config.clone();
    let size: BoxedWidget = Box::new(Slider::new((c.dockbar.thickness - 32.0) / 64.0, move |v| {
        update_config(&write, |c| c.dockbar.thickness = 32.0 + v * 64.0)
    }));
    page(
        vec![
            card(
                "Top panel",
                vec![
                    item("Top panel", "Show the status bar", panel, || {}),
                    link(
                        "Panel settings",
                        "Position, appearance and contents",
                        Section::Statusbar,
                        nav,
                    ),
                ],
            ),
            card(
                "Dock",
                vec![
                    item("Dock position", "Screen edge", position, || {}),
                    unsupported("Auto-hide dock", "Automatic hiding is not available yet"),
                    item(
                        "Dock size",
                        format!("{:.0} px", c.dockbar.thickness),
                        size,
                        || {},
                    ),
                    link(
                        "Dock settings",
                        "Appearance and contents",
                        Section::Dockbar,
                        nav,
                    ),
                ],
            ),
            card(
                "Extras",
                vec![
                    link(
                        "Widgets",
                        "Clock, weather, launcher and more",
                        Section::Islands,
                        nav,
                    ),
                    detail_link(
                        "Notifications & popups",
                        "Application messages, volume and brightness indicators",
                        Page::Notifications,
                        nav,
                    ),
                    detail_link(
                        "Hot corners",
                        "Actions at screen corners",
                        Page::HotCorners,
                        nav,
                    ),
                    link(
                        "Desktop icons",
                        "Folders, shortcuts and layout",
                        Section::DesktopIcons,
                        nav,
                    ),
                    link(
                        "Status icons",
                        "Network, battery, audio and brightness",
                        Section::Tray,
                        nav,
                    ),
                ],
            ),
        ],
        state,
    )
}
pub mod hot_corners;
