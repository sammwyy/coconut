use crate::services::accounts::Account;
use creamui_theme::use_theme;
use creamui_widgets::{IconImage, IconSource};

/// The account's avatar, decoded from disk, or a filled circle with its
/// display name's initial when it has none set.
pub fn account_icon(account: &Account) -> IconSource {
    if let Some(image) = account
        .icon_path
        .as_deref()
        .and_then(|path| creamui_image::ImageData::from_path(path).ok())
    {
        return IconSource::Image(IconImage {
            image: image.image().clone(),
            monochrome: false,
        });
    }
    let theme = use_theme();
    let display_name = if account.real_name.is_empty() {
        &account.username
    } else {
        &account.real_name
    };
    IconSource::Initial {
        letter: display_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_ascii_uppercase(),
        background: theme.accent,
        text_color: theme.selection_text,
    }
}
