use super::*;
use crate::browser::valid_filename;

pub(crate) fn portal_footer(b: &BrowserState, tab: &Tab) -> BoxedWidget {
    let portal = use_is_portal().expect("portal footer requires a chooser context");
    let t = use_theme();
    let cancel = b.clone();
    let accept = b.clone();
    let save = portal.request.kind == PortalType::SaveFile;
    let mut children = Vec::new();
    if save {
        children.push(label("File name", 12.0, true));
        let state = b.clone();
        children.push(Box::new(
            TextInput::controlled(&b.name)
                .layout(Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    ..TextInput::default_style()
                })
                .font_size(13.0)
                .on_submit(move || state.accept()),
        ) as BoxedWidget);
    } else {
        children.push(view(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                ..row(0.0)
            },
            vec![label(short(&b.context.request.title, 38), 13.0, false)],
        ));
    }
    children.push(action("Cancel", false, true, move || cancel.close()));
    let enabled = if save {
        matches!(tab.location(), Location::Directory(_)) && valid_filename(&b.name.value())
    } else if b.context.directories_only() {
        matches!(tab.location(), Location::Directory(_))
    } else {
        b.visible_selection(tab).is_some()
    };
    let replacing = match tab.location() {
        Location::Directory(p) => b.overwrite.get().as_ref() == Some(&p.join(b.name.value())),
        _ => false,
    };
    let accept_label = if save && replacing {
        "Replace".into()
    } else {
        portal.accept_label()
    };
    children.push(action(&accept_label, true, enabled, move || {
        accept.accept()
    }));
    Box::new(
        RawView::new(pad(
            Style {
                flex_shrink: 0.0,
                ..row(12.0)
            },
            20.0,
            14.0,
        ))
        .background(t.colors.surface.mix(t.colors.accent, 0.025))
        .with_children(children),
    )
}

pub(crate) fn filter_bar(b: &BrowserState) -> Option<BoxedWidget> {
    let portal = use_is_portal()?;
    if portal.request.kind == PortalType::SelectFolder || portal.request.filters.is_empty() {
        return None;
    }
    let labels = portal
        .request
        .filters
        .iter()
        .map(|filter| filter.name.as_str())
        .collect::<Vec<_>>();
    let browser = b.clone();
    Some(view(
        pad(
            Style {
                flex_shrink: 0.0,
                ..row(12.0)
            },
            24.0,
            6.0,
        ),
        vec![
            view(
                Style {
                    flex_grow: 1.0,
                    ..row(0.0)
                },
                vec![label(short(&b.context.request.title, 40), 13.0, false)],
            ),
            label("File type", 12.0, true),
            Box::new(
                creamui_widgets::Select::controlled(&labels, portal.filter)
                    .width(220.0)
                    .height(34.0)
                    .on_select(move |_| {
                        creamui_reactive::batch(|| {
                            browser.edit(|tab| tab.selected = None);
                            browser.message.set(None);
                            browser.overwrite.set(None);
                        });
                    }),
            ),
        ],
    ))
}
