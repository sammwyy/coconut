use crate::*;

#[derive(Clone)]
pub(crate) struct ViewContext {
    pub size: Size,
    pub config: Signal<ShellConfig>,
    pub integrations: Rc<coconut_api::Registry>,
    pub appearance: Signal<creamui_theme::ResolvedAppearance>,
    pub view: Router,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    pub wallpaper_color_picker: ColorPickerController,
    pub desktop_icons_color_picker: ColorPickerController,
    pub appearance_accent_picker: ColorPickerController,
    pub appearance_custom_accent: Signal<bool>,
    pub users: Signal<Vec<users::Account>>,
    pub profile: users::ProfileControllers,
    pub icons: SettingsIcons,
    pub wallpaper_gallery: crate::views::personalization::wallpaper::GalleryState,
    pub window_settings: Rc<crate::views::windows::WindowState>,
    pub shortcut_settings: Rc<crate::views::windows::ShortcutState>,
    pub content_scroll: ScrollController,
    pub sidebar_scroll: ScrollController,
    pub dock_scroll: ScrollController,
    pub island_settings: crate::views::desktop::island_settings::State,
    pub settings_search: TextController,
    pub connectivity: crate::views::connectivity::State,
    pub maximized: Signal<bool>,
    pub native: crate::views::native::State,
}
