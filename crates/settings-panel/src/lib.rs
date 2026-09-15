mod common;
mod sections;

use coconut_core::ShellConfig;
use creamui_core::layout::{AlignItems, Dimension, FlexDirection, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{platform::WindowRole, AppBuilder, WindowHandle, WindowOptions};
use creamui_widgets::{nested_sidebar, SidebarNavController, SidebarNode, TabColors};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Desktop,
    Appearance,
    Wallpaper,
    Shell,
    General,
    Tray,
    Widgets,
}

fn tree() -> Vec<SidebarNode<Section>> {
    vec![
        SidebarNode::parent(
            Section::Desktop,
            "Desktop",
            vec![
                SidebarNode::leaf(Section::Appearance, "Appearance"),
                SidebarNode::leaf(Section::Wallpaper, "Wallpaper"),
            ],
        ),
        SidebarNode::parent(
            Section::Shell,
            "Shell",
            vec![
                SidebarNode::leaf(Section::General, "General"),
                SidebarNode::leaf(Section::Tray, "Tray"),
                SidebarNode::leaf(Section::Widgets, "Widgets"),
            ],
        ),
    ]
}

pub fn run() {
    let config = Signal::new(ShellConfig::load());
    let active_theme_id = Signal::new(creamui_theme::active_theme_id());
    let view = Signal::new(Section::Appearance);
    let nav = SidebarNavController::<Section>::new();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));

    let initial_theme = creamui_theme::active_theme();

    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                WindowOptions {
                    title: "Coconut Settings".into(),
                    width: 760,
                    height: 520,
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
                move |size| build(size, &config, &active_theme_id, &view, &nav, &window),
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
) -> BoxedWidget {
    let sidebar_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(200.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(12.0),
            right: LengthPercentage::Length(12.0),
            top: LengthPercentage::Length(16.0),
            bottom: LengthPercentage::Length(16.0),
        },
        ..Default::default()
    };
    let item_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(36.0),
        },
        align_items: Some(AlignItems::Center),
        ..Default::default()
    };
    let current = view.get();
    let select = view.clone();
    let sidebar = nested_sidebar(
        TabColors::sidebar(),
        sidebar_style,
        item_style,
        &tree(),
        nav,
        Some(&current),
        move |section| select.set(section),
    );

    let content = match current {
        Section::Appearance => sections::appearance::build(size, active_theme_id, window),
        Section::Wallpaper => sections::wallpaper::build(size, config),
        Section::General => sections::general::build(size, config),
        Section::Tray => sections::tray::build(size, config),
        Section::Widgets => sections::widgets::build(size, config),
        Section::Desktop | Section::Shell => sections::appearance::build(size, active_theme_id, window),
    };

    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)}>
            {sidebar}
            {content}
        </Flex>
    })
}
