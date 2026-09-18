use crate::common::{group, row};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Style};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_theme::{AppearanceSelection, Color, CornerStyle, ResolvedAppearance};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{ColorPicker, ColorPickerController, SegmentedControl, Text, TextSize};
use std::cell::RefCell;
use std::rc::Rc;

const THEME_CARD: (f32, f32) = (116.0, 82.0);
const VARIANT_CARD: (f32, f32) = (104.0, 70.0);
const DOT: f32 = 26.0;
const CUSTOM_W: f32 = 92.0;

pub fn build(
    _: Size,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    accent_picker: &ColorPickerController,
    custom: &Signal<bool>,
) -> BoxedWidget {
    let current = appearance.get();
    let ui = creamui_theme::use_theme();
    let themes = creamui_theme_loader::list_themes().unwrap_or_else(|error| {
        eprintln!("settings: failed to list CreamUI themes: {error}");
        vec![creamui_theme_loader::builtin_theme()]
    });
    let selected_theme = themes
        .iter()
        .find(|theme| theme.id == current.theme_id)
        .cloned()
        .unwrap_or_else(creamui_theme_loader::builtin_theme);

    let theme_cards = themes
        .into_iter()
        .map(|theme| {
            theme_card(
                theme,
                &current,
                appearance,
                window,
                custom,
                ui.colors.accent,
            )
        })
        .collect::<Vec<_>>();
    let variant_cards = selected_theme
        .variants
        .iter()
        .map(|(id, variant)| {
            variant_card(id, *variant, &current, appearance, window, ui.colors.accent)
        })
        .collect::<Vec<_>>();

    let mut accent_dots = selected_theme
        .accent_presets
        .iter()
        .map(|preset| {
            accent_dot(
                preset.color,
                preset.name.clone(),
                &current,
                appearance,
                window,
                custom,
            )
        })
        .collect::<Vec<_>>();
    let custom_dot = custom_accent_dot(&current, custom);
    let custom_picker = custom_accent_picker(&current, appearance, window, accent_picker, custom);
    accent_dots.push(custom_dot);
    accent_dots.push(custom_picker);

    group(vec![
        row("Theme", selection_control(theme_cards)),
        row("Color scheme", selection_control(variant_cards)),
        row("Accent color", selection_control(accent_dots)),
        row("Corners", corners_control(&current, appearance, window)),
    ])
}

/// Shared by CreamUI widgets and the compositor's window frames.
fn corners_control(
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) -> BoxedWidget {
    let selected = CornerStyle::ALL
        .iter()
        .position(|style| *style == current.corners)
        .unwrap_or(0);
    let set_appearance = appearance.clone();
    let set_window = window.clone();
    let selection = AppearanceSelection {
        theme: Some(current.theme_id.clone()),
        variant: Some(current.variant_id.clone()),
        accent: Some(current.accent),
        font_family: current.font_family.clone(),
        corners: None,
    };
    let control = SegmentedControl::new(selected, move |index| {
        apply(
            AppearanceSelection {
                corners: Some(CornerStyle::ALL[index]),
                ..selection.clone()
            },
            &set_appearance,
            &set_window,
        );
    });
    Box::new(
        CornerStyle::ALL
            .iter()
            .fold(control, |control, style| control.option(title_case(style.id()))),
    )
}

fn selection_control(children: Vec<BoxedWidget>) -> BoxedWidget {
    let ui = creamui_theme::use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} wrap={Wrap::Wrap} gap={ui.spacing_small} children={children} />
    })
}

fn theme_card(
    theme: creamui_theme::ThemeDefinition,
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    custom: &Signal<bool>,
    selected_border: Color,
) -> BoxedWidget {
    let active = theme.id == current.theme_id;
    let preview = theme.default_theme();
    let set_appearance = appearance.clone();
    let set_window = window.clone();
    let set_custom = custom.clone();
    let id = theme.id.clone();
    let variant = theme.default_variant.clone();
    let font_family = current.font_family.clone();
    let corners = Some(current.corners);
    Box::new(jsx! {
        <RawButton
            style={Style { layout: creamui_core::layout::Style { size: fixed(THEME_CARD.0, THEME_CARD.1), ..Default::default() }, ..Default::default() }}
            background={creamui_theme::use_theme().colors.surface_elevated}
            border={(if active { selected_border } else { creamui_theme::use_theme().colors.border }, if active { 2.0 } else { 1.0 })}
            corner_radius={creamui_theme::use_theme().card_radius}
            on_click={move || {
                set_custom.set(false);
                apply(AppearanceSelection { theme: Some(id.clone()), variant: Some(variant.clone()), accent: None, font_family: font_family.clone(), corners }, &set_appearance, &set_window);
            }}
        >
            <Flex direction={FlexDirection::Column} size={THEME_CARD} gap={5.0} padding={6.0}>
                <Flex direction={FlexDirection::Column} grow={1.0} padding={6.0} gap={5.0} background={preview.colors.surface} corner_radius={preview.radius_medium}>
                    <Flex size={(48.0, 5.0)} background={preview.colors.text_primary} corner_radius={3.0} />
                    <Flex size={(70.0, 4.0)} background={preview.colors.text_secondary} corner_radius={3.0} />
                    <Flex grow={1.0} />
                    <Flex size={(38.0, 12.0)} background={preview.colors.accent} corner_radius={6.0} />
                </Flex>
                <RawText color={creamui_theme::use_theme().colors.text_primary} font_size={12.0}>{theme.name}</RawText>
            </Flex>
        </RawButton>
    })
}

