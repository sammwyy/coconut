use creamui_core::BoxedWidget;
use creamui_widgets::Select;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let appearance = &context.appearance;
    let handle = &context.window;
    let s = state.snapshot.get();
    let body = {
        let selected = appearance.get();
        let family = selected
            .font_family
            .as_deref()
            .unwrap_or("Plus Jakarta Sans")
            .split(',')
            .next()
            .unwrap_or_default()
            .trim();
        let fonts = s.entries("fonts").to_vec();
        let labels: Vec<_> = fonts.iter().map(|f| f.name.as_str()).collect();
        let index = fonts.iter().position(|f| f.name == family).unwrap_or(0);
        let appearance = appearance.clone();
        let handle = handle.clone();
        let control = if fonts.is_empty() {
            value(family)
        } else {
            Box::new(
                Select::controlled(&labels, state.select("interface-font", index))
                    .searchable()
                    .on_select(move |i| {
                        if let Some(font) = fonts.get(i) {
                            let current = appearance.peek();
                            crate::views::personalization::appearance::apply(
                                creamui_theme::AppearanceSelection {
                                    theme: Some(current.theme_id),
                                    variant: Some(current.variant_id),
                                    accent: Some(current.accent),
                                    font_family: Some(format!("{}, system-ui", font.name)),
                                    corners: Some(current.corners),
                                },
                                &appearance,
                                &handle,
                            );
                        }
                    }),
            ) as BoxedWidget
        };
        vec![card(
            "Typography",
            vec![
                item(
                    "Interface font",
                    "Installed fonts and Coconut's bundled font",
                    control,
                    || {},
                ),
                state.preference(
                    "Text scaling",
                    "text-scale",
                    "Controlled by the active desktop",
                ),
            ],
        )]
    };
    detail_page(body, state)
}
