use crate::routes::destination::Section;
use crate::{components, routes};
use creamui_router::Router;

use crate::icons::SettingsIcons;
use creamui_core::layout::{
    Dimension, FlexDirection, LengthPercentage, LengthPercentageAuto, Position, Style,
};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_widgets::{Icon, IconSource, RawButton, RawView, Text};
use std::cell::RefCell;
use std::rc::Rc;

fn sidebar_item(
    current: &Section,
    section: routes::destination::Section,
    label: &str,
    icon: IconSource,
    color: creamui_theme::Color,
    view: &Router,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let active = crate::routes::destination::parent(current) == section;
    let select = view.clone();
    let item = RawButton::new(
        Style {
            size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(36.0) },
            ..Default::default()
        },
        move || routes::navigate(&select, &section),
    )
    .background(if active { theme.colors.selection_background } else { creamui_theme::Color::rgba(0, 0, 0, 0) })
    .hover_style(creamui_core::StateStyle::new().background(if active { theme.colors.selection_background } else { theme.colors.surface_hover }))
    .corner_radius(12.0)
    .child(Box::new(jsx! {
        <RawView style={Style {
            size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Percent(1.0) },
            align_items: Some(creamui_core::layout::AlignItems::Center),
            gap: creamui_core::layout::Size { width: LengthPercentage::Length(10.0), height: LengthPercentage::Length(0.0) },
            padding: creamui_core::layout::Rect { left: LengthPercentage::Length(8.0), right: LengthPercentage::Length(8.0), top: LengthPercentage::Length(0.0), bottom: LengthPercentage::Length(0.0) },
            ..Default::default()
        }}>
            {components::icon_badge(icon, color)}
            {Box::new(Text::new(label).font_size(13.0)) as BoxedWidget}
        </RawView>
    }));
    Box::new(item)
}

fn sidebar_group(label: &str, items: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={4.0}>
            {Box::new(Text::secondary(label).font_size(10.0).bold(true).padding_left(8.0)) as BoxedWidget}
            {Box::new(RawView::new(Style { flex_direction: FlexDirection::Column, ..Default::default() }).with_children(items)) as BoxedWidget}
        </Flex>
    })
}

pub(crate) fn settings_sidebar(
    current: &Section,
    view: &Router,
    icons: &SettingsIcons,
) -> BoxedWidget {
    use creamui_theme::Color;
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={16.0} padding={12.0}>
            {sidebar_group("NETWORK", vec![
                sidebar_item(current, Section::Connectivity, "Connectivity", icons.status.clone(), Color::rgb(0, 153, 214), view),
                sidebar_item(current, Section::Hardware, "Hardware", icons.devices.clone(), Color::rgb(0, 173, 188), view),
            ])}
            {sidebar_group("LOOK & FEEL", vec![
                sidebar_item(current, Section::Personalization, "Personalization", icons.paintbrush.clone(), Color::rgb(193, 99, 190), view),
                sidebar_item(current, Section::Desktop, "Desktop", icons.wallpaper.clone(), Color::rgb(110, 105, 224), view),
                sidebar_item(current, Section::Windows, "Windows", icons.windows.clone(), Color::rgb(78, 125, 222), view),
            ])}
            {sidebar_group("WORKFLOW", vec![
                sidebar_item(current, Section::ShortcutsCategory, "Shortcuts", icons.shortcuts.clone(), Color::rgb(222, 126, 54), view),
                sidebar_item(current, Section::Applications, "Applications", icons.applications.clone(), Color::rgb(0, 177, 115), view),
            ])}
            {sidebar_group("PEOPLE", vec![
                sidebar_item(current, Section::Users, "Users & Accounts", icons.users.clone(), Color::rgb(213, 88, 91), view),
                sidebar_item(current, Section::Privacy, "Privacy & Security", icons.privacy.clone(), Color::rgb(54, 171, 107), view),
                sidebar_item(current, Section::Accessibility, "Accessibility", icons.accessibility.clone(), Color::rgb(0, 167, 187), view),
            ])}
            {sidebar_group("SYSTEM", vec![
                sidebar_item(current, Section::System, "System", icons.system.clone(), Color::rgb(77, 142, 229), view),
                sidebar_item(current, Section::About, "About", icons.about.clone(), Color::rgb(0, 148, 191), view),
            ])}
        </Flex>
    })
}

pub(crate) fn distribution_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release.lines().find_map(|line| {
                line.strip_prefix("ID=")
                    .map(|id| id.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| std::env::consts::OS.to_owned())
}

pub(crate) fn window_controls(
    window: &Rc<RefCell<Option<WindowHandle>>>,
    icons: &SettingsIcons,
    maximized: &Signal<bool>,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let buttons = [
        icons.minimize.clone(),
        icons.maximize.clone(),
        icons.close.clone(),
    ]
    .into_iter()
    .enumerate()
    .map(|(action, icon)| {
        let window = window.clone();
        let maximized = maximized.clone();
        Box::new(
            RawButton::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(28.0),
                        height: Dimension::Length(28.0),
                    },
                    align_items: Some(creamui_core::layout::AlignItems::Center),
                    justify_content: Some(creamui_core::layout::JustifyContent::Center),
                    ..Default::default()
                },
                move || {
                    if let Some(handle) = window.borrow().as_ref() {
                        match action {
                            0 => handle.minimize(),
                            1 => {
                                let next = !maximized.peek();
                                handle.set_maximized(next);
                                maximized.set(next);
                            }
                            _ => handle.close(),
                        }
                    }
                },
            )
            .background(theme.colors.text_secondary.mix(theme.colors.surface, 0.88))
            .hover_style(
                creamui_core::StateStyle::new()
                    .background(theme.colors.text_secondary.mix(theme.colors.surface, 0.75)),
            )
            .corner_radius(14.0)
            .child(Box::new(
                Icon::new(icon, theme.colors.text_primary).size(16.0),
            )),
        ) as BoxedWidget
    })
    .collect();
    Box::new(
        RawView::new(Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                right: LengthPercentageAuto::Length(16.0),
                top: LengthPercentageAuto::Length(14.0),
                left: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            gap: creamui_core::layout::Size {
                width: LengthPercentage::Length(6.0),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        })
        .with_children(buttons),
    )
}

pub(crate) fn detail_header(
    current: &Section,
    nav: &creamui_router::Router,
    width: f32,
) -> BoxedWidget {
    let parent = crate::routes::destination::parent(current);
    let title = crate::routes::destination::page_title(current);
    let back = nav.clone();
    let decorations = creamui_render::use_window_decorations();
    let controls = decorations.controls;
    let right = if decorations.mode == creamui_render::WindowDecorationMode::Client {
        128.0
    } else if controls.width > 0 && controls.x as f32 > width / 2.0 {
        (width - controls.x as f32 + 16.0).max(36.0)
    } else {
        36.0
    };
    let left = if controls.width > 0 && (controls.x as f32) < width / 2.0 {
        ((controls.x + controls.width) as f32 - 256.0 + 16.0).max(36.0)
    } else {
        36.0
    };
    Box::new(
        jsx! { <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} height={56.0} shrink={0.0} padding_left={left} padding_right={right}>
            {crate::components::back_button(move || crate::routes::navigate(&back, &parent))}
            {Box::new(Text::new(title).font_size(14.0).bold(true)) as BoxedWidget}
        </Flex> },
    )
}
