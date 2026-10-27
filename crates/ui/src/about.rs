//! Shared About layout; the `about!` macro feeds each application's metadata.
use creamui_core::{
    layout::{
        AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, LengthPercentage, Style,
    },
    BoxedWidget, StateStyle, Styled, TextAlign,
};
use creamui_theme::use_theme;
use creamui_widgets::{Icon, IconSource, Link, RawView, Text};

pub struct Metadata {
    pub version: &'static str,
    pub description: &'static str,
    pub authors: &'static str,
    pub license: &'static str,
    pub repository: &'static str,
}

pub fn build(display_name: &str, icon: impl Into<IconSource>, metadata: Metadata) -> BoxedWidget {
    let theme = use_theme();
    let repository = metadata.repository.trim_end_matches('/');
    let identity = RawView::new(column(10.0)).with_children(vec![
        Box::new(
            RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(72.0),
                    height: Dimension::Length(72.0),
                },
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::Center),
                ..Default::default()
            })
            .background(theme.colors.surface_elevated)
            .corner_radius(20.0)
            .child(Box::new(
                Icon::new(icon.into(), theme.colors.accent).size(38.0),
            )),
        ),
        Box::new(
            Text::new(display_name)
                .font_size(30.0)
                .bold(true)
                .text_align(TextAlign::Center),
        ),
        Box::new(
            Text::secondary(format!("Version {}", metadata.version))
                .font_size(12.0)
                .text_align(TextAlign::Center),
        ),
        Box::new(
            Text::secondary(metadata.description)
                .font_size(13.0)
                .text_align(TextAlign::Center)
                .layout(full_width()),
        ),
    ]);
    let details = RawView::new(column(6.0)).with_children(vec![
        Box::new(
            Text::new(format!(
                "Created by {}",
                metadata.authors.replace(':', ", ")
            ))
            .font_size(13.0)
            .text_align(TextAlign::Center),
        ),
        Box::new(
            Text::secondary(metadata.license)
                .font_size(12.0)
                .text_align(TextAlign::Center),
        ),
    ]);
    let links = [
        ("Source code", repository.to_owned()),
        ("Report an issue", format!("{repository}/issues")),
        ("Documentation", format!("{repository}/blob/HEAD/README.md")),
        ("Releases", format!("{repository}/releases")),
    ]
    .into_iter()
    .map(|(label, url)| {
        Box::new(
            Link::new(label, move || open_link(&url))
                .font_size(13.0)
                .layout(Style {
                    flex_shrink: 0.0,
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(22.0),
                    },
                    ..Default::default()
                })
                .focus_style(StateStyle::new().outline(theme.colors.accent, 1.5)),
        ) as BoxedWidget
    })
    .collect();
    Box::new(
        RawView::new(Style {
            padding: creamui_core::layout::Rect {
                top: LengthPercentage::Length(24.0),
                bottom: LengthPercentage::Length(24.0),
                left: LengthPercentage::Length(0.0),
                right: LengthPercentage::Length(0.0),
            },
            ..column(28.0)
        })
        .with_children(vec![
            Box::new(identity),
            Box::new(details),
            Box::new(
                RawView::new(Style {
                    flex_wrap: FlexWrap::Wrap,
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    gap: creamui_core::layout::Size {
                        width: LengthPercentage::Length(24.0),
                        height: LengthPercentage::Length(12.0),
                    },
                    ..full_width()
                })
                .with_children(links),
            ),
        ]),
    )
}

fn open_link(url: &str) {
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        // The first quoted argument of `start` is a window title.
        command.args(["/C", "start", ""]);
        command
    };
    match command.arg(url).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("Couldn't open link: {error}"),
    }
}

fn full_width() -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Center),
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        },
        ..full_width()
    }
}
