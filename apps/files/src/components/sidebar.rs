use super::*;

pub(crate) fn sidebar(b: &BrowserState, tab: &Tab) -> BoxedWidget {
    let home = b.clone();
    let home_path = files::home();
    let mut places = vec![nav_item(
        b,
        "Home",
        "home",
        tab.location() == &Location::Home,
        move || home.navigate(Location::Home),
    )];
    let mut pinned = Vec::new();
    let pins = b.pins.get();
    if pins.is_empty() {
        pinned.push(view(
            pad(row(0.0), 10.0, 3.0),
            vec![label("Pin your favorite folders", 11.0, true)],
        ));
    }
    for path in pins {
        let state = b.clone();
        let unpin = b.clone();
        let target = path.clone();
        pinned.push(view(
            row(0.0),
            vec![
                nav_item(
                    b,
                    &files::name(&path),
                    "folder",
                    tab.location() == &Location::Directory(path.clone()),
                    move || state.navigate(Location::Directory(path.clone())),
                ),
                icon_button(b, "close", true, false, move || {
                    unpin.toggle_pin(target.clone())
                }),
            ],
        ));
    }
    for place in b.places.iter() {
        // The dedicated Home destination opens Files' overview. Do not repeat
        // the filesystem's home directory immediately beneath it.
        if place.path == home_path || place.label == "Home folder" {
            continue;
        }
        let state = b.clone();
        let path = place.path.clone();
        places.push(nav_item(
            b,
            &place.label,
            place.icon,
            tab.location() == &Location::Directory(path.clone()),
            move || state.navigate(Location::Directory(path.clone())),
        ));
    }
    let mut drives = Vec::new();
    for drive in b.drives.get() {
        let state = b.clone();
        let path = drive.path.clone();
        drives.push(drive_item(
            b,
            &drive,
            tab.location() == &Location::Directory(path.clone()),
            move || state.navigate(Location::Directory(path.clone())),
        ));
    }
    view(
        pad(column(13.0), 12.0, 14.0),
        vec![
            sidebar_section("Places", places),
            sidebar_section("Pinned", pinned),
            sidebar_section("Drives", drives),
        ],
    )
}

fn drive_item(
    b: &BrowserState,
    drive: &files::Drive,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    let (used, detail) = drive
        .space
        .map(|(free, total)| {
            let used = if total == 0 {
                0.0
            } else {
                1.0 - free as f32 / total as f32
            };
            (
                used.clamp(0.0, 1.0),
                format!(
                    "{} used of {}",
                    files::size(total.saturating_sub(free)),
                    files::size(total)
                ),
            )
        })
        .unwrap_or((0.0, drive.path.display().to_string()));
    let color = if used >= 0.9 {
        t.colors.danger
    } else {
        t.colors.accent
    };
    Box::new(button(
        pad(
            Style {
                flex_shrink: 0.0,
                ..column(4.0)
            },
            9.0,
            6.0,
        ),
        active,
        click,
        vec![
            view(
                row(11.0),
                vec![
                    glyph(
                        b,
                        "drive",
                        17.0,
                        if active {
                            t.colors.accent
                        } else {
                            t.colors.text_secondary
                        },
                    ),
                    label(short(&drive.label, 21), 13.0, !active),
                ],
            ),
            view(
                column(3.0),
                vec![
                    Box::new(
                        RawView::new(fixed(166.0, 4.0))
                            .background(t.colors.border)
                            .corner_radius(2.0)
                            .child(Box::new(
                                RawView::new(fixed(166.0 * used, 4.0))
                                    .background(color)
                                    .corner_radius(2.0),
                            )),
                    ),
                    label(detail, 10.0, true),
                ],
            ),
        ],
    ))
}

fn sidebar_section(title: &str, items: Vec<BoxedWidget>) -> BoxedWidget {
    view(
        column(2.0),
        vec![sidebar_heading(title), view(column(1.0), items)],
    )
}

pub(crate) fn nav_item(
    b: &BrowserState,
    title: &str,
    icon: &str,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    Box::new(button(
        pad(
            Style {
                flex_grow: 1.0,
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(36.0),
                },
                flex_shrink: 0.0,
                ..row(11.0)
            },
            9.0,
            0.0,
        ),
        active,
        click,
        vec![
            glyph(
                b,
                icon,
                17.0,
                if active {
                    t.colors.accent
                } else {
                    t.colors.text_secondary
                },
            ),
            Box::new(
                Text::new(short(title, 21))
                    .font_size(13.0)
                    .color(if active {
                        t.colors.accent
                    } else {
                        t.colors.text_secondary
                    }),
            ),
        ],
    ))
}

pub(crate) fn sidebar_heading(text: &str) -> BoxedWidget {
    view(
        pad(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(18.0),
                },
                flex_shrink: 0.0,
                ..row(0.0)
            },
            9.0,
            0.0,
        ),
        vec![Box::new(
            Text::new(text.to_uppercase())
                .font_size(10.0)
                .color(use_theme().colors.text_secondary),
        )],
    )
}
