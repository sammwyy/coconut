use coconut_plugin_kit::chrome::{
    shell_accent, shell_border, shell_card, shell_control, shell_control_hover, shell_muted,
    shell_on_accent, shell_panel, shell_selected, shell_text, ISLAND_RADIUS,
};
use coconut_plugin_kit::{
    app_categories, app_matches, launch_app, open_settings, settings_program, AppCatalog, AppEntry,
};
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::layout::{AlignItems, FlexDirection, JustifyContent, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, StateStyle, Style, Styled};
use creamui_image::{Image, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{fixed, Align, Flex, Justify, Wrap};
use creamui_widgets::RawButton;
use creamui_widgets::{Icon, ScrollController, ScrollView, Symbol, TextController, TextInput};
use std::rc::Rc;

const WIDTH: u32 = 600;
const HEIGHT: u32 = 530;
const TILE: f32 = 132.0;
const TILE_H: f32 = 104.0;

#[derive(Clone)]
pub struct DrawerState {
    query: TextController,
    category: Signal<String>,
    scroll: ScrollController,
}

impl Default for DrawerState {
    fn default() -> Self {
        let query = TextController::default();
        let scroll = ScrollController::default();
        let search_scroll = scroll.clone();
        query.on_change(move |_, next| {
            search_scroll.set(0.0);
            Some(next.to_owned())
        });
        Self {
            query,
            category: Signal::new("All".into()),
            scroll,
        }
    }
}

/// The app drawer panel: a searchable, categorized grid of installed
/// applications. See this module's doc comment for how `AppCatalog` is
/// built and shared, and where `DrawerState` comes from.
pub struct AppDrawerPanel;

impl Panel for AppDrawerPanel {
    fn id(&self) -> &'static str {
        "app_drawer"
    }

    fn title(&self) -> &'static str {
        "Coconut App Drawer"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn autofocus(&self) -> bool {
        true
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        let catalog = ctx.shared.get::<AppCatalog>().unwrap_or_else(|| {
            eprintln!(
                "app_drawer: SharedState is missing AppCatalog (see AppDrawerPlugin::panels); showing an empty drawer"
            );
            AppCatalog::new()
        });
        let state = ctx.scratch.get_or_insert_with(DrawerState::default);
        build_drawer(
            catalog,
            state,
            ctx.close.clone(),
            ctx.shared.get::<coconut_plugin_kit::pins::Pins>(),
        )
    }
}

