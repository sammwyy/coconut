mod common;
mod sections;

use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{platform::WindowRole, AppBuilder, WindowHandle, WindowOptions};
use creamui_widgets::{
    nested_sidebar, ColorPickerController, SidebarNavController, SidebarNode, Surface, SurfaceRole,
    Symbol,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Appearance,
    Theme,
    Wallpaper,
    Taskbar,
    General,
    Tray,
    Widgets,
}

fn tree() -> Vec<SidebarNode<Section>> {
    vec![SidebarNode::parent(
        Section::Appearance,
        Symbol::Appearance,
        "Appearance",
        vec![
            SidebarNode::leaf(Section::Theme, Symbol::Appearance, "Theme"),
            SidebarNode::leaf(Section::Wallpaper, Symbol::Image, "Wallpaper"),
            SidebarNode::group(
                Section::Taskbar,
                "Taskbar",
                vec![
                    SidebarNode::leaf(Section::General, Symbol::Sliders, "Position"),
                    SidebarNode::leaf(Section::Tray, Symbol::Grid, "Status area"),
                    SidebarNode::leaf(Section::Widgets, Symbol::Check, "Widgets"),
                ],
            ),
        ],
    )]
}

pub fn run() {
    let config = Signal::new(ShellConfig::load());
    let active_theme_id = Signal::new(creamui_theme::active_theme_id());
    let view = Signal::new(Section::Theme);
    let nav = SidebarNavController::new();
    let wallpaper_color_picker = ColorPickerController::new();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let initial_theme = creamui_theme::active_theme();

    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                WindowOptions {
                    title: "Settings".into(),
                    width: 800,
                    height: 540,
                    decorations: true,
                    resizable: true,
                    transparent: false,
                    role: WindowRole::Normal,
                    theme: initial_theme,
                    ..Default::default()
                },
                initial_theme.colors.surface,
                {
                    let window = window.clone();
                    move |handle| *window.borrow_mut() = Some(handle)
                },
                move |size| {
                    build(
                        size,
                        &config,
                        &active_theme_id,
                        &view,
                        &nav,
                        &window,
                        &wallpaper_color_picker,
                    )
                },
            );
        })
        .run();
}

fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    active_theme_id: &Signal<String>,
    view: &Signal<Section>,
    nav: &SidebarNavController<Section>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    wallpaper_color_picker: &ColorPickerController,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();

    let sidebar_style = Style {
        flex_direction: FlexDirection::Column,
        size: creamui_core::layout::Size {
            width: Dimension::Length(196.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(14.0),
            right: LengthPercentage::Length(10.0),
            top: LengthPercentage::Length(20.0),
            bottom: LengthPercentage::Length(20.0),
        },
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(2.0),
            height: LengthPercentage::Length(2.0),
        },
        ..Default::default()
    };
    let current = view.get();
    let select = view.clone();
    let sidebar = nested_sidebar(
        sidebar_style,
        &tree(),
        nav,
        Some(&current),
        move |section| {
            select.set(section);
        },
    );

    let content = match current {
        Section::Theme => sections::appearance::build(size, active_theme_id, window),
        Section::Wallpaper => sections::wallpaper::build(size, config, wallpaper_color_picker),
        Section::General => sections::general::build(size, config),
        Section::Tray => sections::tray::build(size, config),
        Section::Widgets => sections::widgets::build(size, config),
        Section::Appearance | Section::Taskbar => {
            unreachable!("sidebar parents are not selectable")
        }
    };

    let panel_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(28.0),
            right: LengthPercentage::Length(28.0),
            top: LengthPercentage::Length(24.0),
            bottom: LengthPercentage::Length(24.0),
        },
        ..Default::default()
    };
    let panel: BoxedWidget = Box::new(Surface::new(SurfaceRole::Panel, panel_style).child(content));
    let panel_outer_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(0.0),
            right: LengthPercentage::Length(20.0),
            top: LengthPercentage::Length(20.0),
            bottom: LengthPercentage::Length(20.0),
        },
        ..Default::default()
    };

    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)} background={theme.surface}>
            {sidebar}
            <RawView style={panel_outer_style}>
                {panel}
            </RawView>
        </Flex>
    })
}
