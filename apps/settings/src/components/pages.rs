use crate::components;

use creamui_core::layout::FlexDirection;
use creamui_core::BoxedWidget;
use creamui_macros::jsx;
use creamui_widgets::{Text, TextSize};

pub(crate) fn unavailable_page(name: &str) -> BoxedWidget {
    components::section(
        "",
        "",
        vec![components::group(vec![components::row(
            "Not available yet",
            Box::new(
                Text::secondary(format!(
                    "{name} is part of the Settings layout, but Coconut does not support it yet."
                ))
                .size(TextSize::Sm),
            ),
        )])],
    )
}

pub(crate) fn category_pages(pages: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={32.0} children={pages} />
    })
}
