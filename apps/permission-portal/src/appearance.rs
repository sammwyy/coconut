use creamui_theme::Theme;

pub fn load_system_theme() -> Theme {
    match coconut_integration_cream::load_system_appearance() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("coconut-permission-portal: failed to load CreamUI system theme: {error}");
            Theme::light()
        }
    }
}
