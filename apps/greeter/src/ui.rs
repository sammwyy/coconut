use std::rc::Rc;

use creamui_core::{
    layout::{Dimension, Style},
    BoxedWidget, Painter, Point, Rect, Size, StateStyle, Styled, TextAlign, Widget,
};
use creamui_image::{ImageData, SvgSize};
use creamui_render::AppHandle;
use creamui_theme::{Color, ColorScheme, Theme};
use creamui_widgets::{
    layout::{fixed, Align, Flex, Justify, StyleExt},
    AppTitlebar, Avatar, Icon, IconImage, IconSource, RawButton, Select, Spinner, Text, TextInput,
};

use crate::{Greeter, NoticeKind, User};

pub const BACKGROUND: Color = Color::rgb(25, 30, 33);
const WHITE: Color = Color::rgb(248, 249, 251);
const SECONDARY: Color = Color::rgba(255, 255, 255, 190);
const GLASS: Color = Color::rgba(255, 255, 255, 25);
const EDGE: Color = Color::rgba(255, 255, 255, 58);
const CLEAR: Color = Color::rgba(0, 0, 0, 0);
const FONT: &str = "Plus Jakarta Sans, system-ui";

pub struct Assets {
    wallpaper: ImageData,
    person: IconSource,
    power: IconSource,
    restart: IconSource,
    arrow: IconSource,
}

