use crate::common::{group, row, section, update_config};
use coconut_core::{BarPosition, ShellConfig};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::SegmentedControl;

const POSITIONS: [BarPosition; 2] = [BarPosition::Top, BarPosition::Bottom];

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
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

    section(
        "Bar",
        "Choose where the bar sits on screen.",
        vec![group(vec![row("Bar position", control)])],
    )
}
