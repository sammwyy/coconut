use crate::routes::destination::{page_description, page_title, section_presentation, Section};
use crate::{components, routes};
use creamui_router::RouterOutlet;

use creamui_core::layout::{
    Dimension, FlexDirection, LengthPercentage, LengthPercentageAuto, Position, Style,
};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_render::WindowDecorationMode;
use creamui_widgets::{
    Avatar, CUIWindowDragArea, Heading, Icon, IconSource, RawView, Text, TextInput, TextSize,
};

use crate::components::navigation::{distribution_name, settings_sidebar, window_controls};

pub(crate) fn build(context: &crate::views::context::ViewContext) -> BoxedWidget {
    let size = context.size;
    let view = &context.view;
    let window = &context.window;
    let users = &context.users;
    let icons = &context.icons;
    let content_scroll = &context.content_scroll;
    let sidebar_scroll = &context.sidebar_scroll;
    let settings_search = &context.settings_search;
    let connectivity = &context.connectivity;
    let maximized = &context.maximized;

    let theme = creamui_theme::use_theme();

    let current = routes::current(view);
    let connectivity_detail = current == Section::Connectivity && connectivity.showing_detail();
    let native_detail = crate::routes::destination::parent(&current) != current;
    let decorations = creamui_render::use_window_decorations();
    let show_page_header = !connectivity_detail && !native_detail;
    let account_list = users.get();
    let title = page_title(&current);
    let description = page_description(&current);
    let (category_icon, category_color) = section_presentation(&current, icons);
    creamui_reactive::provide_context(components::PageStyle {
        color: category_color,
        title: title.clone(),
    });
    let sidebar = settings_sidebar(&current, view, icons);
    let content_has_own_scroll = matches!(current, Section::Statusbar | Section::Dockbar);
    creamui_reactive::provide_context(context.clone());
    let content = RouterOutlet::render();

    let sidebar_shell_style = Style {
        flex_direction: FlexDirection::Column,
        size: creamui_core::layout::Size {
            width: Dimension::Length(256.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        border: creamui_core::layout::Rect {
            right: LengthPercentage::Length(1.0),
            left: LengthPercentage::Length(0.0),
            top: LengthPercentage::Length(0.0),
            bottom: LengthPercentage::Length(0.0),
        },
        ..Default::default()
    };
    // The concept does not have a separate application toolbar: the page
    // heading belongs to the scrollable document, with a generous inset and
    // no divider beneath it.  Keeping it in the document also prevents large
    // headings from being squeezed into a fixed-height strip.
    let page_header_style = Style {
        flex_direction: FlexDirection::Row,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(0.0),
            right: LengthPercentage::Length(0.0),
            top: LengthPercentage::Length(48.0),
            bottom: LengthPercentage::Length(0.0),
        },
        ..Default::default()
    };
    let content_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_shrink: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    };
    let page_header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={page_header_style} gap={16.0} align={creamui_widgets::layout::Align::Center}>
            <Flex size={(56.0, 56.0)} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Center} background={creamui_core::LinearGradient::new(180.0, category_color.mix(creamui_theme::Color::rgb(255, 255, 255), 0.22), category_color)} corner_radius={16.0}>
                {Box::new(Icon::new(category_icon, creamui_theme::Color::rgb(255, 255, 255)).size(28.0)) as BoxedWidget}
            </Flex>
            <Flex direction={FlexDirection::Column} gap={4.0} justify={creamui_widgets::layout::Justify::Center}>
                {Box::new(Heading::lg(title).font_size(24.0)) as BoxedWidget}
                {Box::new(Text::secondary(description).font_size(14.0)) as BoxedWidget}
            </Flex>
        </Flex>
    });
    let panel_content: BoxedWidget = if content_has_own_scroll {
        let header = if native_detail {
            crate::components::navigation::detail_header(&current, view, size.width)
        } else {
            page_header
        };
        Box::new(jsx! {
            <Flex direction={FlexDirection::Column} grow={1.0} gap={if native_detail { 0.0 } else { 32.0 }}>
                {header}
                {content}
            </Flex>
        })
    } else {
        let scroll: BoxedWidget = Box::new(
            creamui_widgets::RawScrollView::controlled(content_style, content_scroll.clone())
                .scrollbar_width(2.0)
                .scrollbar_margin(0.0)
                .scrollbar_color(creamui_theme::Color::rgba(
                    theme.colors.text_secondary.r,
                    theme.colors.text_secondary.g,
                    theme.colors.text_secondary.b,
                    38,
                ))
                .child(Box::new(
                    RawView::new(Style {
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        // Let the form use the available width at the normal
                        // window size; retain a readable cap on large screens.
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        max_size: creamui_core::layout::Size {
                            width: Dimension::Length(864.0),
                            height: Dimension::Auto,
                        },
                        align_self: Some(creamui_core::layout::AlignSelf::Center),
                        gap: creamui_core::layout::Size {
                            width: LengthPercentage::Length(0.0),
                            height: LengthPercentage::Length(32.0),
                        },
                        padding: creamui_core::layout::Rect {
                            left: LengthPercentage::Length(if size.width < 1000.0 {
                                24.0
                            } else {
                                40.0
                            }),
                            right: LengthPercentage::Length(if size.width < 1000.0 {
                                24.0
                            } else {
                                40.0
                            }),
                            top: LengthPercentage::Length(if show_page_header {
                                0.0
                            } else if connectivity_detail
                                && matches!(
                                    connectivity.current(),
                                    crate::views::connectivity::View::KnownNetworks
                                        | crate::views::connectivity::View::NearbyNetworks
                                )
                            {
                                16.0
                            } else if connectivity_detail || native_detail {
                                28.0
                            } else {
                                11.0
                            }),
                            bottom: LengthPercentage::Length(64.0),
                        },
                        ..Default::default()
                    })
                    .with_children(if show_page_header {
                        vec![page_header, content]
                    } else {
                        vec![content]
                    }),
                )),
        );
        if connectivity_detail || native_detail {
            let header = if connectivity_detail {
                crate::views::connectivity::detail_header(connectivity, size.width)
            } else {
                crate::components::navigation::detail_header(&current, view, size.width)
            };
            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} grow={1.0} gap={0.0}>
                    {header}
                    {scroll}
                </Flex>
            })
        } else {
            scroll
        }
    };
    let main_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    let profile_name = std::env::var("USER").unwrap_or_else(|_| "User".into());
    let account = account_list
        .iter()
        .find(|account| account.username == profile_name);
    let display_name = account
        .map(|account| account.real_name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&profile_name);
    let profile_initial = display_name
        .chars()
        .next()
        .unwrap_or('U')
        .to_uppercase()
        .to_string();
    let mut avatar = Avatar::new(40.0)
        .fallback_color(theme.colors.accent)
        .initials(profile_initial)
        .initials_color(theme.colors.selection_text);
    if let Some(IconSource::Image(image)) = account.map(crate::components::account::account_icon) {
        avatar = avatar.image(Box::new(
            Icon::new(IconSource::Image(image), theme.colors.text_primary).size(40.0),
        ));
    }
    let search: BoxedWidget = Box::new(
        TextInput::controlled_with_style(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(32.0),
                },
                padding: creamui_core::layout::Rect {
                    left: LengthPercentage::Length(32.0),
                    right: LengthPercentage::Length(12.0),
                    top: LengthPercentage::Length(0.0),
                    bottom: LengthPercentage::Length(0.0),
                },
                ..Default::default()
            },
            settings_search,
        )
        .placeholder("Search settings")
        .font_size(12.0)
        .background(
            theme
                .colors
                .surface_elevated
                .mix(theme.colors.surface, 0.45),
        ),
    );
    let sidebar_header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={16.0} style={Style { flex_direction: FlexDirection::Column, flex_shrink: 0.0, padding: creamui_core::layout::Rect { left: LengthPercentage::Length(16.0), right: LengthPercentage::Length(16.0), top: LengthPercentage::Length(20.0), bottom: LengthPercentage::Length(12.0) }, ..Default::default() }}>
            <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} padding_xy={(4.0, 0.0)}>
                {Box::new(avatar) as BoxedWidget}
                <Flex direction={FlexDirection::Column} gap={2.0}>
                    {Box::new(Text::new(display_name).font_size(13.0).bold(true)) as BoxedWidget}
                    {Box::new(Text::secondary(distribution_name()).size(TextSize::Xs)) as BoxedWidget}
                </Flex>
            </Flex>
            <RawView style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(32.0) }, ..Default::default() }}>
                {search}
                {Box::new(RawView::new(Style { position: Position::Absolute, inset: creamui_core::layout::Rect { left: LengthPercentageAuto::Length(10.0), top: LengthPercentageAuto::Length(8.0), right: LengthPercentageAuto::Auto, bottom: LengthPercentageAuto::Auto }, ..Default::default() }).child(Box::new(Icon::new(icons.search.clone(), theme.colors.text_secondary).size(16.0)))) as BoxedWidget}
            </RawView>
        </Flex>
    });
    // Declare background regions, not invisible buttons. CreamUI gives the
    // compact header's back button and any future controls input priority.
    let drag_area: BoxedWidget = Box::new(CUIWindowDragArea::new().layout(Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(256.0),
            right: LengthPercentageAuto::Length(0.0),
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(if connectivity_detail || native_detail {
                56.0
            } else {
                48.0
            }),
        },
        ..Default::default()
    }));
    // Only the empty strip above the account, not the avatar/search/sidebar.
    let sidebar_drag_area: BoxedWidget = Box::new(CUIWindowDragArea::new().layout(Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Auto,
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Length(256.0),
            height: Dimension::Length(16.0),
        },
        ..Default::default()
    }));
    let controls = if decorations.mode == WindowDecorationMode::Client {
        window_controls(window, icons, maximized)
    } else {
        Box::new(RawView::new(Style::default())) as BoxedWidget
    };

    let window_surface = theme.colors.surface;
    let sidebar_surface = window_surface.mix(theme.colors.accent, 0.035);
    let page_surface = creamui_core::LinearGradient::new(
        150.0,
        window_surface,
        window_surface.mix(theme.colors.accent, 0.055),
    );
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)} background={window_surface}>
            <RawView style={sidebar_shell_style} background={sidebar_surface}>
                {sidebar_header}
                {Box::new(creamui_widgets::RawScrollView::controlled(Style { flex_grow: 1.0, min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(0.0) }, ..Default::default() }, sidebar_scroll.clone()).scrollbar(false).child(sidebar)) as BoxedWidget}
                {Box::new(RawView::new(Style { position: Position::Absolute, inset: creamui_core::layout::Rect { left: LengthPercentageAuto::Auto, right: LengthPercentageAuto::Length(0.0), top: LengthPercentageAuto::Length(0.0), bottom: LengthPercentageAuto::Length(0.0) }, size: creamui_core::layout::Size { width: Dimension::Length(1.0), height: Dimension::Percent(1.0) }, ..Default::default() }).background(theme.colors.border)) as BoxedWidget}
            </RawView>
            <RawView style={main_style} background={page_surface}>
                {panel_content}
            </RawView>
            {drag_area}
            {sidebar_drag_area}
            {controls}
        </Flex>
    })
}
