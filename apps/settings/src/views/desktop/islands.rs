use crate::{
    components::{group, labeled_group, section, setting_row},
    routes,
    views::context::ViewContext,
};
use coconut_plugin_app_launcher::{AppLauncherIsland, OpenWindowsIsland};
use coconut_plugin_kit::{
    bar_editor::{BarItem, BAR_ITEMS},
    Island,
};
use creamui_core::{BoxedWidget, Styled};
use creamui_widgets::{
    layout::{Align, Flex, Wrap},
    Button, ButtonSize, ButtonState, ButtonVariant, Icon, SegmentedControl, Symbol, Text,
    TextInput, TextSize,
};

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<ViewContext>();
    if let Some(id) = creamui_router::use_params().get("widget_id") {
        return detail(&context, id);
    }
    catalog(&context)
}

pub(crate) fn control_center_route() -> BoxedWidget {
    let context = creamui_reactive::use_context::<ViewContext>();
    detail(&context, "control_center")
}

pub(crate) fn matches(item: &BarItem, query: &str) -> bool {
    let text = format!(
        "{} {} {} {} {}",
        item.id,
        item.label,
        item.description,
        item.category,
        match item.id {
            "tray.volume" => "sound audio tray",
            "tray.wifi" => "network internet tray",
            "tray.battery" => "power energy tray",
            "control_center" => "tray status icons quick settings",
            "clock" => "date time calendar",
            "clipboard" => "copy paste history",
            _ => "island panel widget",
        }
    )
    .to_lowercase();
    query.split_whitespace().all(|word| text.contains(word))
}

