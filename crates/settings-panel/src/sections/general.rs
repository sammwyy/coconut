use crate::common::{section, update_config};
use coconut_core::{BarPosition, ShellConfig};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::{SegmentedControl, Text, TextSize};

const POSITIONS: [BarPosition; 2] = [BarPosition::Top, BarPosition::Bottom];

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let theme = use_theme();
    let selected = POSITIONS
        .iter()
        .position(|position| *position == config.get().bar.position)
        .unwrap_or(0);
    let set_position = config.clone();
    let control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index: usize| {
            update_config(&set_position, |c| c.bar.position = POSITIONS[index]);
        })
        .option("Top")
        .option("Bottom"),
    );
    let row = jsx! {
        <Flex direction={FlexDirection::Column} gap={theme.spacing_small}>
            {Box::new(Text::new("Bar position").size(TextSize::Sm)) as BoxedWidget}
            {control}
        </Flex>
    };
    section(
        "General",
        "Where the dock sits on screen.",
        vec![Box::new(row)],
    )
}
