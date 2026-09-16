use crate::common::{group, row, section, update_config};
use coconut_core::{ClickAction, DesktopColor, IconShape, ShellConfig};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_theme::{use_theme, Color};
use creamui_widgets::layout::Align;
use creamui_widgets::{
    ColorPicker, ColorPickerController, SegmentedControl, Slider, Switch, Text, TextSize,
};

const ICON_SIZE_RANGE: (f32, f32) = (32.0, 140.0);
const SPACING_RANGE: (f32, f32) = (0.0, 48.0);
const PADDING_RANGE: (f32, f32) = (0.0, 32.0);
const SHAPES: [IconShape; 3] = [IconShape::Square, IconShape::Rounded, IconShape::Circle];
const CLICK_ACTIONS: [ClickAction; 2] = [ClickAction::Select, ClickAction::Open];

pub fn build(_: Size, config: &Signal<ShellConfig>, picker: &ColorPickerController) -> BoxedWidget {
    let icons = config.get().desktop.icons;

    let layout = group(vec![
        slider_row("Icon size", icons.size, ICON_SIZE_RANGE, config, |c| {
            &mut c.desktop.icons.size
        }),
        slider_row("Spacing", icons.spacing, SPACING_RANGE, config, |c| {
            &mut c.desktop.icons.spacing
        }),
        slider_row("Padding", icons.padding, PADDING_RANGE, config, |c| {
            &mut c.desktop.icons.padding
        }),
    ]);

    let shape_selected = SHAPES
        .iter()
        .position(|shape| *shape == icons.shape)
        .unwrap_or(0);
    let set_shape = config.clone();
    let shape_control: BoxedWidget = Box::new(
        SegmentedControl::new(shape_selected, move |index: usize| {
            update_config(&set_shape, |c| c.desktop.icons.shape = SHAPES[index]);
        })
        .option("Square")
        .option("Rounded")
        .option("Circle"),
    );

    let set_background = config.clone();
    let background_switch: BoxedWidget = Box::new(Switch::new(icons.background, move || {
        update_config(&set_background, |c| {
            c.desktop.icons.background = !c.desktop.icons.background;
        });
    }));

    let apply_color = config.clone();
    let color: BoxedWidget = Box::new(ColorPicker::controlled(
        Color::rgb(
            icons.background_color.r,
            icons.background_color.g,
            icons.background_color.b,
        ),
        picker,
        move |color| {
            update_config(&apply_color, |c| {
                c.desktop.icons.background_color = DesktopColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                };
            });
        },
    ));

    let background = group(vec![
        row("Background", background_switch),
        row("Background color", color),
        row("Shape", shape_control),
    ]);

    let click_selected = CLICK_ACTIONS
        .iter()
        .position(|action| *action == icons.click)
        .unwrap_or(0);
    let set_click = config.clone();
    let click_control: BoxedWidget = Box::new(
        SegmentedControl::new(click_selected, move |index: usize| {
            update_config(&set_click, |c| c.desktop.icons.click = CLICK_ACTIONS[index]);
        })
        .option("Select")
        .option("Open"),
    );
    let behavior = group(vec![row("Click", click_control)]);

    section(
        "Desktop icons",
        "How icons on the desktop grid look and space themselves.",
        vec![layout, background, behavior],
    )
}

fn slider_row(
    label: &str,
    value: f32,
    range: (f32, f32),
    config: &Signal<ShellConfig>,
    field: impl Fn(&mut ShellConfig) -> &mut f32 + Copy + 'static,
) -> BoxedWidget {
    let theme = use_theme();
    let (min, max) = range;
    let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let apply = config.clone();
    let slider: BoxedWidget = Box::new(Slider::new(normalized, move |normalized: f32| {
        update_config(&apply, |c| *field(c) = min + normalized * (max - min));
    }));
    let value_text = format!("{:.0} px", value);
    let control: BoxedWidget = Box::new(creamui_macros::jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={theme.spacing_medium}>
            {slider}
            {Box::new(Text::secondary(value_text).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    });
    row(label, control)
}
