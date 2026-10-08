use crate::components::{group, row, section};
use crate::services::accounts::Account;
use crate::services::accounts::{set_own_avatar, sync_own_account};
use coconut_core::UserProfile;
use creamui_core::layout::{LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size, StateStyle, Style as WidgetStyle, Styled};
use creamui_theme::use_theme;
use creamui_widgets::layout::fixed;
use creamui_widgets::{
    RawButton, Select, SelectController, Text, TextController, TextInput, TextSize,
};

#[derive(Clone)]
pub struct ProfileControllers {
    first_name: TextController,
    last_name: TextController,
    city: TextController,
    region: TextController,
    country: TextController,
    latitude: Option<f64>,
    longitude: Option<f64>,
    country_select: SelectController,
    region_select: SelectController,
}

impl ProfileControllers {
    pub fn load(accounts: &[Account]) -> Self {
        let mut profile = UserProfile::load();
        if profile.display_name().is_empty() {
            if let Some(account) = accounts.iter().find(|account| {
                std::env::var("USER")
                    .ok()
                    .is_some_and(|username| username == account.username)
            }) {
                let mut names = account.real_name.splitn(2, ' ');
                profile.first_name = names.next().unwrap_or_default().to_owned();
                profile.last_name = names.next().unwrap_or_default().to_owned();
            }
        }
        let country_index = coconut_core::geo::countries("")
            .iter()
            .position(|item| item.code == profile.country)
            .unwrap_or(0);
        let region_index = coconut_core::geo::regions(&profile.country, "")
            .iter()
            .position(|item| item.code == profile.region)
            .unwrap_or(0);
        Self {
            first_name: TextController::new(profile.first_name),
            last_name: TextController::new(profile.last_name),
            city: TextController::new(profile.city),
            region: TextController::new(profile.region),
            country: TextController::new(profile.country),
            latitude: profile.latitude,
            longitude: profile.longitude,
            country_select: SelectController::new(country_index),
            region_select: SelectController::new(region_index),
        }
    }

    fn profile(&self) -> UserProfile {
        UserProfile {
            first_name: self.first_name.peek(),
            last_name: self.last_name.peek(),
            city: self.city.peek(),
            region: self.region.peek(),
            country: self.country.peek(),
            latitude: self.latitude,
            longitude: self.longitude,
        }
    }
}

pub fn build(_: Size, profile: &ProfileControllers) -> BoxedWidget {
    let own_profile = profile.clone();
    let save = Box::new(
        RawButton::new(button_style(), move || {
            let mut profile = own_profile.profile();
            let saved_profile = UserProfile::load();
            if profile.location() != saved_profile.location() {
                profile.latitude = None;
                profile.longitude = None;
            }
            if let Err(error) = profile.save() {
                eprintln!("settings: could not save profile: {error}");
                return;
            }
            if let Err(error) = coconut_core::ipc::publish_user_profile_changed() {
                eprintln!("settings: could not notify the desktop process about the profile update: {error}");
            }
            if let Err(error) = sync_own_account(&profile) {
                eprintln!("settings: could not update the system account: {error}");
            }
        })
        .child(Box::new(Text::new("Save profile").size(TextSize::Sm)) as BoxedWidget),
    ) as BoxedWidget;

    let avatar = avatar_button();
    let profile_group = group(vec![
        row("First name", input(&profile.first_name, "First name")),
        row("Last name", input(&profile.last_name, "Last name")),
        row("Avatar", avatar),
        row("City", input(&profile.city, "City")),
        row(
            "Country",
            country_search(&profile.country, &profile.country_select),
        ),
        row(
            "Region",
            region_search(&profile.country, &profile.region, &profile.region_select),
        ),
        row("", save),
    ]);

    section(
        "Users",
        "Your profile is used by the desktop. Location is saved for weather services.",
        vec![profile_group],
    )
}

fn input(controller: &TextController, placeholder: &str) -> BoxedWidget {
    Box::new(
        TextInput::controlled(controller)
            .placeholder(placeholder)
            .layout(Style {
                size: fixed(260.0, 34.0),
                padding: creamui_core::layout::Rect {
                    left: LengthPercentage::Length(12.0),
                    right: LengthPercentage::Length(12.0),
                    top: LengthPercentage::Length(0.0),
                    bottom: LengthPercentage::Length(0.0),
                },
                ..Default::default()
            }),
    )
}

fn country_search(controller: &TextController, select: &SelectController) -> BoxedWidget {
    let matches = coconut_core::geo::countries("");
    let labels: Vec<_> = matches
        .iter()
        .map(|item| format!("{} — {}", item.name, item.code))
        .collect();
    let options: Vec<_> = labels.iter().map(String::as_str).collect();
    let value = controller.clone();
    Box::new(
        Select::controlled(&options, select.clone())
            .searchable()
            .on_select(move |index| {
                if let Some(item) = matches.get(index) {
                    value.set_value(item.code.clone());
                }
            }),
    )
}

fn region_search(
    country: &TextController,
    controller: &TextController,
    select: &SelectController,
) -> BoxedWidget {
    let matches = coconut_core::geo::regions(&country.value(), "");
    let labels: Vec<_> = matches
        .iter()
        .map(|item| format!("{} — {}", item.name, item.code))
        .collect();
    let options: Vec<_> = labels.iter().map(String::as_str).collect();
    let value = controller.clone();
    Box::new(
        Select::controlled(&options, select.clone())
            .searchable()
            .on_select(move |index| {
                if let Some(item) = matches.get(index) {
                    value.set_value(item.code.clone());
                }
            }),
    )
}

fn avatar_button() -> BoxedWidget {
    Box::new(
        RawButton::new(button_style(), move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Choose an avatar")
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
            {
                if let Err(error) = set_own_avatar(&path) {
                    eprintln!("settings: could not update avatar: {error}");
                }
            }
        })
        .child(Box::new(Text::new("Choose image").size(TextSize::Sm)) as BoxedWidget),
    )
}

fn button_style() -> WidgetStyle {
    let theme = use_theme();
    WidgetStyle::new()
        .layout(Style {
            size: fixed(124.0, 34.0),
            ..Default::default()
        })
        .background(theme.surface_elevated)
        .corner_radius(theme.button_radius)
        .hover(StateStyle::new().background(theme.surface_hover))
}

pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let profile = &context.profile;
    build(size, profile)
}
