use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let window_settings = &context.window_settings;
    let native = &context.native;
    build(native, window_settings)
}

fn build(state: &State, window: &crate::views::windows::WindowState) -> BoxedWidget {
    page(
        vec![
            card(
                "Vision",
                vec![
                    state.preference(
                        "High contrast",
                        "contrast",
                        "Increase contrast between interface elements",
                    ),
                    state.preference("Screen zoom", "zoom", "Magnify the desktop"),
                    state.preference("Text size", "text-scale", "Interface text scaling"),
                    window.reduce_motion(state.preference(
                        "Reduce motion",
                        "reduce-motion",
                        "Limit animation and transitions",
                    )),
                ],
            ),
            card(
                "Interaction",
                vec![
                    state.preference(
                        "Visual alerts",
                        "visual-alerts",
                        "Flash the screen instead of ringing a bell",
                    ),
                    state.preference(
                        "Sticky keys",
                        "sticky-keys",
                        "Enter modifier keys one at a time",
                    ),
                ],
            ),
        ],
        state,
    )
}