fn build_drawer(
    catalog: AppCatalog,
    state: DrawerState,
    on_launch: Rc<dyn Fn()>,
    pins: Option<coconut_plugin_kit::pins::Pins>,
) -> BoxedWidget {
    let pin_error = pins
        .as_ref()
        .and_then(|pins| pins.error())
        .map(|message| {
            let action = pins.clone();
            Box::new(
                Flex::row()
                    .gap(8.0)
                    .align(Align::Center)
                    .child(Box::new(creamui_widgets::RawText::new(
                        message,
                        shell_text(),
                        12.0,
                    )))
                    .child(Box::new(
                        RawButton::new(small_button_style(), move || {
                            if let Some(pins) = &action {
                                pins.dismiss_error();
                            }
                        })
                        .child(Box::new(Icon::new(Symbol::Close, shell_muted()).size(14.0))),
                    )),
            ) as BoxedWidget
        })
        .unwrap_or_else(|| {
            Box::new(creamui_widgets::RawView::new(
                Style::new().display(creamui_core::layout::Display::None),
            ))
        });
    let query = state.query.value();
    let selected_category = state.category.get();
    let catalog_apps = catalog.apps();
    let loaded = catalog.loaded();
    let categories = app_categories(&catalog_apps);
    let apps: Vec<_> = catalog_apps
        .into_iter()
        .filter(|app| app_matches(app, &query, &selected_category))
        .collect();
    let content: BoxedWidget = if apps.is_empty() {
        let message = if loaded {
            "No applications found"
        } else {
            "Scanning applications…"
        };
        Box::new(jsx! {
            <Flex grow={1.0} size={(568.0, 304.0)} align={Align::Center} justify={Justify::Center}>
                <RawText color={shell_muted()} font_size={16.0}>{message}</RawText>
            </Flex>
        })
    } else {
        let mut grid = Flex::row()
            .gap(6.0)
            .justify(Justify::Start)
            .wrap(Wrap::Wrap)
            .padding(6.0);
        for app in apps {
            let launch = app.clone();
            let close = on_launch.clone();
            let button = Box::new(
                RawButton::new(app_style(), move || {
                    launch_app(&launch);
                    close();
                })
                .keyboard_focus(false)
                .child(app_card(&app)),
            );
            let mut tile = coconut_plugin_kit::Interactive::new(button);
            if let Some(path) = app.desktop_file.clone() {
                tile = tile.drag_files(vec![path.clone()]);
                if let Some(pins) = pins.clone() {
                    tile = tile.context_menu(move |_| {
                        let action = pins.clone();
                        let target = path.clone();
                        vec![
                            creamui_widgets::ContextMenuItem::new("pin", "Pin", move || {
                                action.pin(vec![target.clone()])
                            })
                            .enabled(pins.loaded() && !pins.busy() && !pins.contains(&path)),
                        ]
                    });
                }
            }
            grid = grid.child(Box::new(tile));
        }
        Box::new(grid)
    };

    let search = Box::new(
        TextInput::controlled_with_style(search_style(), &state.query)
            .placeholder("Search installed apps")
            .background(shell_control())
            .layout(search_style()),
    ) as BoxedWidget;
    let category_tabs = category_tabs(state.clone(), &categories);
    let settings = settings_program()
        .map(|_| settings_button())
        .unwrap_or_else(empty_button);
    let scroll = Box::new(
        ScrollView::controlled(scroll_style(), state.scroll.clone())
            .background(shell_panel())
            .border(shell_border(), 1.0)
            .corner_radius(ISLAND_RADIUS)
            .child(content),
    ) as BoxedWidget;
    let name = user_name();
    let initial = name
        .chars()
        .next()
        .unwrap_or('U')
        .to_uppercase()
        .to_string();
    let account = settings_program()
        .map(|_| account_button())
        .unwrap_or_else(empty_button);
    let app_management = Box::new(
        RawButton::new(small_button_style(), || {})
            .keyboard_focus(false)
            .child(Box::new(Icon::new(Symbol::Grid, shell_muted()).size(16.0)) as BoxedWidget),
    ) as BoxedWidget;
    let footer_actions = footer_actions(on_launch.clone());
    let search_icon = Box::new(Icon::new(Symbol::Search, shell_muted()).size(16.0)) as BoxedWidget;

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)}>
            <Flex direction={FlexDirection::Row} align={Align::Center} padding={10.0} gap={10.0} background={shell_control()} corner_radius={12.0}>
                <Flex size={(34.0, 34.0)} align={Align::Center} justify={Justify::Center} background={shell_accent()} corner_radius={17.0}>
                    <RawText color={shell_on_accent()} font_size={14.0}>{initial}</RawText>
                </Flex>
                <Flex direction={FlexDirection::Column} gap={2.0}>
                    <RawText color={shell_text()} font_size={12.0}>{name}</RawText>
                    <RawText color={shell_muted()} font_size={10.0}>{std::env::consts::OS.to_owned()}</RawText>
                </Flex>
                <Flex grow={1.0} />
                {account}
                {app_management}
                {settings}
            </Flex>
            {category_tabs}
            {pin_error}
            <Flex grow={1.0}>
                {scroll}
            </Flex>
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0} padding={8.0} border={(shell_border(), 1.0)} corner_radius={12.0}>
                {search_icon}
                {search}
                {footer_actions}
            </Flex>
        </Flex>
    })
}

fn app_card(app: &AppEntry) -> BoxedWidget {
    let icon = app
        .icon
        .clone()
        .map(|data| {
            Box::new(
                Image::new(data)
                    .layout(LayoutStyle {
                        size: fixed(48.0, 48.0),
                        ..Default::default()
                    })
                    .fit(ImageFit::Contain),
            ) as BoxedWidget
        })
        .unwrap_or_else(|| {
            Box::new(jsx! {
                <Flex size={(48.0, 48.0)} align={Align::Center} justify={Justify::Center} background={shell_accent()} corner_radius={14.0}>
                    <RawText color={shell_on_accent()} font_size={20.0}>{app.name.chars().next().unwrap_or('•').to_uppercase().to_string()}</RawText>
                </Flex>
            })
        });
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE, TILE_H)} padding={8.0} gap={7.0} align={Align::Center} justify={Justify::Center}>
            {icon}
            <RawText color={shell_text()} font_size={10.0}>{short_name(&app.name)}</RawText>
        </Flex>
    })
}