fn variant_card(
    id: &str,
    variant: creamui_theme::Theme,
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    selected_border: Color,
) -> BoxedWidget {
    let active = id == current.variant_id;
    let set_appearance = appearance.clone();
    let set_window = window.clone();
    let theme_id = current.theme_id.clone();
    let variant_id = id.to_owned();
    let accent = current.accent;
    let font_family = current.font_family.clone();
    let corners = Some(current.corners);
    let label = title_case(id);
    Box::new(jsx! {
        <RawButton
            style={Style { layout: creamui_core::layout::Style { size: fixed(VARIANT_CARD.0, VARIANT_CARD.1), ..Default::default() }, ..Default::default() }}
            background={creamui_theme::use_theme().colors.surface_elevated}
            border={(if active { selected_border } else { creamui_theme::use_theme().colors.border }, if active { 2.0 } else { 1.0 })}
            corner_radius={creamui_theme::use_theme().card_radius}
            on_click={move || apply(AppearanceSelection { theme: Some(theme_id.clone()), variant: Some(variant_id.clone()), accent: Some(accent), font_family: font_family.clone(), corners }, &set_appearance, &set_window)}
        >
            <Flex direction={FlexDirection::Column} size={VARIANT_CARD} padding={6.0} gap={5.0}>
                <Flex grow={1.0} background={variant.colors.surface} border={(variant.colors.border, 1.0)} corner_radius={variant.radius_medium}>
                    <Flex size={(32.0, 1.0)} background={variant.colors.accent} corner_radius={variant.radius_medium} />
                </Flex>
                <RawText color={creamui_theme::use_theme().colors.text_primary} font_size={12.0}>{label}</RawText>
            </Flex>
        </RawButton>
    })
}

fn accent_dot(
    color: Color,
    _name: String,
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    custom: &Signal<bool>,
) -> BoxedWidget {
    let active = color == current.accent;
    let set_appearance = appearance.clone();
    let set_window = window.clone();
    let set_custom = custom.clone();
    let theme_id = current.theme_id.clone();
    let variant_id = current.variant_id.clone();
    let font_family = current.font_family.clone();
    let corners = Some(current.corners);
    Box::new(jsx! {
        <RawButton
            style={Style { layout: creamui_core::layout::Style { size: fixed(DOT, DOT), ..Default::default() }, ..Default::default() }}
            background={color}
            border={(if active { creamui_theme::use_theme().colors.text_primary } else { color }, if active { 3.0 } else { 1.0 })}
            corner_radius={DOT / 2.0}
            on_click={move || {
                set_custom.set(false);
                apply(AppearanceSelection { theme: Some(theme_id.clone()), variant: Some(variant_id.clone()), accent: Some(color), font_family: font_family.clone(), corners }, &set_appearance, &set_window);
            }}
        >
            <Flex size={(DOT, DOT)} align={Align::Center} justify={Justify::Center} />
        </RawButton>
    })
}

fn custom_accent_dot(current: &ResolvedAppearance, custom: &Signal<bool>) -> BoxedWidget {
    let active = custom.get();
    let set_custom = custom.clone();
    let border = if active {
        creamui_theme::use_theme().colors.text_primary
    } else {
        creamui_theme::use_theme().colors.border
    };
    Box::new(jsx! {
        <RawButton
            style={Style { layout: creamui_core::layout::Style { size: fixed(CUSTOM_W, DOT), ..Default::default() }, ..Default::default() }}
            background={creamui_theme::use_theme().colors.surface_elevated}
            border={(border, if active { 2.0 } else { 1.0 })}
            corner_radius={creamui_theme::use_theme().button_radius}
            on_click={move || set_custom.set(true)}
        >
            <Flex direction={FlexDirection::Row} size={(CUSTOM_W, DOT)} padding={4.0} gap={6.0} align={Align::Center}>
                <Flex size={(22.0, 22.0)} background={current.accent} corner_radius={11.0} />
                {Box::new(Text::new("Custom").size(TextSize::Sm)) as BoxedWidget}
            </Flex>
        </RawButton>
    })
}

fn custom_accent_picker(
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    picker: &ColorPickerController,
    custom: &Signal<bool>,
) -> BoxedWidget {
    if !custom.get() {
        return Box::new(jsx! { <Flex /> });
    }
    let set_appearance = appearance.clone();
    let set_window = window.clone();
    let theme_id = current.theme_id.clone();
    let variant_id = current.variant_id.clone();
    let font_family = current.font_family.clone();
    let corners = Some(current.corners);
    Box::new(ColorPicker::controlled(
        current.accent,
        picker,
        move |accent| {
            apply(
                AppearanceSelection {
                    theme: Some(theme_id.clone()),
                    variant: Some(variant_id.clone()),
                    accent: Some(accent),
                    font_family: font_family.clone(),
                    corners,
                },
                &set_appearance,
                &set_window,
            );
        },
    ))
}

fn apply(
    selection: AppearanceSelection,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
) {
    if let Err(error) = creamui_theme_loader::write_system_appearance(&selection) {
        eprintln!("settings: failed to save CreamUI appearance: {error}");
        return;
    }
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(next) => {
            if let Some(font_family) = &next.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            if let Some(handle) = window.borrow().as_ref() {
                handle.set_theme(next.theme);
            }
            appearance.set(next);
            if let Err(error) = coconut_core::ipc::publish_theme_reload() {
                eprintln!("settings: failed to publish theme reload: {error}");
            }
        }
        Err(error) => eprintln!("settings: failed to load CreamUI appearance: {error}"),
    }
}

fn title_case(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