fn catalog(context: &ViewContext) -> BoxedWidget {
    let search = context.island_settings.search.clone();
    let scroll = context.content_scroll.clone();
    search.on_change(move |_, next| {
        scroll.set(0.0);
        Some(next.to_owned())
    });
    let query = search.value().to_lowercase();
    let mut body: Vec<BoxedWidget> = vec![Box::new(
        TextInput::controlled(&search)
            .placeholder("Find widgets, panels or controls…")
            .width(creamui_core::LengthValue::Percent(100.0)),
    )];
    let mut count = 0;
    for category in [
        "Applications",
        "Information",
        "Productivity",
        "System controls",
    ] {
        let rows = BAR_ITEMS
            .iter()
            .filter(|item| item.category == category && matches(item, &query))
            .map(|item| {
                count += 1;
                widget_link(context, item)
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            body.push(labeled_group(category, group(rows)));
        }
    }
    if count == 0 {
        body.push(group(vec![Box::new(
            Flex::column()
                .gap(6.0)
                .padding(20.0)
                .child(Box::new(Text::new("No matching widgets").bold(true)))
                .child(Box::new(
                    Text::secondary("Try a name or a task, such as clock, sound or clipboard.")
                        .size(TextSize::Sm),
                )),
        )]));
    }
    section("Widgets", "Every widget and its panel, in one place. Choose an item to configure its behavior and find it on your bars.", body)
}

fn widget_link(context: &ViewContext, item: &'static BarItem) -> BoxedWidget {
    let router = context.view.clone();
    let config = context.config.get();
    let bars = [("Statusbar", &config.statusbar), ("Dock", &config.dockbar)]
        .into_iter()
        .filter_map(|(label, bar)| {
            bar.sections
                .iter()
                .any(|group| group.islands.iter().any(|entry| entry.id == item.id))
                .then_some(label)
        })
        .collect::<Vec<_>>();
    let location = if bars.is_empty() {
        "Not added to a bar".to_owned()
    } else {
        bars.join(" · ")
    };
    setting_row::icon_item(
        item.label,
        format!("{} · {location}", item.description),
        Some(crate::components::icon_badge(
            crate::components::setting_icon(item.label),
            crate::components::category_color(),
        )),
        Box::new(Icon::new(
            Symbol::ChevronRight,
            creamui_theme::use_theme().colors.text_secondary,
        )),
        move || routes::open(&router, item.settings_route),
    )
}

fn detail(context: &ViewContext, id: &str) -> BoxedWidget {
    let Some(item) = BAR_ITEMS.iter().find(|item| item.id == id) else {
        return crate::views::not_found::route_view("Widget not found");
    };
    let mut body = Vec::new();
    match id {
        "app_launcher" | "open_windows" => {
            let schema = if id == "app_launcher" { AppLauncherIsland.config_schema() } else { OpenWindowsIsland.config_schema() };
            body.push(super::island_settings::build(context.size, item.id, "Widget options", &schema, &context.island_settings));
        }
        "clock" => body.push(clock(context)),
        "clipboard" => {
            body.push(crate::views::privacy::clipboard::history_settings(&context.native));
            let status = context.native.status.get();
            if !status.is_empty() { body.push(Box::new(Text::secondary(status).size(TextSize::Sm))); }
            body.push(group(vec![page_link(context, "Application access", "Review which applications can read or write your clipboard", "/privacy/clipboard")]));
        }
        "control_center" | "tray.wifi" | "tray.bluetooth" | "tray.battery" | "tray.volume" | "tray.brightness" => body.push(super::tray::widget_options(context, id)),
        "workspaces" => body.push(group(vec![page_link(context, "Workspace behavior", "Names, switching and compositor settings", "/windows/workspaces")])),
        "weather" => body.push(group(vec![page_link(context, "Forecast location", "Uses the city and country in your profile", "/users/me")])),
        "current_playing" => body.push(group(vec![Box::new(Flex::column().padding(16.0).gap(6.0)
            .child(Box::new(Text::new("Appears while media is playing")))
            .child(Box::new(Text::secondary("The widget follows your active media player and hides when playback is unavailable.").size(TextSize::Sm))))])),
        _ => {}
    }
    body.push(placements(context, item.id));
    if !context.island_settings.status.get().is_empty()
        && matches!(
            id,
            "clock"
                | "control_center"
                | "tray.wifi"
                | "tray.bluetooth"
                | "tray.battery"
                | "tray.volume"
                | "tray.brightness"
        )
    {
        body.push(Box::new(
            Text::secondary(context.island_settings.status.get()).size(TextSize::Sm),
        ));
    }
    section("", item.description, body)
}

pub(crate) fn page_link(
    context: &ViewContext,
    label: &str,
    hint: &str,
    route: &'static str,
) -> BoxedWidget {
    let router = context.view.clone();
    setting_row::item(
        label,
        hint,
        Box::new(Icon::new(
            Symbol::ChevronRight,
            creamui_theme::use_theme().colors.text_secondary,
        )),
        move || routes::open(&router, route),
    )
}

fn placements(context: &ViewContext, id: &str) -> BoxedWidget {
    let config = context.config.get();
    let rows = [
        (coconut_core::BarKind::Statusbar, &config.statusbar),
        (coconut_core::BarKind::Dockbar, &config.dockbar),
    ]
    .into_iter()
    .map(|(kind, bar)| {
        let groups = bar
            .sections
            .iter()
            .enumerate()
            .filter(|(_, group)| group.islands.iter().any(|entry| entry.id == id))
            .map(|(index, group)| {
                format!(
                    "Group {}{}",
                    index + 1,
                    if group.enabled && bar.enabled {
                        ""
                    } else {
                        " (hidden)"
                    }
                )
            })
            .collect::<Vec<_>>();
        let hint = if groups.is_empty() {
            "Not added · add and arrange items in the bar editor".to_owned()
        } else {
            groups.join(" · ")
        };
        let router = context.view.clone();
        setting_row::item(
            kind.title(),
            hint,
            Box::new(Text::secondary("Arrange…").size(TextSize::Sm)),
            move || routes::open(&router, kind.route()),
        )
    })
    .collect();
    labeled_group("Placement", group(rows))
}

fn clock(context: &ViewContext) -> BoxedWidget {
    let state = &context.island_settings;
    let formats = ["%I:%M %p", "%H:%M", "%H:%M:%S"];
    let selected = formats
        .iter()
        .position(|format| *format == state.clock.value())
        .unwrap_or(3);
    let draft = state.clock.clone();
    let presets = SegmentedControl::new(selected, move |index| {
        if let Some(format) = formats.get(index) {
            draft.set_value(*format);
        }
    })
    .option("12-hour")
    .option("24-hour")
    .option("Seconds");
    let write = state.clone();
    let window = context.window.clone();
    let controls = Flex::column()
        .gap(12.0)
        .padding(16.0)
        .child(Box::new(Text::new("Time format").bold(true)))
        .child(Box::new(presets))
        .child(Box::new(
            Flex::row()
                .wrap(Wrap::Wrap)
                .align(Align::Center)
                .gap(8.0)
                .child(Box::new(TextInput::controlled(&state.clock).width(180.0)))
                .child(Box::new(
                    Button::styled(
                        ButtonVariant::Primary,
                        ButtonSize::Sm,
                        "Apply",
                        ButtonState::Normal,
                        move || {
                            if let Some(window) = window.borrow().as_ref() {
                                write.save_clock(window);
                            }
                        },
                    )
                    .disabled(state.busy.get() || state.clock.value() == state.saved_clock.get()),
                )),
        ))
        .child(Box::new(
            Text::secondary(
                "A custom format uses tokens such as %H:%M. Changes apply in a new session.",
            )
            .size(TextSize::Sm),
        ));
    group(vec![Box::new(controls)])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_matches_tasks_and_every_item_has_a_unique_detail_route() {
        let mut routes = std::collections::HashSet::new();
        for item in BAR_ITEMS {
            assert!(routes.insert(item.settings_route));
            assert!(matches(item, &item.label.to_lowercase()));
        }
        assert!(matches(
            BAR_ITEMS
                .iter()
                .find(|item| item.id == "tray.volume")
                .unwrap(),
            "sound"
        ));
        assert!(matches(
            BAR_ITEMS
                .iter()
                .find(|item| item.id == "control_center")
                .unwrap(),
            "tray"
        ));
        assert!(!matches(&BAR_ITEMS[0], "no widget matches this"));
    }
}
