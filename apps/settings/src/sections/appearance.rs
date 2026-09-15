use crate::common::section;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{Text, TextSize};
use std::cell::RefCell;
use std::rc::Rc;

const CARD_W: f32 = 140.0;
const CARD_H: f32 = 96.0;
const PREVIEW_H: f32 = 64.0;

pub fn build(
    _: Size,
    active_theme_id: &Signal<String>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let theme = use_theme();
    let active_id = active_theme_id.get();
    let cards: Vec<BoxedWidget> = creamui_theme::list_themes()
        .into_iter()
        .map(|info| {
            let is_active = info.id == active_id;
            let active_theme_id = active_theme_id.clone();
            let window = window.clone();
            let id = info.id.clone();
            let preview: BoxedWidget = match info
                .thumbnail
                .as_ref()
                .and_then(|path| ImageData::from_path(path).ok())
            {
                Some(data) => Box::new(
                    Image::new(data)
                        .layout(creamui_core::layout::Style {
                            size: fixed(CARD_W, PREVIEW_H),
                            ..Default::default()
                        })
                        .fit(ImageFit::Cover),
                ),
                None => Box::new(jsx! {
                    <Flex direction={FlexDirection::Row} size={(CARD_W, PREVIEW_H)}>
                        <Flex size={(CARD_W * 0.7, PREVIEW_H)} background={info.colors.surface} />
                        <Flex size={(CARD_W * 0.3, PREVIEW_H)} background={info.colors.accent} />
                    </Flex>
                }),
            };
            let button_style = creamui_core::Style {
                layout: creamui_core::layout::Style {
                    size: fixed(CARD_W, CARD_H),
                    ..Default::default()
                },
                ..Default::default()
            };
            let card: BoxedWidget = Box::new(jsx! {
                <RawButton
                    style={button_style}
                    background={theme.colors.surface}
                    corner_radius={theme.card_radius}
                    on_click={move || {
                        if creamui_theme::set_active_theme(&id) {
                            active_theme_id.set(id.clone());
                            if let Some(handle) = window.borrow().as_ref() {
                                handle.set_theme(creamui_theme::active_theme());
                            }
                        }
                    }}
                >
                    <Flex direction={FlexDirection::Column} align={Align::Center} justify={Justify::Center} gap={6.0} padding={4.0}>
                        {preview}
                        {Box::new(Text::new(title_case(&info.id)).size(TextSize::Sm)) as BoxedWidget}
                    </Flex>
                </RawButton>
            });
            // A ring around the selected card, mac-style, rather than a
            // filled highlight: an outer box tinted with the accent color,
            // showing only as the inset padding around the card inside it.
            let ring = if is_active {
                theme.colors.accent
            } else {
                Color::rgba(0, 0, 0, 0)
            };
            Box::new(jsx! {
                <Flex
                    size={(CARD_W + 6.0, CARD_H + 6.0)}
                    align={Align::Center}
                    justify={Justify::Center}
                    background={ring}
                    corner_radius={theme.card_radius + 3.0}
                >
                    {card}
                </Flex>
            }) as BoxedWidget
        })
        .collect();

    section(
        "Appearance",
        "The color theme shared by Coconut and every other CreamUI app.",
        vec![Box::new(jsx! {
            <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
        })],
    )
}

fn title_case(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
