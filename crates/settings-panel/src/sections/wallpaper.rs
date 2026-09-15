use crate::common::{section, update_config};
use coconut_core::ShellConfig;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::{FilePicker, Text, TextSize};
use std::path::PathBuf;

pub fn build(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let theme = use_theme();
    let current = config
        .get()
        .desktop
        .wallpaper
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    let set_wallpaper = config.clone();
    let picker: BoxedWidget = Box::new(
        FilePicker::new(current.clone(), move |path: PathBuf| {
            update_config(&set_wallpaper, |c| c.desktop.wallpaper = Some(path));
        })
        .title("Choose a wallpaper")
        .filter("Images", ["png", "jpg", "jpeg", "webp"]),
    );

    let clear_config = config.clone();
    let clear: BoxedWidget = Box::new(jsx! {
        <Button on_click={move || update_config(&clear_config, |c| c.desktop.wallpaper = None)}>"Clear"</Button>
    });

    let caption = if current.is_empty() {
        "Using the built-in background.".to_owned()
    } else {
        format!("Applies after Coconut's shell restarts. Current: {current}")
    };

    section(
        "Wallpaper",
        "The desktop background image.",
        vec![
            Box::new(jsx! {
                <Flex direction={FlexDirection::Row} gap={theme.spacing_medium}>
                    {picker}
                    {clear}
                </Flex>
            }),
            Box::new(Text::new(caption).size(TextSize::Sm)),
        ],
    )
}
