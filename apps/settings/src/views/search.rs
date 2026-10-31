use crate::{
    components::{group, labeled_group, setting_row},
    routes,
    views::context::ViewContext,
};
use coconut_plugin_kit::bar_editor::BAR_ITEMS;
use creamui_core::{BoxedWidget, Styled};
use creamui_widgets::{layout::Flex, Icon, Symbol, Text, TextSize};

#[derive(Debug)]
pub(crate) struct Result {
    pub route: String,
    pub title: String,
    pub description: String,
    pub widget: bool,
}

pub(crate) fn results(query: &str) -> Vec<Result> {
    let query = query.trim().to_lowercase();
    let mut results = BAR_ITEMS
        .iter()
        .filter(|item| super::desktop::islands::matches(item, &query))
        .map(|item| Result {
            route: item.settings_route.into(),
            title: item.label.into(),
            description: item.description.into(),
            widget: true,
        })
        .collect::<Vec<_>>();
    results.extend(
        routes::searchable_pages()
            .into_iter()
            .filter(|(route, title)| {
                let haystack = format!("{route} {title}").to_lowercase();
                query.split_whitespace().all(|word| haystack.contains(word))
            })
            .map(|(route, title)| Result {
                route,
                title,
                description: "System settings".into(),
                widget: false,
            }),
    );
    results
}

pub(crate) fn build(context: &ViewContext, query: &str) -> BoxedWidget {
    let mut body = Vec::new();
    let results = results(query);
    for (widget, title) in [(true, "Widgets"), (false, "Settings")] {
        let rows = results
            .iter()
            .filter(|result| result.widget == widget)
            .map(|result| {
                let route = result.route.clone();
                let router = context.view.clone();
                let search = context.settings_search.clone();
                setting_row::item(
                    &result.title,
                    &result.description,
                    Box::new(Icon::new(
                        Symbol::ChevronRight,
                        creamui_theme::use_theme().colors.text_secondary,
                    )),
                    move || {
                        creamui_reactive::batch(|| {
                            search.set_value("");
                            routes::open(&router, &route);
                        });
                    },
                )
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            body.push(labeled_group(title, group(rows)));
        }
    }
    if body.is_empty() {
        body.push(group(vec![Box::new(
            Flex::column()
                .padding(20.0)
                .gap(6.0)
                .child(Box::new(Text::new("No matching settings").bold(true)))
                .child(Box::new(
                    Text::secondary(
                        "Try a widget name or a topic such as clipboard, sound or display.",
                    )
                    .size(TextSize::Sm),
                )),
        )]));
    }
    Box::new(Flex::column().gap(24.0).with_children(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_search_finds_widgets_by_task_and_keeps_permissions_distinct() {
        let clipboard = results("clipboard");
        assert!(clipboard
            .iter()
            .any(|result| result.route == "/desktop/widgets/clipboard"));
        assert!(clipboard
            .iter()
            .any(|result| result.route == "/privacy/clipboard"));
        assert!(results("sound")
            .iter()
            .any(|result| result.route == "/desktop/widgets/tray.volume"));
        assert!(results("tray")
            .iter()
            .any(|result| result.route == "/desktop/widgets/control_center"));
        assert!(results("there is no setting with this name").is_empty());
    }
}
