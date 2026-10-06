use crate::common::icon_badge;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Style, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::WindowHandle;
use creamui_theme::{AppearanceSelection, Color, CornerStyle, ResolvedAppearance};
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{ColorPicker, ColorPickerController, RawView, SegmentedControl, Text};
use std::cell::RefCell;
use std::rc::Rc;

const THEME_CARD: (f32, f32) = (116.0, 82.0);
const VARIANT_CARD: (f32, f32) = (58.0, 50.0);
const DOT: f32 = 24.0;
const CUSTOM_W: f32 = 28.0;

pub struct OverviewControls {
    pub theme: BoxedWidget,
    pub style: BoxedWidget,
    pub accent: BoxedWidget,
    pub corners: BoxedWidget,
}

pub fn overview(
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    accent_picker: &ColorPickerController,
    custom: &Signal<bool>,
) -> OverviewControls {
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
    let mut variants = selected_theme.variants.iter().collect::<Vec<_>>();
    variants.sort_by_key(|(id, _)| match id.as_str() {
        "light" => 0,
        "dark" => 1,
        "midnight" => 2,
        _ => 3,
    });
    let variant_cards = variants
        .into_iter()
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
    if accent_dots.is_empty() {
        // Themes may omit presets; keep the standard palette usable as well
        // as the custom picker without replacing a theme's own choices.
        accent_dots = [
            Color::rgb(0, 153, 211),
            Color::rgb(227, 86, 86),
            Color::rgb(37, 166, 82),
            Color::rgb(130, 113, 233),
            Color::rgb(203, 125, 0),
            Color::rgb(212, 85, 145),
        ]
        .into_iter()
        .map(|color| accent_dot(color, String::new(), &current, appearance, window, custom))
        .collect();
    }
    let custom_dot = custom_accent_dot(&current, custom);
    let custom_picker = custom_accent_picker(&current, appearance, window, accent_picker, custom);
    accent_dots.push(custom_dot);
    accent_dots.push(custom_picker);

    OverviewControls {
        theme: compact_row(
            "Theme",
            r#"<path d="m14 4 6 6M3 21l4-1L21 6l-3-3L4 17Z"/>"#,
            selection_control(theme_cards),
        ),
        style: compact_row(
            "Style",
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>"#,
            selection_control(variant_cards),
        ),
        accent: compact_row(
            "Accent color",
            r#"<path d="M12 2s-7 8-7 13a7 7 0 0 0 14 0c0-5-7-13-7-13Z"/>"#,
            selection_control(accent_dots),
        ),
        corners: compact_row(
            "Corners",
            r#"<rect x="3" y="3" width="18" height="18" rx="5"/>"#,
            corners_control(&current, appearance, window),
        ),
    }
}

fn compact_row(label: &str, glyph: &str, control: BoxedWidget) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={creamui_core::layout::Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(creamui_core::layout::AlignItems::Center),
            gap: creamui_core::layout::Size { width: creamui_core::layout::LengthPercentage::Length(12.0), height: creamui_core::layout::LengthPercentage::Length(0.0) },
            min_size: creamui_core::layout::Size { width: creamui_core::layout::Dimension::Auto, height: creamui_core::layout::Dimension::Length(56.0) },
            padding: creamui_core::layout::Rect { left: creamui_core::layout::LengthPercentage::Length(16.0), right: creamui_core::layout::LengthPercentage::Length(16.0), top: creamui_core::layout::LengthPercentage::Length(10.0), bottom: creamui_core::layout::LengthPercentage::Length(10.0) },
            ..Default::default()
        }} align={Align::Center} gap={12.0}>
            {icon_badge(crate::icons::outline(glyph), Color::rgb(185, 87, 178))}
            <Flex grow={1.0}>{Box::new(Text::new(label.to_owned())) as BoxedWidget}</Flex>
            {control}
        </Flex>
    })
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
    Box::new(CornerStyle::ALL.iter().fold(control, |control, style| {
        control.option(title_case(style.id()))
    }))
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
            corner_radius={12.0}
            on_click={move || apply(AppearanceSelection { theme: Some(theme_id.clone()), variant: Some(variant_id.clone()), accent: Some(accent), font_family: font_family.clone(), corners }, &set_appearance, &set_window)}
        >
            <Flex direction={FlexDirection::Column} size={VARIANT_CARD} padding={4.0} gap={3.0}>
                <Flex direction={FlexDirection::Row} grow={1.0} padding={4.0} gap={4.0} background={variant.colors.surface} corner_radius={7.0}>
                    <Flex grow={1.0} background={variant.colors.text_secondary.mix(variant.colors.surface, 0.55)} corner_radius={6.0} />
                    <Flex grow={1.0} background={variant.colors.surface_elevated} corner_radius={6.0} />
                </Flex>
                <Flex justify={Justify::Center}>
                    <RawText color={creamui_theme::use_theme().colors.text_primary} font_size={9.0}>{label}</RawText>
                </Flex>
            </Flex>
        </RawButton>
    })
}