impl Assets {
    pub fn load() -> Self {
        Self {
            wallpaper: ImageData::from_bytes(include_bytes!(
                "../../../assets/wallpapers/dandelion.webp"
            ))
            .expect("bundled desktop wallpaper"),
            person: icon(
                r#"<circle cx="12" cy="8" r="4" fill="black" stroke="none"/><path d="M4 21v-2a8 8 0 0 1 16 0v2" fill="black" stroke="none"/>"#,
            ),
            power: icon(r#"<path d="M12 3v8m-5.7-6a8 8 0 1 0 11.4 0"/>"#),
            restart: icon(r#"<path d="M20 10a8 8 0 1 0-1 7M20 3v7h-7"/>"#),
            arrow: icon(r#"<path d="M5 12h14m-6-6 6 6-6 6"/>"#),
        }
    }
}

fn icon(path: &str) -> IconSource {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{path}</svg>"##
    );
    let image =
        ImageData::from_svg(svg.as_bytes(), SvgSize::Max(192)).expect("bundled greeter icon");
    IconSource::Image(IconImage {
        image: image.image().clone(),
        monochrome: true,
    })
}

pub fn load_fonts() {
    for (weight, bytes) in [
        (
            creamui_fonts::FontWeight::Regular,
            include_bytes!("../../../assets/fonts/plus-jakarta-sans/Regular.ttf").as_slice(),
        ),
        (
            creamui_fonts::FontWeight::Bold,
            include_bytes!("../../../assets/fonts/plus-jakarta-sans/SemiBold.ttf").as_slice(),
        ),
    ] {
        if let Err(error) = creamui_fonts::register_bytes("Plus Jakarta Sans", weight, bytes) {
            eprintln!("coconut-greeter: unable to load interface font: {error}");
        }
    }
    creamui_fonts::set_preferred_family(Some(FONT.into()));
}

pub fn theme() -> Theme {
    Theme {
        colors: ColorScheme {
            text_disabled: SECONDARY,
            ..ColorScheme::dark()
        },
        font_family: FONT,
        input_radius: 10.0,
        button_radius: 8.0,
        menu_radius: 10.0,
        menu_item_radius: 6.0,
        ..Theme::dark()
    }
}

fn text(value: impl Into<String>, size: f32, color: Color) -> Text {
    Text::new(value).font_size(size).color(color)
}

pub fn view(state: &Greeter, app: &AppHandle, size: Size) -> BoxedWidget {
    let inset = if size.width < 600.0 { 24.0 } else { 36.0 };
    let sessions = state.sessions.get();
    let users = state.users.get();
    let selected = state.selected_user.get();
    let busy = state.busy.get();
    let mut root = Flex::column().fill();
    if state.backend.is_preview() {
        root = root.child(Box::new(
            AppTitlebar::new(size.width)
                .height(32.0)
                .foreground(SECONDARY),
        ));
    }

    let mut header = Flex::row()
        .full_width()
        .height(36.0)
        .shrink(0.0)
        .align(Align::Center);
    if sessions.len() > 1 {
        let options: Vec<_> = sessions
            .iter()
            .map(|session| session.name.as_str())
            .collect();
        if busy {
            header = header.child(Box::new(
                text(options[state.session_menu.peek_selected()], 13.0, SECONDARY)
                    .height(36.0)
                    .padding_left(12.0),
            ));
        } else {
            header = header.child(Box::new(
                Select::controlled(&options, state.session_menu.clone())
                    .width(190.0)
                    .height(36.0),
            ));
        }
    }

    let account = users
        .iter()
        .find(|user| Some(&user.username) == selected.as_ref());
    let center = Flex::column()
        .full_width()
        .grow(1.0)
        .align(Align::Center)
        .justify(Justify::Center)
        .child(login(state, app, account, size.width - inset * 2.0));

    let mut footer = Flex::row()
        .full_width()
        .height(64.0)
        .shrink(0.0)
        .gap(20.0)
        .align(Align::End)
        .justify(Justify::Between);
    let users_width = (size.width - inset * 2.0 - 160.0).max(0.0);
    if users.len() > 1 {
        footer = footer.child(user_selector(state, app, &users, users_width));
    } else {
        footer = footer.child(Box::new(Flex::row().grow(1.0)));
    }
    let mut power = Flex::row().gap(24.0).shrink(0.0).align(Align::End);
    for (label, action, icon) in [
        ("Restart", "reboot", state.assets.restart.clone()),
        ("Shut down", "poweroff", state.assets.power.clone()),
    ] {
        let state_click = state.clone();
        let app = app.clone();
        power = power.child(Box::new(
            RawButton::new(
                Style::default()
                    .flex_column()
                    .align(Align::Center)
                    .gap(8.0)
                    .width(58.0)
                    .shrink(0.0),
                move || state_click.power(&app, action, label),
            )
            .corner_radius(8.0)
            .hover_style(StateStyle::new().background(GLASS))
            .focus_style(StateStyle::new().outline(WHITE, 1.5))
            .disabled(busy)
            .child(Box::new(
                Flex::row()
                    .size(36.0, 36.0)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .background(GLASS)
                    .border(EDGE, 1.0)
                    .corner_radius(18.0)
                    .child(Box::new(Icon::new(icon, WHITE).size(19.0))),
            ))
            .child(Box::new(text(label, 10.0, SECONDARY))),
        ));
    }
    footer = footer.child(Box::new(power));
    root = root.child(Box::new(
        Flex::column()
            .full_width()
            .grow(1.0)
            .padding(inset)
            .child(Box::new(header))
            .child(Box::new(center))
            .child(Box::new(footer)),
    ));
    Box::new(Desktop {
        wallpaper: state.assets.wallpaper.clone(),
        content: Some(Box::new(root)),
    })
}

fn avatar(state: &Greeter, size: f32, selected: bool) -> Avatar {
    Avatar::new(size)
        .fallback_color(Color::rgba(126, 144, 159, 210))
        .ring(
            if selected { WHITE } else { EDGE },
            if selected { 2.0 } else { 1.0 },
        )
        .image(Box::new(
            Flex::row()
                .size(size, size)
                .justify(Justify::Center)
                .align(Align::Center)
                .child(Box::new(
                    Icon::new(state.assets.person.clone(), WHITE).size(size * 0.56),
                )),
        ))
}

fn login(state: &Greeter, app: &AppHandle, user: Option<&User>, available: f32) -> BoxedWidget {
    let width = available.clamp(0.0, 280.0);
    let busy = state.busy.get();
    let prompt = state.prompt.get();
    let notice = state.notice.get();
    let submit = {
        let state = state.clone();
        let app = app.clone();
        move || state.submit(&app)
    };
    let mut input = TextInput::controlled(&state.response)
        .width(width)
        .height(42.0)
        .padding_right(46.0)
        .background(Color::rgba(255, 255, 255, 32))
        .border(EDGE, 1.0)
        .color(WHITE)
        .placeholder(
            prompt
                .as_ref()
                .map_or("Password", |prompt| prompt.message.trim_end_matches(':')),
        )
        .on_submit(submit.clone());
    if !prompt.as_ref().is_some_and(|prompt| prompt.echo) {
        input = input.password();
    }
    let field = Flex::row()
        .size(width, 42.0)
        .child(Box::new(input))
        .child(Box::new(
            RawButton::new(
                Style {
                    position: creamui_core::layout::Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: creamui_core::layout::LengthPercentageAuto::Auto,
                        right: creamui_core::layout::LengthPercentageAuto::Length(5.0),
                        top: creamui_core::layout::LengthPercentageAuto::Length(5.0),
                        bottom: creamui_core::layout::LengthPercentageAuto::Auto,
                    },
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .justify(Justify::Center)
                        .size(32.0, 32.0)
                },
                submit,
            )
            .background(Color::rgba(255, 255, 255, 24))
            .corner_radius(7.0)
            .hover_style(StateStyle::new().background(Color::rgba(255, 255, 255, 55)))
            .focus_style(StateStyle::new().outline(WHITE, 1.5))
            .disabled(busy || user.is_none())
            .child(if busy {
                Box::new(Spinner::new().size(16.0))
            } else {
                Box::new(Icon::new(state.assets.arrow.clone(), WHITE).size(18.0))
            }),
        ));
    let mut login = Flex::column()
        .width(width)
        .shrink(0.0)
        .align(Align::Center)
        .gap(18.0)
        .child(Box::new(avatar(state, 112.0, false)))
        .child(Box::new(
            text(user.map_or("", display_name), 23.0, WHITE)
                .bold(true)
                .text_align(TextAlign::Center),
        ))
        .child(Box::new(field))
        .child(Box::new(
            Flex::column()
                .width(width)
                .height(48.0)
                .align(Align::Center)
                .child(Box::new(
                    text(
                        notice.text,
                        12.0,
                        match notice.kind {
                            NoticeKind::Error => Color::rgb(255, 211, 204),
                            _ => SECONDARY,
                        },
                    )
                    .width(width)
                    .text_align(TextAlign::Center),
                )),
        ));
    if let Some(demo) = &state.demo {
        login = login.child(Box::new(
            Flex::column()
                .width(width)
                .padding_xy(14.0, 11.0)
                .gap(5.0)
                .background(Color::rgba(15, 20, 25, 90))
                .border(Color::rgba(255, 255, 255, 32), 1.0)
                .corner_radius(9.0)
                .child(Box::new(
                    text("Live session login", 11.0, SECONDARY).bold(true),
                ))
                .child(Box::new(text(
                    format!("Username: {}\nPassword: {}", demo.username, demo.password),
                    12.0,
                    WHITE,
                ))),
        ));
    }
    Box::new(login)
}

fn display_name(user: &User) -> &str {
    if user.display_name.trim().is_empty() {
        &user.username
    } else {
        &user.display_name
    }
}

fn user_selector(state: &Greeter, app: &AppHandle, users: &[User], width: f32) -> BoxedWidget {
    let mut row = Flex::row()
        .gap(12.0)
        .height(64.0)
        .shrink(0.0)
        .align(Align::End);
    let selected = state.selected_user.get();
    for user in users {
        let state_click = state.clone();
        let app = app.clone();
        let username = user.username.clone();
        let active = selected.as_ref() == Some(&username);
        let name: String = display_name(user).chars().take(12).collect();
        row = row.child(Box::new(
            RawButton::new(
                Style::default()
                    .flex_column()
                    .align(Align::Center)
                    .gap(7.0)
                    .width(62.0)
                    .shrink(0.0),
                move || state_click.select_user(&app, username.clone()),
            )
            .corner_radius(8.0)
            .hover_style(StateStyle::new().background(GLASS))
            .focus_style(StateStyle::new().outline(WHITE, 1.5))
            .disabled(state.busy.get())
            .child(Box::new(avatar(state, 36.0, active)))
            .child(Box::new(text(
                name,
                10.0,
                if active { WHITE } else { SECONDARY },
            ))),
        ));
    }
    let maximum = (users.len() as f32 * 74.0 - 12.0 - width).max(0.0);
    Box::new(UserStrip {
        width,
        row: Some(Box::new(row)),
        offset: state.users_offset.get().min(maximum),
        on_scroll: {
            let offset = state.users_offset.clone();
            Rc::new(move |delta| {
                offset.update(|value| *value = (*value + delta).clamp(0.0, maximum))
            })
        },
    })
}

struct UserStrip {
    width: f32,
    row: Option<BoxedWidget>,
    offset: f32,
    on_scroll: Rc<dyn Fn(f32)>,
}

impl Widget for UserStrip {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: fixed(self.width, 64.0),
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.row.take().into_iter().collect()
    }
    fn clips_children(&self) -> bool {
        true
    }
    fn scroll_offset(&self) -> Point {
        Point {
            x: self.offset,
            y: 0.0,
        }
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        Some(self.on_scroll.clone())
    }
}

struct Desktop {
    wallpaper: ImageData,
    content: Option<BoxedWidget>,
}

impl Widget for Desktop {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let scale = (rect.width / self.wallpaper.width() as f32)
            .max(rect.height / self.wallpaper.height() as f32);
        let width = self.wallpaper.width() as f32 * scale;
        let height = self.wallpaper.height() as f32 * scale;
        painter.draw_image(
            Rect {
                x: rect.x + (rect.width - width) / 2.0,
                y: rect.y + (rect.height - height) / 2.0,
                width,
                height,
            },
            self.wallpaper.image(),
            None,
        );
        painter.fill_rect(rect, Color::rgba(5, 10, 16, 90), 0.0);
        painter.fill_linear_gradient(rect, CLEAR, Color::rgba(4, 8, 12, 110), 180.0, 0.0);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.content.take().into_iter().collect()
    }
    fn clips_children(&self) -> bool {
        true
    }
}
