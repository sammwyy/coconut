use coconut_core::ShellConfig;
use creamui_core::BoxedWidget;

use crate::components::system_settings::*;

pub(crate) fn route_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let state = &context.native;
    let config = &context.config;
    let body = {
        let state = state.clone();
        let config = config.clone();
        vec![value("Reset only Coconut's panels, dock, desktop and asset-pack configuration. Your files, accounts, OS, Blair configuration and system theme are not reset. The previous shell configuration is backed up first."),
            action("Back up and reset shell settings", move || {
                match config.peek().reset_with_backup() { Ok(path) => { config.set(ShellConfig::default()); let _ = coconut_core::ipc::publish_shell_config(&config.peek()); state.status.set(format!("Reset complete. Backup: {}", path.display())); }, Err(error) => state.status.set(format!("Could not reset: {error}")) }
            })]
    };
    detail_page(body, state)
}
