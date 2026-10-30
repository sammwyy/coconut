use self::{
    directory::{columns_view, directory_header, directory_view},
    home::home_view,
};
use crate::components::*;
use crate::components::{navigation::*, portal_footer::portal_footer, sidebar::sidebar};
use crate::config::TabPosition;
use crate::navigation::DirectoryView;
pub(crate) mod directory;
mod home;

pub(crate) fn build(b: &BrowserState, size: Size) -> BoxedWidget {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(b.context.clone());
        build_scoped(b, size)
    })
}

fn build_scoped(b: &BrowserState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let tab = b.tab();
    let sidebar_width = if size.width < 850.0 { 192.0 } else { 236.0 };
    let content_width = (size.width - sidebar_width).max(180.0);
    let tabs_at_top = b.explorer.tab_position == TabPosition::Top;
    let has_tab_strip = b.tabs.get().len() > 1;
    let content = if tab.location() == &Location::Home {
        home_view(b, &tab, content_width)
    } else if tab.view == DirectoryView::Columns {
        columns_view(b, &tab, content_width)
    } else {
        directory_view(b, &tab, content_width)
    };
    let scroll: BoxedWidget =
        if tab.view == DirectoryView::Columns && tab.location() != &Location::Home {
            content
        } else {
            Box::new(
                RawScrollView::controlled(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..column(0.0)
                    },
                    tab.scroll.clone(),
                )
                .child(content),
            )
        };
    let mut main = Vec::new();
    if tabs_at_top {
        if let Some(tabs) = tab_bar(b, content_width, sidebar_width, true) {
            main.push(tabs);
        }
    }
    main.push(toolbar(
        b,
        &tab,
        sidebar_width,
        content_width,
        !(tabs_at_top && has_tab_strip),
    ));
    if let Some(filters) = portal_footer::filter_bar(b) {
        main.push(filters);
    }
    if tab.location() != &Location::Home && tab.view == DirectoryView::List {
        // This is deliberately outside RawScrollView: column labels remain
        // available while a long directory is being scrolled.
        main.push(directory_header(b, &tab, content_width));
    }
    let context = b.clone();
    let location = tab.location().clone();
    main.push(Box::new(
        RawView::new(Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            ..column(0.0)
        })
        .with_context_menu(move |point| {
            let path = match &location {
                Location::Home => files::home(),
                Location::Directory(path) => path.clone(),
            };
            context.open_context_menu(point, coconut_file_menu::FileMenuTarget::Background(path));
        })
        .child(scroll),
    ));
    if let Some(message) = b.message.get() {
        main.push(view(
            pad(row(8.0), 24.0, 8.0),
            vec![label(message, 12.0, true)],
        ));
    }
    if use_is_portal().is_some() {
        main.push(portal_footer(b, &tab));
    }
    if tabs_at_top {
        if b.explorer.show_statusbar {
            main.push(statusbar(b, &tab, false));
        }
    } else if b.explorer.show_statusbar {
        let strip_width = (content_width - COMPACT_STATUS_WIDTH).max(0.0);
        let mut bottom = Vec::new();
        if let Some(tabs) = tab_bar(b, strip_width, sidebar_width, false) {
            bottom.push(tabs);
        } else {
            bottom.push(view(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    ..row(0.0)
                },
                vec![],
            ));
        }
        bottom.push(statusbar(b, &tab, true));
        main.push(view(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(CHROME_HEIGHT),
                },
                flex_shrink: 0.0,
                ..row(0.0)
            },
            bottom,
        ));
    } else if let Some(tabs) = tab_bar(b, content_width, sidebar_width, false) {
        main.push(tabs);
    }
    let content = view(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(size.width),
                height: Dimension::Length(size.height),
            },
            align_items: Some(AlignItems::Stretch),
            ..row(0.0)
        },
        vec![
            Box::new(
                CUIWindowDragArea::new()
                    .layout(Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(sidebar_width),
                            height: Dimension::Percent(1.0),
                        },
                        flex_shrink: 0.0,
                        ..column(0.0)
                    })
                    .background(t.colors.surface.mix(t.colors.accent, 0.035))
                    .with_children(vec![Box::new(
                        RawScrollView::controlled(
                            Style {
                                flex_grow: 1.0,
                                ..column(0.0)
                            },
                            b.sidebar_scroll.clone(),
                        )
                        .scrollbar(false)
                        .child(sidebar(b, &tab)),
                    )]),
            ),
            Box::new(
                RawView::new(Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    ..column(0.0)
                })
                .background(t.colors.surface)
                .with_children(main),
            ),
        ],
    );
    let mut layers = vec![content];
    layers.extend(b.file_menu.overlays(size));
    view(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(size.width),
                height: Dimension::Length(size.height),
            },
            ..Default::default()
        },
        layers,
    )
}