fn category_tabs(state: DrawerState, categories: &[String]) -> BoxedWidget {
    let selected = state.category.get();
    let mut category_list = Flex::row()
        .gap(4.0)
        .wrap(Wrap::Wrap)
        .justify(Justify::Center)
        .padding(4.0)
        .background(shell_control())
        .corner_radius(12.0);
    for label in categories {
        let category = state.category.clone();
        let scroll = state.scroll.clone();
        let label = label.clone();
        let active = selected == label;
        let action_label = label.clone();
        category_list = category_list.child(Box::new(
            RawButton::new(category_button_style(active), move || {
                category.set(action_label.clone());
                scroll.set(0.0);
            })
            .keyboard_focus(false)
            .child(Box::new(jsx! {
                <RawText color={if active { shell_accent() } else { shell_muted() }} font_size={11.0}>{label}</RawText>
            }) as BoxedWidget),
        ) as BoxedWidget);
    }
    Box::new(category_list)
}

fn short_name(name: &str) -> String {
    const MAX: usize = 14;
    let mut shortened: String = name.chars().take(MAX).collect();
    if name.chars().count() > MAX {
        shortened.push('…');
    }
    shortened
}

fn app_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TILE, TILE_H),
            ..Default::default()
        })
        .corner_radius(12.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn search_style() -> creamui_core::layout::Style {
    LayoutStyle {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Auto,
            height: creamui_core::layout::Dimension::Length(30.0),
        },
        flex_grow: 1.0,
        ..Default::default()
    }
}

fn scroll_style() -> creamui_core::layout::Style {
    LayoutStyle {
        size: fixed(568.0, 318.0),
        ..Default::default()
    }
}

fn category_button_style(active: bool) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Auto,
                height: creamui_core::layout::Dimension::Length(26.0),
            },
            ..Default::default()
        })
        .padding(8.0)
        .background(if active {
            shell_selected()
        } else {
            shell_control()
        })
        .corner_radius(10.0)
        .hover(StateStyle::new().background(if active {
            shell_selected()
        } else {
            shell_control_hover()
        }))
}

fn settings_button_style() -> Style {
    centered_icon_button_style(34.0)
        .background(shell_control())
        .corner_radius(9.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn small_button_style() -> Style {
    centered_icon_button_style(32.0)
        .background(shell_control())
        .corner_radius(8.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

/// Raw buttons do not center a child by default. Keep every compact action
/// button on a shared flex layout so its glyph remains optically centered.
fn centered_icon_button_style(size: f32) -> Style {
    Style::new().layout(LayoutStyle {
        size: fixed(size, size),
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        ..Default::default()
    })
}

fn account_button() -> BoxedWidget {
    let mut style = small_button_style();
    style.layout.size = fixed(64.0, 32.0);
    Box::new(
        RawButton::new(style, || open_settings("/users/me"))
            .child(Box::new(
                jsx! { <RawText color={shell_text()} font_size={11.0}>{"Account"}</RawText> },
            ) as BoxedWidget)
            .keyboard_focus(false),
    )
}

fn empty_button() -> BoxedWidget {
    Box::new(Flex::row().size(32.0, 32.0))
}

fn footer_actions(on_close: Rc<dyn Fn()>) -> BoxedWidget {
    let close = on_close.clone();
    let buttons = [Symbol::Moon, Symbol::Sliders, Symbol::Close];
    let mut actions = Flex::row().gap(2.0);
    for symbol in buttons {
        let handler = close.clone();
        actions = actions.child(Box::new(
            RawButton::new(small_button_style(), move || {
                if matches!(symbol, Symbol::Close) {
                    handler();
                }
            })
            .keyboard_focus(false)
            .child(Box::new(Icon::new(symbol, shell_muted()).size(15.0)) as BoxedWidget),
        ) as BoxedWidget);
    }
    Box::new(actions)
}

fn settings_button() -> BoxedWidget {
    Box::new(
        RawButton::new(settings_button_style(), || open_settings("/connectivity"))
            .keyboard_focus(false)
            .child(Box::new(Icon::new(Symbol::Sliders, shell_muted()).size(18.0)) as BoxedWidget),
    )
}

fn user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "User".into())
}
