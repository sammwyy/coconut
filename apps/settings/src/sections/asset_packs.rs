use crate::common::{section, update_config};
use coconut_core::ShellConfig;
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size, Style};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::use_theme;
use creamui_widgets::layout::{fixed, Align, Justify, Wrap};
use creamui_widgets::{Text, TextSize};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const CARD_W: f32 = 148.0;
const CARD_H: f32 = 96.0;

pub fn icon_packs(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let selected = config.get().appearance.icon_theme;
    packs(
        "Icons",
        "Choose the icon style used by your apps and desktop.",
        list_packs("icons"),
        &selected,
        config,
        |shell| &mut shell.appearance.icon_theme,
    )
}

pub fn sound_themes(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let selected = config.get().appearance.sound_theme;
    packs(
        "Sound",
        "Choose the sound style used for desktop feedback.",
        list_packs("sounds"),
        &selected,
        config,
        |shell| &mut shell.appearance.sound_theme,
    )
}

pub fn cursor_themes(_: Size, config: &Signal<ShellConfig>) -> BoxedWidget {
    let selected = config.get().appearance.cursor_theme;
    packs(
        "Cursor",
        "Choose the mouse pointer theme.",
        list_cursor_packs(),
        &selected,
        config,
        |shell| &mut shell.appearance.cursor_theme,
    )
}

fn packs(
    title: &str,
    subtitle: &str,
    mut packs: Vec<String>,
    selected: &str,
    config: &Signal<ShellConfig>,
    field: impl Fn(&mut ShellConfig) -> &mut String + Copy + 'static,
) -> BoxedWidget {
    if !packs.iter().any(|pack| pack == selected) {
        packs.insert(0, selected.to_owned());
    }
    let theme = use_theme();
    let cards = packs
        .into_iter()
        .map(|pack| {
            let active = pack == selected;
            let set_pack = config.clone();
            let selected_pack = pack.clone();
            let preview = if active {
                theme.colors.accent
            } else {
                theme.colors.surface_hover
            };
            Box::new(jsx! {
                <RawButton
                    style={Style { layout: creamui_core::layout::Style { size: fixed(CARD_W, CARD_H), ..Default::default() }, ..Default::default() }}
                    background={theme.colors.surface_elevated}
                    border={(if active { theme.colors.accent } else { theme.colors.border }, if active { 2.0 } else { 1.0 })}
                    corner_radius={theme.card_radius}
                    on_click={move || update_config(&set_pack, |shell| *field(shell) = selected_pack.clone())}
                >
                    <Flex direction={FlexDirection::Column} size={(CARD_W, CARD_H)} padding={10.0} gap={10.0} align={Align::Start} justify={Justify::Between}>
                        <Flex size={(42.0, 42.0)} background={preview} corner_radius={theme.radius_medium} />
                        {Box::new(Text::new(pack).size(TextSize::Sm)) as BoxedWidget}
                    </Flex>
                </RawButton>
            }) as BoxedWidget
        })
        .collect::<Vec<_>>();
    section(
        title,
        subtitle,
        vec![Box::new(jsx! {
            <Flex direction={FlexDirection::Row} wrap={Wrap::Wrap} gap={theme.spacing_medium} children={cards} />
        })],
    )
}

fn list_packs(kind: &str) -> Vec<String> {
    let mut packs = BTreeSet::new();
    for directory in xdg_data_directories() {
        let Ok(entries) = fs::read_dir(directory.join(kind)) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    packs.insert(name.to_owned());
                }
            }
        }
    }
    packs.into_iter().collect()
}

/// Cursor themes share the same `icons/<name>/` directories as regular icon
/// themes (there's no dedicated `cursors/` top-level folder in the
/// freedesktop layout), so a theme only counts here if it also ships a
/// `cursors` subfolder — otherwise `list_packs("icons")` would list every
/// icon theme as if it were a cursor theme too.
fn list_cursor_packs() -> Vec<String> {
    let mut packs = BTreeSet::new();
    for directory in xdg_data_directories() {
        let Ok(entries) = fs::read_dir(directory.join("icons")) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("cursors").is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    packs.insert(name.to_owned());
                }
            }
        }
    }
    packs.into_iter().collect()
}

fn xdg_data_directories() -> Vec<PathBuf> {
    let mut directories = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .into_iter()
        .collect::<Vec<_>>();
    let system =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    directories.extend(std::env::split_paths(&system));
    directories
}
