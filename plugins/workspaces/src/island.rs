use coconut_api::desktop::{Workspace, WorkspaceCapabilities};
use coconut_plugin_kit::chrome::{
    shell_control_hover, shell_island, shell_island_border as shell_border, shell_muted, shell_text,
};
use coconut_plugin_kit::{design, Island, IslandRenderContext};
use creamui_core::layout::Style as LayoutStyle;
use creamui_core::{BoxedWidget, StateStyle, Style, Styled};
use creamui_reactive::Signal;
use creamui_theme::Color;
use creamui_widgets::layout::{fixed, Align, Flex, Justify};
use creamui_widgets::{RawButton, RawText};
use std::rc::Rc;

const CHIP_WIDTH: f32 = 26.0;
const CHIP_HEIGHT: f32 = 24.0;

/// The live workspace list plus the management callbacks
/// [`WorkspacesIsland`] needs. A dedicated newtype (rather than bare
/// signals/`Rc` closures) since `SharedState` is keyed by type alone.
#[derive(Clone)]
pub struct WorkspaceListState {
    pub workspaces: Signal<Vec<Workspace>>,
    pub capabilities: Signal<WorkspaceCapabilities>,
    pub activate: Rc<dyn Fn(String)>,
    pub create: Rc<dyn Fn() -> Option<String>>,
    pub refresh: Rc<dyn Fn()>,
}

impl Default for WorkspaceListState {
    fn default() -> Self {
        Self {
            workspaces: Signal::new(Vec::new()),
            capabilities: Signal::new(WorkspaceCapabilities::default()),
            activate: Rc::new(|_| {}),
            create: Rc::new(|| None),
            refresh: Rc::new(|| {}),
        }
    }
}

/// The dock's workspace switcher: one chip per compositor workspace plus a
/// trailing "+" chip when the backend can create workspaces on demand.
/// Also registered as "logo" for `shell.toml`s written before the rename.
pub struct WorkspacesIsland(pub &'static str);

impl Island for WorkspacesIsland {
    fn id(&self) -> &'static str {
        self.0
    }

    fn is_visible(&self, ctx: &IslandRenderContext<'_>) -> bool {
        let state = workspace_state(ctx);
        !state.workspaces.get().is_empty() || state.capabilities.get().create
    }

    fn build(&self, ctx: &IslandRenderContext<'_>) -> BoxedWidget {
        let s = ctx.scale;
        let state = workspace_state(ctx);
        let workspaces = state.workspaces.get();
        let capabilities = state.capabilities.get();

        let mut chips: Vec<BoxedWidget> = Vec::new();
        for (index, workspace) in workspaces.iter().enumerate() {
            let id = workspace.id.clone();
            let activate = state.activate.clone();
            let refresh = state.refresh.clone();
            chips.push(Box::new(
                RawButton::new(chip_style(workspace.active, s), move || {
                    activate(id.clone());
                    refresh();
                })
                .child(workspace_label(workspace, index, s)),
            ));
        }
        if capabilities.create {
            let create = state.create.clone();
            let activate = state.activate.clone();
            let refresh = state.refresh.clone();
            chips.push(Box::new(
                RawButton::new(chip_style(false, s), move || {
                    if let Some(id) = create() {
                        activate(id);
                    }
                    refresh();
                })
                .child(Box::new(
                    Flex::row()
                        .size(CHIP_WIDTH * s, CHIP_HEIGHT * s)
                        .align(Align::Center)
                        .justify(Justify::Center)
                        .child(Box::new(
                            RawText::new("+", shell_muted(), 11.0 * s).bold(true),
                        )),
                )),
            ));
        }

        let mut container = if ctx.position.is_vertical() {
            Flex::column()
        } else {
            Flex::row()
        }
        .padding(4.0 * s)
        .gap(2.0 * s)
        .align(Align::Center)
        .background(shell_island())
        .border(shell_border(), 1.0)
        .corner_radius(16.0 * s);
        for chip in chips {
            container = container.child(chip);
        }
        Box::new(if shell_island().a > 0 {
            container.box_shadow(design::island_shadow(s))
        } else {
            container
        })
    }
}

fn workspace_state(ctx: &IslandRenderContext<'_>) -> WorkspaceListState {
    ctx.shared.get::<WorkspaceListState>().unwrap_or_default()
}

fn workspace_label(workspace: &Workspace, index: usize, scale: f32) -> BoxedWidget {
    let label = chip_label(workspace, index);
    let color = if workspace.active {
        let surface = creamui_theme::use_theme().colors.surface;
        Color::rgb(surface.r, surface.g, surface.b)
    } else if workspace.window_count > 0 {
        shell_text()
    } else {
        shell_muted()
    };
    Box::new(
        Flex::row()
            .size(CHIP_WIDTH * scale, CHIP_HEIGHT * scale)
            .align(Align::Center)
            .justify(Justify::Center)
            .child(Box::new(
                RawText::new(label, color, 11.0 * scale).bold(true),
            )),
    )
}

/// Compositor-provided names only render when short enough for a chip;
/// longer ones (KWin's "Desktop N" fallbacks) collapse to the 1-based slot.
fn chip_label(workspace: &Workspace, index: usize) -> String {
    if !workspace.name.is_empty() && workspace.name.chars().count() <= 6 {
        workspace.name.clone()
    } else {
        (index + 1).to_string()
    }
}

fn chip_style(active: bool, scale: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(CHIP_WIDTH * scale, CHIP_HEIGHT * scale),
            ..Default::default()
        })
        .background(if active {
            shell_text()
        } else {
            Color::rgba(0, 0, 0, 0)
        })
        .corner_radius(12.0 * scale)
        .hover(StateStyle::new().background(if active {
            shell_text()
        } else {
            shell_control_hover()
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str) -> Workspace {
        Workspace {
            id: "1".into(),
            name: name.into(),
            active: false,
            window_count: 0,
            output: None,
        }
    }

    #[test]
    fn chip_labels_use_short_names_and_fall_back_to_the_slot_number() {
        assert_eq!(chip_label(&workspace("dev"), 0), "dev");
        assert_eq!(chip_label(&workspace(""), 2), "3");
        assert_eq!(chip_label(&workspace("Desktop 3"), 2), "3");
    }
}
