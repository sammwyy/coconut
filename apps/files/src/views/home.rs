use crate::components::*;

pub(crate) fn home_view(b: &BrowserState, tab: &Tab, width: f32) -> BoxedWidget {
    let t = use_theme();
    let query = tab.search.value().to_lowercase();
    let matches = |path: &Path| files::name(path).to_lowercase().contains(&query);
    let mut cards = Vec::new();
    for path in b.pins.get().into_iter().filter(|p| matches(p)) {
        let state = b.clone();
        let name = files::name(&path);
        cards.push(Box::new(
            button(
                pad(
                    Style {
                        align_items: Some(AlignItems::Center),
                        ..column(8.0)
                    },
                    12.0,
                    8.0,
                ),
                false,
                move || state.navigate(Location::Directory(path.clone())),
                vec![
                    glyph(b, "folder-large", 76.0, t.colors.accent),
                    label(short(&name, 16), 13.0, false),
                ],
            )
            .width(116.0),
        ) as BoxedWidget);
    }
    if cards.is_empty() {
        cards.push(label(
            if query.is_empty() {
                "Your pinned folders will appear here."
            } else {
                "No matching pinned folders."
            },
            13.0,
            true,
        ));
    }
    let mut drives = Vec::new();
    for drive in b
        .drives
        .get()
        .into_iter()
        .filter(|d| d.label.to_lowercase().contains(&query))
    {
        let state = b.clone();
        let path = drive.path.clone();
        let card_width = ((width - 72.0) / 2.0).clamp(210.0, 286.0);
        let mut details = vec![
            label(&drive.label, 13.0, false),
            label(
                drive
                    .space
                    .map(|(free, total)| {
                        format!(
                            "{} used of {} ({}%)",
                            files::size(total.saturating_sub(free)),
                            files::size(total),
                            (100.0 * (1.0 - free as f32 / total.max(1) as f32)).round()
                        )
                    })
                    .unwrap_or_else(|| drive.path.display().to_string()),
                11.0,
                true,
            ),
        ];
        if let Some((free, total)) = drive.space {
            let used = if total == 0 {
                0.0
            } else {
                1.0 - free as f32 / total as f32
            };
            details.push(Box::new(
                RawView::new(fixed(card_width - 94.0, 4.0))
                    .background(t.colors.border)
                    .corner_radius(2.0)
                    .child(Box::new(
                        RawView::new(fixed((card_width - 94.0) * used.clamp(0.0, 1.0), 4.0))
                            .background(if used >= 0.9 {
                                t.colors.danger
                            } else {
                                t.colors.accent
                            })
                            .corner_radius(2.0),
                    )),
            ));
        }
        drives.push(Box::new(
            button(
                pad(
                    Style {
                        gap: creamui_core::layout::Size {
                            width: LengthPercentage::Length(16.0),
                            height: LengthPercentage::Length(0.0),
                        },
                        ..fixed(card_width, 94.0)
                    },
                    16.0,
                    12.0,
                ),
                false,
                move || state.navigate(Location::Directory(path.clone())),
                vec![
                    glyph(b, "drive", 40.0, t.colors.accent),
                    view(column(9.0), details),
                ],
            )
            .background(t.colors.surface.mix(t.colors.text_secondary, 0.055)),
        ) as BoxedWidget);
    }
    let mut recent_rows = Vec::new();
    for path in b
        .recent
        .get()
        .into_iter()
        .filter(|p| matches(p) && b.context.accepts(p, false))
    {
        let filename = files::name(&path);
        let parent = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let active = tab.selected.as_ref() == Some(&path);
        recent_rows.push(Box::new(entry_button(
            b,
            pad(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(38.0),
                    },
                    ..row(14.0)
                },
                6.0,
                0.0,
            ),
            active,
            files::Entry {
                path: path.clone(),
                directory: false,
                bytes: 0,
                modified: None,
                created: None,
                permissions: 0,
            },
            vec![
                glyph(b, file_icon(Path::new(&filename)), 20.0, t.colors.accent),
                view(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        ..row(0.0)
                    },
                    vec![label(short(&filename, 36), 12.0, false)],
                ),
                label(
                    short(&parent, ((width - 340.0) / 7.0).max(10.0) as usize),
                    11.0,
                    true,
                ),
            ],
        )) as BoxedWidget);
    }
    if recent_rows.is_empty() {
        recent_rows.push(view(
            pad(column(8.0), 0.0, 10.0),
            vec![
                label(
                    if query.is_empty() {
                        "A little space for your recent work"
                    } else {
                        "No matching recent files"
                    },
                    14.0,
                    false,
                ),
                label("Files you open with File Explorer appear here.", 12.0, true),
            ],
        ));
    }
    let mut sections = vec![
        view(
            column(14.0),
            vec![heading(b, "Pinned", "pin"), wrap(cards, 8.0)],
        ),
        view(
            column(16.0),
            vec![heading(b, "Drives", "drive"), wrap(drives, 12.0)],
        ),
        view(
            column(14.0),
            vec![
                heading(b, "Recently opened", "clock"),
                view(column(0.0), recent_rows),
            ],
        ),
    ];
    if b.context.directories_only() {
        sections.pop();
    }
    view(pad(column(28.0), 32.0, 24.0), sections)
}