fn accent_dot(
    preset_color: Color,
    _name: String,
    current: &ResolvedAppearance,
    appearance: &Signal<ResolvedAppearance>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    custom: &Signal<bool>,
) -> BoxedWidget {
    let active = preset_color == current.accent;
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
            background={preset_color}
            outline={(if active { creamui_theme::use_theme().colors.text_secondary } else { Color::rgba(0, 0, 0, 0) }, 2.0)}
            corner_radius={DOT / 2.0}
            on_click={move || {
                set_custom.set(false);
                apply(AppearanceSelection { theme: Some(theme_id.clone()), variant: Some(variant_id.clone()), accent: Some(preset_color), font_family: font_family.clone(), corners }, &set_appearance, &set_window);
            }}
        >
            <Flex size={(DOT, DOT)} align={Align::Center} justify={Justify::Center}>
                {Box::new(RawView::new(creamui_core::layout::Style { size: fixed(6.0, 6.0), ..Default::default() }).background(if active { Color::rgb(255, 255, 255) } else { Color::rgba(0, 0, 0, 0) }).corner_radius(3.0)) as BoxedWidget}
            </Flex>
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
            corner_radius={CUSTOM_W / 2.0}
            on_click={move || set_custom.set(!set_custom.peek())}
        >
            <Flex size={(CUSTOM_W, DOT)} align={Align::Center} justify={Justify::Center}>
                {Box::new(creamui_widgets::Icon::new(crate::icons::outline(r#"<path d="M12 2s-7 8-7 13a7 7 0 0 0 14 0c0-5-7-13-7-13Z"/>"#), if active { current.accent } else { creamui_theme::use_theme().colors.text_secondary }).size(14.0)) as BoxedWidget}
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

pub(super) fn apply(
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
                handle.set_theme(coconut_plugin_kit::design::coconut_appearance(&next));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::group;
    use creamui_core::{Renderer, Size};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Theme, ThemeProvider};

    #[test]
    fn preset_colors_render_and_custom_picker_remains_accessible() {
        creamui_reactive::with_context_scope(|| {
            let theme = crate::common::settings_theme(Theme::light());
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let appearance = Signal::new(ResolvedAppearance {
                theme_id: "default".into(),
                variant_id: "light".into(),
                theme,
                accent: theme.colors.accent,
                corners: Default::default(),
                font_family: None,
            });
            let window = Rc::new(RefCell::new(None));
            let picker = ColorPickerController::new();
            let custom = Signal::new(false);
            let preset = creamui_theme_loader::builtin_theme().accent_presets[0].color;
            let dot = accent_dot(
                preset,
                String::new(),
                &appearance.peek(),
                &appearance,
                &window,
                &custom,
            );
            assert_eq!(dot.style().paint.background, Some(preset.into()));
            let OverviewControls {
                theme: theme_control,
                style,
                accent,
                corners,
            } = overview(&appearance, &window, &picker, &custom);
            let root = Box::new(
                creamui_widgets::layout::Flex::column()
                    .size(688.0, 300.0)
                    .child(group(vec![theme_control, style, accent, corners])),
            );
            let mut recorder = SceneRecorder::new();
            recorder.begin(688, 300, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = Renderer::new().render(
                root,
                Size {
                    width: 688.0,
                    height: 300.0,
                },
                &mut recorder,
            );
            let list = recorder.finish();
            let mut raster = Rasterizer::new(688, 300);
            raster.render(&list, &Damage::Full);
            for preset in creamui_theme_loader::builtin_theme().accent_presets {
                assert!(
                    raster
                        .pixmap()
                        .pixels()
                        .iter()
                        .any(|pixel| pixel.red() == preset.color.r
                            && pixel.green() == preset.color.g
                            && pixel.blue() == preset.color.b
                            && pixel.alpha() == 255),
                    "{} preset must be visible",
                    preset.name
                );
            }
            scene
                .hit_test(creamui_core::Point { x: 654.0, y: 202.0 })
                .expect("custom accent control stays clickable")();
            assert!(
                custom.peek(),
                "custom picker opens without saving an appearance change"
            );
        });
    }
}
