use creamshell_core::ShellConfig;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{platform::WindowRole, AppBuilder, WindowOptions};
use creamui_theme::Theme;
use creamui_widgets::layout::Align;
use creamui_widgets::{Heading, Text, TextSize};

pub fn run() {
    let config = ShellConfig::load();

    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                WindowOptions {
                    title: "CreamShell Settings".into(),
                    width: 640,
                    height: 420,
                    decorations: true,
                    resizable: true,
                    transparent: false,
                    role: WindowRole::Normal,
                    theme: Theme::dark(),
                    ..Default::default()
                },
                Theme::dark().colors.surface,
                |_| {},
                move |size| build(size, &config),
            );
        })
        .run();
}

fn build(size: Size, config: &ShellConfig) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(size.width, size.height)} padding={24.0} gap={16.0} align={Align::Start}>
            {Box::new(Heading::xl("CreamShell Settings")) as BoxedWidget}
            {Box::new(Text::new("This window is a placeholder: the settings UI itself hasn't been built yet.").size(TextSize::Sm)) as BoxedWidget}
            {Box::new(Text::new(format!("Bar position: {:?}", config.bar.position)).size(TextSize::Sm)) as BoxedWidget}
            {Box::new(Text::new(format!("Tray mode: {:?}", config.tray.mode)).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    })
}
