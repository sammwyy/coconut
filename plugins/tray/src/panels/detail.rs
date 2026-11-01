use coconut_plugin_kit::chrome::{
    shell_border, shell_card, shell_muted, shell_panel, shell_text, ISLAND_RADIUS,
};
use creamui_core::{BoxedWidget, Size, Style, Styled};
use creamui_widgets::{layout::Flex, RawScrollView, RawText, ScrollController};

pub(super) fn panel(
    size: Size,
    header: BoxedWidget,
    scroll: ScrollController,
    content: BoxedWidget,
) -> BoxedWidget {
    Box::new(
        Flex::column()
            .size(size.width, size.height)
            .padding(16.0)
            .gap(12.0)
            .background(shell_card())
            .border(shell_border(), 1.0)
            .child(header)
            .child(Box::new(
                RawScrollView::controlled(
                    Style::new().layout(creamui_core::layout::Style {
                        flex_grow: 1.0,
                        ..Default::default()
                    }),
                    scroll,
                )
                .child(content),
            )),
    )
}

pub(super) fn card(content: BoxedWidget) -> BoxedWidget {
    selected_card(content, false)
}

pub(super) fn selected_card(content: BoxedWidget, selected: bool) -> BoxedWidget {
    Box::new(
        Flex::column()
            .padding(12.0)
            .gap(10.0)
            .background(if selected {
                shell_panel().mix(coconut_plugin_kit::chrome::shell_selected(), 0.18)
            } else {
                shell_panel()
            })
            .border(
                if selected {
                    shell_border().mix(coconut_plugin_kit::chrome::shell_accent(), 0.4)
                } else {
                    shell_border()
                },
                1.0,
            )
            .corner_radius(ISLAND_RADIUS)
            .child(content),
    )
}

pub(super) fn toggle(label: &str, checked: bool, change: std::rc::Rc<dyn Fn()>) -> BoxedWidget {
    Box::new(
        Flex::row()
            .gap(8.0)
            .align(creamui_widgets::layout::Align::Center)
            .child(text(label.to_owned(), true, 12.0))
            .child(Box::new(Flex::row().grow(1.0)))
            .child(coconut_plugin_kit::chrome::compact_switch(checked, change)),
    )
}

pub(super) fn text(value: impl Into<String>, primary: bool, size: f32) -> BoxedWidget {
    Box::new(RawText::new(
        value,
        if primary { shell_text() } else { shell_muted() },
        size,
    ))
}

pub(super) fn error(value: String) -> BoxedWidget {
    card(text(value, true, 12.0))
}

pub(super) fn slider_width(size: Size) -> f32 {
    (size.width - 68.0).max(80.0)
}

#[derive(Clone, Default)]
struct Scrolls(
    std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, ScrollController>>>,
);

pub(super) fn scroll_for(
    scratch: &coconut_plugin_kit::SharedState,
    key: String,
) -> ScrollController {
    let scrolls = scratch.get_or_insert_with(Scrolls::default);
    let result = scrolls.0.borrow_mut().entry(key).or_default().clone();
    result
}
