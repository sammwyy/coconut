use crate::components::{group, row, section};
use crate::services::accounts::Account;
use creamui_core::{BoxedWidget, Size};
use creamui_widgets::{Text, TextSize};

fn build(_: Size, accounts: &[Account], username: &str) -> BoxedWidget {
    let Some(account) = accounts.iter().find(|account| account.username == username) else {
        return section("User", "This account is no longer available.", Vec::new());
    };
    let name = if account.real_name.is_empty() {
        account.username.clone()
    } else {
        account.real_name.clone()
    };
    section(
        &name,
        "Account details",
        vec![group(vec![
            row(
                "Username",
                Box::new(Text::secondary(account.username.clone()).size(TextSize::Sm)),
            ),
            row(
                "Account type",
                Box::new(
                    Text::secondary(if account.administrator {
                        "Administrator"
                    } else {
                        "Standard account"
                    })
                    .size(TextSize::Sm),
                ),
            ),
        ])],
    )
}

pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let account_list = context.users.get();
    let username = creamui_router::use_params()
        .remove("username")
        .unwrap_or_default();
    build(size, &account_list, &username)
}
