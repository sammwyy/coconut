use crate::routes::destination::Section;
use creamui_core::BoxedWidget;

use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
pub(crate) fn overview() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let native = &context.native;
    let account_list = context.users.get();
    build(native, &account_list, view)
}
pub mod account;
pub mod create;
pub mod profile;

fn build(
    state: &State,
    accounts: &[crate::services::accounts::Account],
    nav: &creamui_router::Router,
) -> BoxedWidget {
    let username = std::env::var("USER").unwrap_or_default();
    let own = accounts.iter().find(|a| a.username == username);
    let display = own
        .map(|a| {
            if a.real_name.is_empty() {
                a.username.as_str()
            } else {
                a.real_name.as_str()
            }
        })
        .unwrap_or(&username);
    let others = accounts
        .iter()
        .filter(|a| a.username != username)
        .map(|a| {
            link(
                &a.real_name,
                &a.username,
                Section::User(a.username.clone()),
                nav,
            )
        })
        .collect::<Vec<_>>();
    page(
        vec![
            card(
                "You",
                vec![
                    link(display, "Edit profile and avatar", Section::Profile, nav),
                    key_value("Username", &username),
                    key_value(
                        "Account type",
                        own.map(|a| {
                            if a.administrator {
                                "Administrator"
                            } else {
                                "Standard"
                            }
                        })
                        .unwrap_or("Unavailable"),
                    ),
                    detail_link(
                        "Password",
                        "Change the current account password",
                        Page::Password,
                        nav,
                    ),
                ],
            ),
            card(
                "Other users",
                if others.is_empty() {
                    vec![key_value(
                        "Accounts",
                        "No other accounts reported by AccountsService",
                    )]
                } else {
                    others
                },
            ),
            card(
                "Management",
                vec![
                    detail_link(
                        "Add user",
                        "Requires administrator authorization",
                        Page::AddUser,
                        nav,
                    ),
                    detail_link(
                        "Online accounts",
                        "Accounts managed by the active desktop",
                        Page::OnlineAccounts,
                        nav,
                    ),
                ],
            ),
        ],
        state,
    )
}
pub mod online_accounts;
pub mod password;
