pub(crate) use crate::{
    browser::BrowserState,
    context::use_is_portal,
    filesystem as files,
    navigation::{Location, Tab},
    portal::PortalType,
};
pub(crate) use creamui_core::{
    layout::{
        AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, LengthPercentage, Style,
    },
    BoxedWidget, Size, StateStyle, Styled,
};
pub(crate) use creamui_theme::{use_theme, Color};
pub(crate) use creamui_widgets::{
    CUIWindowDragArea, Icon, RawButton, RawScrollView, RawView, Text, TextInput,
};
pub(crate) use std::{
    path::{Component, Path},
    rc::Rc,
};

pub(crate) mod navigation;
pub(crate) mod portal_footer;
pub(crate) mod sidebar;
pub(crate) const CLEAR: Color = Color::rgba(0, 0, 0, 0);

pub(crate) fn row(gap: f32) -> Style {
    Style {
        align_items: Some(AlignItems::Center),
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    }
}

pub(crate) fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        ..row(gap)
    }
}

pub(crate) fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        },
        flex_shrink: 0.0,
        ..row(0.0)
    }
}

pub(crate) fn pad(mut style: Style, x: f32, y: f32) -> Style {
    style.padding = creamui_core::layout::Rect {
        left: LengthPercentage::Length(x),
        right: LengthPercentage::Length(x),
        top: LengthPercentage::Length(y),
        bottom: LengthPercentage::Length(y),
    };
    style
}

pub(crate) fn view(style: Style, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(RawView::new(style).with_children(children))
}

pub(crate) fn label(text: impl Into<String>, size: f32, secondary: bool) -> BoxedWidget {
    Box::new(if secondary {
        Text::secondary(text).font_size(size)
    } else {
        Text::new(text).font_size(size)
    })
}

pub(crate) fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

pub(crate) fn glyph(b: &BrowserState, name: &str, size: f32, color: Color) -> BoxedWidget {
    Box::new(Icon::new(b.icons.get(name), color).size(size))
}

pub(crate) fn button(
    style: Style,
    active: bool,
    click: impl Fn() + 'static,
    children: Vec<BoxedWidget>,
) -> RawButton {
    let t = use_theme();
    RawButton::new(style, click)
        .background(if active {
            t.colors.selection_background
        } else {
            CLEAR
        })
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .with_children(children)
}

pub(crate) fn icon_button(
    b: &BrowserState,
    icon: &str,
    enabled: bool,
    active: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        button(
            Style {
                justify_content: Some(JustifyContent::Center),
                ..fixed(32.0, 32.0)
            },
            active,
            click,
            vec![glyph(
                b,
                icon,
                17.0,
                if !enabled {
                    t.colors.text_disabled
                } else if active {
                    t.colors.accent
                } else {
                    t.colors.text_secondary
                },
            )],
        )
        .disabled(!enabled),
    )
}

pub(crate) fn entry_button(
    b: &BrowserState,
    style: Style,
    active: bool,
    entry: files::Entry,
    children: Vec<BoxedWidget>,
) -> RawButton {
    let context = b.clone();
    let context_path = entry.path.clone();
    let context_directory = entry.directory;
    let pointer = b.clone();
    let keyboard = b.clone();
    let keyboard_entry = entry.clone();
    let skip_click = Rc::new(std::cell::Cell::new(false));
    let skip_key_click = skip_click.clone();
    button(
        style,
        active,
        move || {
            if !skip_click.replace(false) {
                pointer.select(entry.clone());
            }
        },
        children,
    )
    .with_context_menu(move |point| {
        context.open_context_menu(
            point,
            coconut_file_menu::FileMenuTarget::Item {
                path: context_path.clone(),
                directory: context_directory,
            },
        )
    })
    .on_key_press(move |input| {
        if input.key == creamui_core::Key::Enter && !input.modifiers.ctrl {
            // RawButton invokes its click callback after Enter. Consume that
            // synthetic click so keyboard activation does not select twice.
            skip_key_click.set(true);
            if keyboard.context.is_save() && !keyboard_entry.directory {
                keyboard.select(keyboard_entry.clone());
            } else {
                keyboard.activate(keyboard_entry.path.clone(), keyboard_entry.directory);
            }
        }
    })
}

pub(crate) fn action(
    text: &str,
    primary: bool,
    enabled: bool,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        button(
            pad(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(34.0),
                    },
                    justify_content: Some(JustifyContent::Center),
                    flex_shrink: 0.0,
                    ..row(0.0)
                },
                16.0,
                0.0,
            ),
            false,
            click,
            vec![Box::new(Text::new(text).font_size(13.0).color(
                if !enabled {
                    t.colors.text_disabled
                } else if primary {
                    t.colors.selection_text
                } else {
                    t.colors.text_primary
                },
            ))],
        )
        .background(if primary && enabled {
            t.colors.accent
        } else {
            t.colors.surface_elevated
        })
        .hover_style(StateStyle::new().background(if primary {
            t.colors.accent.mix(t.colors.text_primary, 0.1)
        } else {
            t.colors.surface_hover
        }))
        .disabled(!enabled),
    )
}

pub(crate) fn heading(b: &BrowserState, title: &str, icon: &str) -> BoxedWidget {
    view(
        row(12.0),
        vec![
            glyph(b, icon, 21.0, use_theme().colors.text_primary),
            Box::new(Text::new(title).font_size(20.0).bold(true)),
        ],
    )
}

pub(crate) fn wrap(children: Vec<BoxedWidget>, gap: f32) -> BoxedWidget {
    view(
        Style {
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::Start),
            ..row(gap)
        },
        children,
    )
}

pub(crate) fn file_icon(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "svg" | "webp" | "gif" => "image",
        "mp3" | "ogg" | "wav" | "flac" => "music",
        "mp4" | "mkv" | "webm" | "mov" => "video",
        _ => "file",
    }
}

pub(crate) fn empty_view(b: &BrowserState, title: &str, detail: &str) -> BoxedWidget {
    view(
        pad(
            Style {
                align_items: Some(AlignItems::Center),
                ..column(12.0)
            },
            12.0,
            60.0,
        ),
        vec![
            glyph(b, "folder", 40.0, use_theme().colors.text_disabled),
            label(title, 17.0, false),
            label(detail, 12.0, true),
        ],
    )
}
